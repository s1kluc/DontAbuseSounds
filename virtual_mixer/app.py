import sys
from pathlib import Path
from PySide6.QtGui import QIcon
from PySide6.QtCore import QStandardPaths, QTimer
from PySide6.QtWidgets import (QApplication, QComboBox, QFileDialog, QGridLayout,
    QGroupBox, QHBoxLayout, QLabel, QMainWindow, QMessageBox, QPushButton,
    QVBoxLayout, QWidget)
from .core import AudioCore, PortAudioAdapter, Project


GUIDES = {
    'darwin': 'macOS: Install BlackHole from https://existential.audio/blackhole/. '
              'Restart this app after installation. Select BlackHole as the output here and '
              'as the microphone/input in Discord or your call app. In OBS add Audio Input '
              'Capture and select BlackHole. For local monitoring, create a Multi-Output '
              'Device in Audio MIDI Setup with BlackHole and your headphones, enable drift '
              'correction for the secondary device, and select that output here.',
    'win32': 'Windows: Install VB-CABLE from https://vb-audio.com/Cable/ and follow its '
             'restart instructions. Select CABLE Input as this app’s output and CABLE '
             'Output as the microphone in Discord/calls or Audio Input Capture in OBS.',
    'linux': 'Linux: With PulseAudio or PipeWire’s PulseAudio compatibility service, run '
             'pactl load-module module-null-sink sink_name=virtual_mixer '
             'sink_properties=device.description=VirtualMixer. Select VirtualMixer here. '
             'Use pavucontrol to route the receiving app’s recording stream to Monitor of '
             'VirtualMixer. The sink is temporary; configure your audio service for persistence.'
}


