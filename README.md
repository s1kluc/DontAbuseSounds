# Virtual Mixer

Native Qt desktop soundboard with a shared PortAudio core: CoreAudio on macOS,
Windows audio hosts, and ALSA/PulseAudio on Linux. macOS is the initial target.

## Run

Install Python 3.11–3.14, then:

```sh
cd /Users/lucaslipis/.codex/virtual-mixer
python3.13 -m venv .venv
.venv/bin/python -m pip install -e .
.venv/bin/virtual-mixer
```

Alternatively use `uv venv --python 3.13` and `uv pip install -e .`.
Windows uses `.venv\Scripts\virtual-mixer.exe`. Linux needs the distribution's
PortAudio runtime, for example `libportaudio2`. Python 3.15 is not supported yet.

Select an output, assign audio to a pad, and click to play or stop. Pads play
simultaneously; Stop all silences all voices. Audio is converted to the device's
sample rate and channel count. WAV, FLAC, AIFF, OGG and MP3 availability depends on
libsndfile codecs. Imports are copied into app data and limited to five minutes,
mono/stereo, and 30 million samples. Decode happens on the UI thread; use short
clips. Simultaneous loud sounds may clip. Input selection is a saved reference;
microphone capture, monitoring, hotkeys and MIDI are future work.

Qt AppDataLocation stores project.json and sounds, normally
`~/Library/Application Support/VirtualMixer/VirtualMixer` on macOS. Saves replace
JSON atomically. A corrupt project disables saving to preserve the original.
Run one instance per project folder. Device IDs use platform, host and name;
identically named devices use enumeration order and may be ambiguous after reordering.
Virtual detection is a name hint. Custom virtual and Multi-Output devices can be selected.
No output is automatically selected; losing or changing the output stops playback.
Device scans run every 2.5 seconds. No deliberate fallback to system speakers occurs.

## Setup

The app includes platform instructions and does not install drivers.

