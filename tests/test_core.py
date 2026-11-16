import json
import tempfile
import unittest
from virtual_mixer.core import AudioCore, PortAudioAdapter, Project


class CoreTests(unittest.TestCase):
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
