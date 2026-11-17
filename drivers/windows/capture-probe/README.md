# Windows hardware microphone routing preview

Windows 11 x64. This test build adds clips to the selected existing recording endpoint. Keep your normal hardware microphone selected in Discord.

1. Launch `DontAbuseSounds-Microphone-Test.exe`.
2. Select your hardware microphone and click **Set up microphone routing**. Approve the Windows administrator prompt. Use the same Windows account to configure and run DontAbuseSounds.
3. The app selects the installed capture output. Reopen Discord’s mic test with the same hardware microphone selected. If Windows asks for a restart, restart and open DontAbuseSounds again before testing. Assign and trigger a pad.
4. For manual diagnostics and removal, `CaptureSetup.ps1 -Action List` enumerates endpoints and `CaptureSetup.ps1 -Action Remove` detaches the effect from the selected endpoint. Restart Windows after detaching.

The test accepts one active endpoint at a time, clips up to one minute, and plays one pad at a time in the capture route. Local playback remains available through the other outputs. Capture attachment stops if the endpoint effects store is absent, read-only, disabled or already contains an endpoint effect. The preview never replaces an existing endpoint effect. Audio policy or driver behavior may prevent loading the unsigned DLL or bypass effects for an application's recording mode. Run the first test with Soundpad disabled for the selected endpoint.

If you only want to check for a brief tone at recording start, use `-Action Install -TestTone` on a clean setup instead. The effect opens the audio feed in clip mode and keeps file reads out of the audio callback. The CLI does not report that Windows loaded the effect. Verify actual audio in Discord or a recording from the same hardware microphone.

References: [APO implementation](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/implementing-audio-processing-objects) and [APO deployment](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/deploying-audio-processing-objects).
