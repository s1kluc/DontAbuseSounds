# Windows hardware microphone routing preview

Windows 11 x64. This test build adds clips to the selected existing recording endpoint. Keep your normal hardware microphone selected in Discord.

1. Extract the ZIP. In 64-bit Windows PowerShell, run `./CaptureSetup.ps1 -Action List` to check the detected recording devices.
2. Using the same Windows account, open 64-bit PowerShell as administrator. Run `./CaptureSetup.ps1 -Action Install` and choose the microphone by number. Reboot Windows.
3. Run the bundled DontAbuseSounds EXE. Under Soundboard output choose **Windows microphone (installed capture effect)**, then click **Start audio**. Assign a clip and trigger the pad. Keep the same hardware microphone selected in Discord.
4. To detach the effect, close DontAbuseSounds, run `./CaptureSetup.ps1 -Action Remove` as administrator, select the same device, and reboot. The unused DLL and feed directory remain on disk. For a clean reinstall after detaching, remove the two test folders **DontAbuseSounds Capture Probe** in Program Files and **DontAbuseSounds Capture Feed** in ProgramData as administrator.

The test accepts one active endpoint at a time, clips up to one minute, and plays one pad at a time in the capture route. Local playback remains available through the other outputs. Capture attachment stops if the endpoint effects store is absent, read-only, disabled or already contains an endpoint effect. The preview never replaces an existing endpoint effect. Audio policy or driver behavior may prevent loading the unsigned DLL or bypass effects for an application's recording mode. Run the first test with Soundpad disabled for the selected endpoint.

If you only want to check for a brief tone at recording start, use `-Action Install -TestTone` on a clean setup instead. The effect opens the audio feed in clip mode and keeps file reads out of the audio callback. The CLI does not report that Windows loaded the effect. Verify actual audio in Discord or a recording from the same hardware microphone.

References: [APO implementation](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/implementing-audio-processing-objects) and [APO deployment](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/deploying-audio-processing-objects).
