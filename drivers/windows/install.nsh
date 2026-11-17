!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\driver\device.exe" "$INSTDIR\driver\SimpleAudioSample.inf"' $0
  ${If} $0 == 3010
    SetRebootFlag true
  ${ElseIf} $0 != 0
    SetErrorLevel $0
    IfSilent +2
    MessageBox MB_ICONSTOP "Could not install DontAbuseSounds Mic (Windows error $0)."
    Abort "Could not install the virtual microphone"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  IfFileExists "$INSTDIR\driver\device.exe" 0 +2
  ExecWait '"$INSTDIR\driver\device.exe" --remove' $0
!macroend
