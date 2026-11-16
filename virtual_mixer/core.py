"""Audio and project logic independent of the desktop UI."""
from dataclasses import dataclass
from pathlib import Path
from threading import Lock
import hashlib
import json
import os
import sys
import uuid


@dataclass(frozen=True)
class Device:
    id: str
    index: int
    name: str
    inputs: int
    outputs: int
    rate: int
    virtual: bool
    connected: bool = True


class PortAudioAdapter:
    """PortAudio selects CoreAudio, WASAPI/MME, or ALSA/PulseAudio by platform."""
    def __init__(self):
        import sounddevice
        self.backend = sounddevice

    def devices(self):
        sd = self.backend
        hosts = sd.query_hostapis()
        result = []
        occurrences = {}
        for index, item in enumerate(sd.query_devices()):
            host = hosts[item['hostapi']]['name']
            key = f"{sys.platform}:{host}:{item['name']}"
            occurrence = occurrences.get(key, 0)
            occurrences[key] = occurrence + 1
            # shortcut: PortAudio has no persistent hardware UID; identical device names
            # use enumeration order. Replace with platform UIDs if duplicates are common.
            identity = hashlib.sha256(f'{key}:{occurrence}'.encode()).hexdigest()[:24]
            name = item['name']
            virtual = any(word in name.lower() for word in
                          ('blackhole', 'vb-audio', 'cable input', 'soundflower',
                           'loopback', 'virtual', 'null sink'))
            result.append(Device(identity, index, f'{name} · {host}',
                                 item['max_input_channels'], item['max_output_channels'],
                                 int(item['default_samplerate']), virtual))
        return result

    def stream(self, device, channels, callback):
        return self.backend.OutputStream(device=device.index, samplerate=device.rate,
                                         channels=channels, dtype='float32', callback=callback)


class AudioCore:
    def __init__(self, adapter):
        self.adapter = adapter
        self.output = None
        self.stream = None
        self.voices = {}
        self.lock = Lock()
        self.status = ''

    def select_output(self, device):
        self.close()
        self.output = device

    def play(self, pad_id, path):
        import numpy as np
        import soundfile as sf
        import soxr
        if self.output is None:
            raise ValueError('Select an available output device first.')
        device = self.output
        channels = min(2, device.outputs)
        if channels < 1:
            raise ValueError('The selected device has no output channels.')
        info = sf.info(str(path))
        # Bound full-file decoding before allocating memory.
        if info.duration > 300 or info.frames * info.channels > 30_000_000:
            raise ValueError('Use a sound shorter than five minutes (maximum 30 million samples).')
        data, rate = sf.read(str(path), dtype='float32', always_2d=True)
        if not len(data) or not np.isfinite(data).all():
            raise ValueError('The audio file is empty or contains invalid samples.')
        if data.shape[1] > 2:
            raise ValueError('Import mono or stereo audio.')
        if channels == 1:
            data = data.mean(axis=1, keepdims=True)
        elif data.shape[1] == 1:
            data = np.repeat(data, 2, axis=1)
        if rate != device.rate:
            data = soxr.resample(data, rate, device.rate)
        if self.stream is None:
            stream = self.adapter.stream(device, channels, self.render)
            try:
                stream.start()
            except Exception:
                stream.close()
                raise
            self.stream = stream
        with self.lock:
            self.voices[pad_id] = (data, 0)

    def render(self, outdata, frames, time, status):
        import numpy as np
        outdata.fill(0)
        if status:
            self.status = str(status)
        with self.lock:
            for pad_id, (data, position) in list(self.voices.items()):
                count = min(frames, len(data) - position)
                outdata[:count] += data[position:position + count]
                if position + count == len(data):
                    del self.voices[pad_id]
                else:
                    self.voices[pad_id] = (data, position + count)
        np.clip(outdata, -1, 1, out=outdata)

    def stop(self, pad_id=None):
        with self.lock:
            if pad_id is None:
                self.voices.clear()
            else:
                self.voices.pop(pad_id, None)

    def playing(self):
        with self.lock:
            return set(self.voices)

    def close(self):
        self.stop()
        stream, self.stream = self.stream, None
        if stream is not None:
            try:
                stream.stop()
            finally:
                stream.close()


class Project:
    def __init__(self, root):
        self.root = Path(root)
        self.file = self.root / 'project.json'
        self.pads = [None] * 12
        self.input_id = None
        self.output_id = None

    def load(self):
        if not self.file.exists():
            return
        value = json.loads(self.file.read_text())
        pads = value.get('pads')
        if value.get('version') != 1 or not isinstance(pads, list) or len(pads) != 12:
            raise ValueError('Unsupported or damaged project file; original file retained.')
        for pad in pads:
            if pad is not None and (not isinstance(pad, dict) or
                                    not isinstance(pad.get('file'), str) or
                                    Path(pad['file']).name != pad['file'] or
                                    not isinstance(pad.get('name'), str)):
                raise ValueError('Invalid pad assignment; original file retained.')
        for key in ('input_id', 'output_id'):
            if value.get(key) is not None and not isinstance(value[key], str):
                raise ValueError('Invalid device setting.')
        self.pads = pads
        self.input_id = value.get('input_id')
        self.output_id = value.get('output_id')

    def save(self):
        self.root.mkdir(parents=True, exist_ok=True)
        temporary = self.root / 'project.json.tmp'
        with temporary.open('w') as handle:
            json.dump(dict(version=1, pads=self.pads, input_id=self.input_id,
                           output_id=self.output_id), handle, indent=2)
            handle.flush()
            os.fsync(handle.fileno())
        temporary.replace(self.file)

    def import_pad(self, index, source):
        import shutil
        import soundfile as sf
        source = Path(source)
        info = sf.info(str(source))
        if info.frames <= 0 or info.channels not in (1, 2) or info.duration > 300 or info.frames * info.channels > 30_000_000:
            raise ValueError('Choose nonempty mono/stereo audio up to five minutes and 30 million samples.')
        assets = self.root / 'sounds'
        assets.mkdir(parents=True, exist_ok=True)
        name = uuid.uuid4().hex + source.suffix.lower()
        target = assets / name
        previous = self.pads[index]
        try:
            shutil.copyfile(source, target)
            self.pads[index] = dict(name=source.stem, file=name)
            self.save()
        except Exception:
            self.pads[index] = previous
            target.unlink(missing_ok=True)
            raise
        if previous:
            (assets / previous['file']).unlink(missing_ok=True)

    def path(self, index):
        return self.root / 'sounds' / self.pads[index]['file']