- macOS: install [BlackHole](https://existential.audio/blackhole/). Select it as
  output here and as microphone/input in Discord/calls or Audio Input Capture in OBS.
  For monitoring, create a Multi-Output Device with headphones and BlackHole in
  Audio MIDI Setup, with drift correction on the secondary device.
- Windows: install [VB-CABLE](https://vb-audio.com/Cable/), output to CABLE Input,
  receive from CABLE Output.
- Linux: with PulseAudio or PipeWire's PulseAudio compatibility, run
  `pactl load-module module-null-sink sink_name=virtual_mixer sink_properties=device.description=VirtualMixer`.
  Output to VirtualMixer and use pavucontrol to capture its monitor in the receiver.
  This sink is temporary; configure the audio service for persistence.

## Checks

```sh
python3 -m unittest discover -s tests -v
```

Tests simulate device enumeration for CoreAudio/WASAPI/ALSA, stable IDs across
index changes, disconnect cleanup, settings restoration and unsafe-path rejection.

Before releasing on **each** platform, manually:

1. Import mono/stereo clips at differing sample rates, restart and verify saved pads.
2. Receive pad audio in OBS/Discord through the virtual input; verify stop and Stop all.
3. Play simultaneous pads; switch outputs during playback and verify old output silence.
4. Disconnect the selected device, verify unavailable status and silence, reconnect,
   reselect and replay. Verify no speaker fallback.
5. Try unsupported audio, missing assets, missing virtual devices and corrupt JSON.

Hardware reception, macOS GUI execution and Windows/Linux parity have not been
verified in this environment. Windows signing requires the certificate setup below.
The source is an MVP implementation, not a hardware-certified release.

API references: [sounddevice](https://python-sounddevice.readthedocs.io/),
[Qt for Python](https://doc.qt.io/qtforpython-6/).

## Windows and Linux executables

The supplied photo is used for the window icon, Windows/macOS executable icon, and Linux
application menu icon. Its aspect ratio is preserved with transparent padding.

Build **on the target operating system**, using Python 3.13:

```sh
python -m pip install -e ".[build]"
python scripts/build.py
```

On Ubuntu, first install `libportaudio2 libsndfile1 libegl1 libxkbcommon-x11-0
libxcb-cursor0 libxcb-icccm4 libxcb-keysyms1 libxcb-shape0 libxcb-xinerama0` using apt.
The build includes Python, Qt and audio modules:

- Windows: `dist/VirtualMixer/VirtualMixer.exe`, plus a ZIP package in `dist/`.
  Extract the entire ZIP and keep the `_internal` folder beside the executable.
- Linux: `dist/VirtualMixer`, plus a tar.gz package in `dist/`. After extraction,
  run `./VirtualMixer`; optionally run `./install-desktop.sh` to install the app and
  photo icon into your user application menu. Linux ELF executables do not embed a
  file-manager icon in the same way as Windows EXE files.

The packaged app does not require Python on the recipient's computer. It still
needs a working graphical desktop/audio service and an installed virtual audio device.
Linux builds made on Ubuntu 22.04 target that release or newer compatible systems;
this does not imply support for every Linux distribution. Architecture follows the
build host; the included CI configuration targets x86-64.

`.github/workflows/build.yml` builds all three systems on pushes, pull requests, or manual triggers, and retains packages as GitHub Actions artifacts. Copy this
project into a GitHub repository first. Pushing a `v*` tag publishes the Windows
ZIP and macOS ZIP/DMG as a GitHub release after both builds pass. Branch
builds continue to check all three platforms.

These build recipes have not yet produced or validated native Windows/Linux/macOS binaries.
No such binaries are included in this source folder. Test audio reception on each
OS using the checklist above before distribution. Windows packages are unsigned
unless signing is configured below.

### Windows signing

SoXR replaces SciPy for sample-rate conversion, removing the SciPy import that
loaded the blocked `_pocketfft_umath` module in issue #1. Other native modules
can still be blocked by Windows application control when unsigned or untrusted.

For local signing, install the Windows SDK and a trusted RSA code-signing
certificate with its private key in `CurrentUser\My`. Set the environment variable
`WINDOWS_CERTIFICATE_THUMBPRINT` to its thumbprint before running `scripts/build.py`.
The build preserves valid vendor signatures and signs remaining EXE, DLL and PYD
files with SHA-256 and an RFC 3161 timestamp. It verifies every native binary before
creating the ZIP; signing or verification failures stop the build.

For GitHub Actions, configure repository secrets `WINDOWS_CERTIFICATE_BASE64`
(base64-encoded PFX containing the certificate and private key) and
`WINDOWS_CERTIFICATE_PASSWORD`. Use a certificate issued by a trusted CA; a
self-signed certificate does not establish public trust. PFX import is supported
only if your certificate provider permits export of the signing key; hardware or
cloud-backed keys require that provider's signing integration instead.
Without these secrets, CI explicitly builds an unsigned test package. Pull requests
never import the certificate. Test with Windows application control enabled;
signing cannot override an organization's explicit deny policies.

## GitHub builds for DontAbuseSounds

Target repository: https://github.com/s1kluc/DontAbuseSounds

The **Desktop builds** workflow runs on pushes and pull requests, and can also be
started with **Actions → Desktop builds → Run workflow** after it is on the default
branch. It builds Windows, Ubuntu 22.04 and macOS 15 using native runners. Download
completed packages from the workflow run's **Artifacts** section. Push a `v*` tag
to publish a Windows and macOS release automatically. Each artifact name includes the runner architecture;
binaries support that architecture, not every CPU type.

macOS produces a ZIP and a DMG containing `VirtualMixer.app` with the supplied photo icon.
Open the DMG and drag the app to the Applications link to install it. The filename
includes the build architecture; the app is not a universal Intel/Apple Silicon binary.
The build verifies the DMG checksum before uploading it.
The app is unsigned and not notarized; Gatekeeper may block opening it until the
user allows it in System Settings → Privacy & Security. Windows builds use the
optional signing configuration above and are otherwise unsigned.

You can also run `python scripts/build.py` locally on macOS after installing the
`build` extra. Native Windows/Linux/macOS build results still need verification;
a successful CI build alone does not verify audio routing on real devices.
