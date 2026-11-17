# DontAbuseSounds

Desktop soundboard with twelve pads, a microphone mix, and its own virtual audio device. The Tauri app and mixer are written in Rust. The Windows driver and macOS HAL plug-in are installed alongside the app.

## Setup

1. On Windows 11 x64, run the setup EXE as an administrator. On macOS 12 or later, open the universal DMG, run the installer package, and restart so CoreAudio loads the plug-in.
2. Open DontAbuseSounds, choose your **hardware microphone**, then select **DontAbuseSounds Feed** on Windows or **DontAbuseSounds Mic** on macOS as the soundboard output. Click **Start audio**.
3. In Discord's Voice & Video settings, choose **DontAbuseSounds Mic** as the input and your usual headphones as the output. Keep DontAbuseSounds running during the call.
4. Assign WAV, MP3, FLAC, AIFF, or Ogg/Vorbis files to the pads. Click a pad to play or stop it. **Stop all sounds** stops the pads while keeping your microphone active.

For local playback, turn the microphone off in the soundboard and choose your headphones or speakers as the soundboard output. If Discord suppresses clips during a call, check its microphone noise suppression and input sensitivity settings.

Use **Import project** before assigning sounds to migrate a legacy Virtual Mixer `project.json`. Keep the old `sounds` folder next to it. The import copies the clips into the new app's data directory, and the audio devices are selected again in the new app.

## Building and testing

Install stable Rust and Node.js 24. From `desktop/`:

```sh
cargo test --locked
npx --yes @tauri-apps/cli@2.12.5 dev
```

The **Desktop builds** workflow produces Windows development artifacts (the EXE and WDK driver files for signing) and a macOS development DMG. The macOS development package uses local ad hoc code signatures. These artifacts are for development and device testing.

The Windows driver is based on Microsoft's [SimpleAudioSample](https://github.com/microsoft/Windows-driver-samples/tree/main/audio/simpleaudiosample) (MS-PL). The macOS plug-in is based on Apple's [NullAudio sample](https://developer.apple.com/documentation/coreaudio/creating-an-audio-server-driver-plug-in). Their license texts are under `drivers/`.

Before publishing, verify on clean Windows 11 x64 and macOS systems that the installer registers the device and Discord can record voice alone, a clip alone and both together. Check that Stop all leaves voice working and that stopping audio or unplugging the microphone stops the route. Include an upgrade and uninstall on Windows and both Mac architectures in the release check. On macOS the plug-in can be removed from `/Library/Audio/Plug-Ins/HAL/DontAbuseSoundsMic.driver` by an administrator, followed by a restart.

## Publishing

- **Windows:** Submit the driver from the matching development workflow to Microsoft for production driver signing. Put the returned `SimpleAudioSample.inf`, `.sys`, and `.cat` at the root of a ZIP. Point repository variables `DAS_WINDOWS_DRIVER_URL` and `DAS_WINDOWS_DRIVER_SHA256` at the HTTPS download and SHA-256 hash. CI validates the ZIP and driver signatures. For the application, installer and installer helper, configure GitHub Actions secrets `WINDOWS_CERTIFICATE_BASE64` (base64 PFX with a trusted Authenticode signing identity) and `WINDOWS_CERTIFICATE_PASSWORD`. A hardware- or cloud-backed signing key needs its provider’s signing integration. CI verifies the timestamped Authenticode signatures before uploading the installer.
- **macOS:** Export the Developer ID Application and Developer ID Installer identities with their private keys together as a password-protected P12. Set secrets `APPLE_CERTIFICATE` (base64 P12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, and `APPLE_PASSWORD` (app-specific password). Set repository variables `APPLE_SIGNING_IDENTITY`, `APPLE_INSTALLER_IDENTITY`, and `APPLE_TEAM_ID`.
- Push a new `v*` tag on the reviewed Rust commit after configuring signing and testing the installers. GitHub Actions publishes the Windows setup EXE and the notarized universal DMG together after both jobs pass.
