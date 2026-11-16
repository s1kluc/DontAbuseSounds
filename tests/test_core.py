import json
import tempfile
import unittest
from virtual_mixer.core import AudioCore, Device, PortAudioAdapter, Project


class CoreTests(unittest.TestCase):
    def test_play_resamples_audio_for_output(self):
        import numpy as np
        import soundfile as sf

        class Stream:
            def start(self): pass
        class Adapter:
            def stream(self, device, channels, callback): return Stream()

        for source_rate, target_rate, source_channels, output_channels in (
                (44100, 48000, 1, 2), (48000, 44100, 2, 1), (48000, 48000, 2, 2)):
            with self.subTest(source_rate=source_rate, target_rate=target_rate), tempfile.TemporaryDirectory() as root:
                signal = .25 * np.sin(2 * np.pi * 440 * np.arange(source_rate // 10) / source_rate)
                source = np.repeat(signal[:, None], source_channels, axis=1)
                path = root + '/tone.wav'
                sf.write(path, source, source_rate, subtype='FLOAT')
                core = AudioCore(Adapter())
                core.select_output(Device('test', 0, 'Test', 0, output_channels, target_rate, False))
                core.play(0, path)
                data, position = core.voices[0]
                self.assertEqual(data.shape, (target_rate // 10, output_channels))
                self.assertEqual(data.dtype, np.float32)
                self.assertEqual(position, 0)
                expected = .25 * np.sin(2 * np.pi * 440 * np.arange(len(data)) / target_rate)
                np.testing.assert_allclose(data[100:-100, 0], expected[100:-100], atol=1e-4)
                if output_channels == 2:
                    np.testing.assert_array_equal(data[:, 0], data[:, 1])

    def test_ids_survive_reordered_devices(self):
        for host in ('CoreAudio', 'WASAPI', 'ALSA'):
            class Backend:
                names = ['Virtual Cable', 'Microphone']
                def query_hostapis(self): return [{'name': host}]
                def query_devices(self):
                    return [dict(name=n, hostapi=0, max_input_channels=1,
                                 max_output_channels=2, default_samplerate=48000) for n in self.names]
            adapter = PortAudioAdapter.__new__(PortAudioAdapter)
            adapter.backend = Backend()
            before = adapter.devices()[0]
            adapter.backend.names.reverse()
            after = adapter.devices()[1]
            self.assertEqual(before.id, after.id)
            self.assertNotEqual(before.index, after.index)
            self.assertTrue(after.virtual)

    def test_disconnect_stops_voices_and_closes_stream(self):
        class Stream:
            stopped = closed = False
            def stop(self): self.stopped = True
            def close(self): self.closed = True
        core = AudioCore(None)
        stream = Stream()
        core.stream = stream
        core.voices[1] = ('audio', 0)
        core.select_output(None)
        self.assertFalse(core.playing())
        self.assertTrue(stream.stopped and stream.closed)
        self.assertIsNone(core.output)

    def test_persistence_and_invalid_paths(self):
        with tempfile.TemporaryDirectory() as root:
            project = Project(root)
            project.pads[2] = dict(name='Bell', file='bell.wav')
            project.output_id = 'device'
            project.save()
            restored = Project(root)
            restored.load()
            self.assertEqual(restored.pads, project.pads)
            self.assertEqual(restored.output_id, 'device')
            data = json.loads(project.file.read_text())
            data['pads'][2]['file'] = '../escape.wav'
            project.file.write_text(json.dumps(data))
            original = project.file.read_bytes()
            with self.assertRaises(ValueError): restored.load()
            self.assertEqual(project.file.read_bytes(), original)


if __name__ == '__main__': unittest.main()