class Window(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle('Virtual Mixer — Soundboard')
        self.resize(880, 670)
        self.project = Project(QStandardPaths.writableLocation(QStandardPaths.AppDataLocation))
        self.adapter = PortAudioAdapter()
        self.core = AudioCore(self.adapter)
        self.devices = []
        self.readonly = False
        load_error = None
        try:
            self.project.load()
        except Exception as error:
            self.readonly = True
            load_error = str(error)
        root = QWidget()
        layout = QVBoxLayout(root)
        title = QLabel('Virtual Mixer')
        title.setStyleSheet('font-size: 28px; font-weight: bold')
        layout.addWidget(title)
        layout.addWidget(QLabel('Soundboard → selected output → OBS / Discord / calls'))
        devices = QGroupBox('Audio devices')
        form = QVBoxLayout(devices)
        self.input = QComboBox()
        self.output = QComboBox()
        form.addWidget(QLabel('Input reference (microphone mixing is a later release)'))
        form.addWidget(self.input)
        form.addWidget(QLabel('Soundboard output'))
        form.addWidget(self.output)
        row = QHBoxLayout()
        refresh = QPushButton('Refresh devices')
        refresh.clicked.connect(self.refresh)
        guide = QPushButton('Virtual device setup')
        guide.clicked.connect(self.guide)
        row.addWidget(refresh)
        row.addWidget(guide)
        form.addLayout(row)
        self.device_status = QLabel()
        self.device_status.setWordWrap(True)
        form.addWidget(self.device_status)
        layout.addWidget(devices)
        grid = QGridLayout()
        self.buttons = []
        for i in range(12):
            box = QVBoxLayout()
            button = QPushButton()
            button.setMinimumHeight(78)
            button.clicked.connect(lambda checked=False, n=i: self.toggle(n))
            assign = QPushButton('Assign audio…')
            assign.setEnabled(not self.readonly)
            assign.clicked.connect(lambda checked=False, n=i: self.assign(n))
            box.addWidget(button)
            box.addWidget(assign)
            grid.addLayout(box, i // 4, i % 4)
            self.buttons.append(button)
        layout.addLayout(grid)
        stop = QPushButton('Stop all sounds')
        stop.clicked.connect(lambda: self.core.stop())
        layout.addWidget(stop)
        self.message = QLabel('Ready. Assign audio to a pad to begin.')
        self.message.setWordWrap(True)
        layout.addWidget(self.message)
        self.setCentralWidget(root)
        self.input.currentIndexChanged.connect(self.choose_input)
        self.output.currentIndexChanged.connect(self.choose_output)
        self.refresh()
        self.timer = QTimer(self)
        self.timer.timeout.connect(self.tick)
        self.timer.start(150)
        self.device_timer = QTimer(self)
        self.device_timer.timeout.connect(self.refresh)
        self.device_timer.start(2500)
        if load_error:
            QTimer.singleShot(0, lambda: self.fail('Project could not be loaded. Saving disabled to preserve it. ' + load_error))

    def fail(self, message):
        QMessageBox.warning(self, 'Virtual Mixer', str(message))

    def save(self):
        if not self.readonly:
            try:
                self.project.save()
            except Exception as error:
                self.fail(error)

    def refresh(self):
        try:
            devices = self.adapter.devices()
        except Exception as error:
            devices = []
            self.message.setText('Device scan failed: ' + str(error))
        self.devices = devices
        for combo, selected, direction in ((self.input, self.project.input_id, 'inputs'),
                                            (self.output, self.project.output_id, 'outputs')):
            combo.blockSignals(True)
            combo.clear()
            combo.addItem('Select a device…', None)
            for device in devices:
                if getattr(device, direction) > 0:
                    combo.addItem(device.name + (' [virtual]' if device.virtual else ''), device.id)
            index = combo.findData(selected) if selected else 0
            if index < 0:
                combo.addItem('Disconnected — select another device', selected)
                index = combo.count() - 1
            combo.setCurrentIndex(index)
            combo.blockSignals(False)
        current = next((d for d in devices if d.id == self.project.output_id and d.outputs), None)
        if current != self.core.output:
            try:
                self.core.select_output(current)
            except Exception as error:
                self.message.setText('Output stopped: ' + str(error))
        if current is None:
            self.device_status.setText('Output unavailable. Select a device or open virtual device setup. Playback is stopped.')
        elif current.virtual:
            self.device_status.setText('Virtual output ready. Select its matching input in the receiving application.')
        else:
            self.device_status.setText('Output connected. Virtual routing is unverified for this device; use setup guidance if needed.')

    def choose_input(self):
        self.project.input_id = self.input.currentData()
        self.save()

    def choose_output(self):
        self.project.output_id = self.output.currentData()
        self.save()
        self.refresh()

    def guide(self):
        QMessageBox.information(self, 'Virtual audio setup', GUIDES.get(sys.platform, GUIDES['linux']) +
            '\n\nDevice labels marked virtual are detected by name. Custom virtual devices can also be selected. '
            'This app does not install drivers. Test reception in your receiving app before broadcasting.')

    def assign(self, index):
        path, _ = QFileDialog.getOpenFileName(self, 'Assign sound', '',
            'Audio (*.wav *.flac *.ogg *.aiff *.aif *.mp3);;All files (*)')
        if path:
            try:
                self.core.stop(index)
                self.project.import_pad(index, path)
                self.tick()
            except Exception as error:
                self.fail(error)

    def toggle(self, index):
        if index in self.core.playing():
            self.core.stop(index)
        elif self.project.pads[index] is None:
            if not self.readonly:
                self.assign(index)
        else:
            try:
                self.core.play(index, self.project.path(index))
                self.message.setText('Playing: ' + self.project.pads[index]['name'])
            except Exception as error:
                self.core.close()
                self.fail(error)
        self.tick()

    def tick(self):
        if self.core.stream is not None and not self.core.stream.active:
            self.core.close()
            self.message.setText('Audio device stopped. Refresh or reselect the output, then play again.')
        playing = self.core.playing()
        for i, button in enumerate(self.buttons):
            pad = self.project.pads[i]
            missing = pad is not None and not self.project.path(i).exists()
            button.setText(f'{i + 1:02d}  ' + ('Stop ■\n' if i in playing else 'Play ▶\n') +
                           (pad['name'] if pad else 'Empty pad') + ('\nFile missing — reassign' if missing else ''))
            button.setStyleSheet('background: #246b50; color: white' if i in playing else '')
        if self.core.status:
            self.message.setText('Audio status: ' + self.core.status)
            self.core.status = ''

    def closeEvent(self, event):
        self.core.close()
        event.accept()


def main():
    app = QApplication(sys.argv)
    app.setOrganizationName('VirtualMixer')
    app.setApplicationName('VirtualMixer')
    app.setWindowIcon(QIcon(str(Path(__file__).parent / 'assets' / 'icon.png')))
    window = Window()
    window.show()
    sys.exit(app.exec())


if __name__ == '__main__':
    main()
