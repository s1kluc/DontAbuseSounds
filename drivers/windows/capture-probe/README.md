# Windows capture endpoint probe

This diagnostic APO adds a quiet half-second test tone when the Windows audio engine opens an opted-in capture pipeline and otherwise passes its samples through. The tone is the first check for whether the physical Yeti X capture endpoint can feed both microphone and generated audio to Discord under its existing device identity. It runs separately from the desktop app.

Extract the Windows x64 probe ZIP. From 64-bit Windows PowerShell in the extracted folder, run `./CaptureSetup.ps1 -Action List` to see the detected recording endpoints. No hardware ID is needed. For an experimental attachment, run `./CaptureSetup.ps1 -Action Install` from an administrator PowerShell and choose the microphone by number. Reboot Windows, keep that same hardware microphone selected in Discord, and start Discord's mic test. For removal, use `./CaptureSetup.ps1 -Action Remove` and reboot again. Remove detaches the selected endpoint. The inactive DLL and its registration remain installed and can be reused.

The device attachment uses the endpoint's existing effects store and accepts a free endpoint-effect slot only. It will stop if the store is missing, protected by Windows permissions, disabled or already occupied. Audio policies or the driver may prevent Windows from loading this unsigned experimental DLL. In that case the device-specific signed package is the production deployment path.

Windows CI checks the signal mix and loads the COM DLL inside a test process and checks that the Windows audio callback mixes a tone into a silent capture buffer. That test does not register the effect or attach it to a device. The hardware proof needs a Windows 11 PC and a physical microphone. Run the first test with Soundpad disabled for the selected device to isolate the result.

The hardware test passes when Discord still uses the existing Yeti X capture input, receives the tone and voice, and continues to receive voice alone after the tone. Package installation and removal must preserve the original device and its effects. Once that succeeds, replace the diagnostic tone with decoded clip samples from the existing Rust app.

References: [Windows APO implementation](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/implementing-audio-processing-objects) and [APO deployment](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/deploying-audio-processing-objects).
