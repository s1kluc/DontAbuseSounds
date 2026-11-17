# Windows capture endpoint probe

This diagnostic APO adds a quiet half-second test tone when the Windows audio engine opens an opted-in capture pipeline and otherwise passes its samples through. The tone is the first check for whether the physical Yeti X capture endpoint can feed both microphone and generated audio to Discord under its existing device identity. It runs separately from the desktop app.

Windows CI checks the signal mix and loads the COM DLL inside a test process. That test does not register the effect or attach it to a device. The hardware proof needs a Windows 11 PC with the Yeti X. Use its audio device hardware ID and driver service to target a signed extension INF and APO package. Verify the capture interface association, preserve any existing effects, and run the first test with Soundpad disabled for this device to isolate the result.

The hardware test passes when Discord still uses the existing Yeti X capture input, receives the tone and voice, and continues to receive voice alone after the tone. Package installation and removal must preserve the original device and its effects. Once that succeeds, replace the diagnostic tone with decoded clip samples from the existing Rust app.

References: [Windows APO implementation](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/implementing-audio-processing-objects) and [APO deployment](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/deploying-audio-processing-objects).
