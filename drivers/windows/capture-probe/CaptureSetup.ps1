param([ValidateSet('List', 'Install', 'Remove')][string]$Action = 'List', [switch]$TestTone, [string]$Endpoint, [switch]$EmbeddedDll)

$ErrorActionPreference = 'Stop'
if (![Environment]::Is64BitProcess) { throw 'Use 64-bit Windows PowerShell.' }
$clsid = '{7B1C8C67-ED98-4672-87C4-A501CB3B08C1}'
$root = 'SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture'
$idProperty = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},7'
$multiProperty = '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},15'
$legacyProperties = @('{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},1', '{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},2')
$modeProperty = '{d3993a3f-99c2-4402-b5ec-a92a0367664b},7'
$disableProperty = '{1da5d803-d492-4edd-8c23-e0c0ffee7f0e},5'
$reservedProperties = @($idProperty, $multiProperty, $modeProperty) + $legacyProperties
$modes = [string[]]@('{C18E2F7E-933D-4965-B7D1-1EEF228D2AF3}', '{98951333-B9CD-48B1-A0A3-FF40682D73F7}')
$hklm = [Microsoft.Win32.Registry]::LocalMachine
$feed = Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'DontAbuseSounds Capture Feed'

if ($Endpoint) {
    if ($Endpoint -notmatch '^\{[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}\}$') {
        throw 'Invalid Windows recording endpoint.'
    }
    $endpointKey = $hklm.OpenSubKey("$root\$Endpoint")
    if (!$endpointKey) { throw 'Selected Windows recording endpoint is unavailable.' }
    try {
        if (($endpointKey.GetValue('DeviceState', 0) -band 1) -eq 0) {
            throw 'Enable and reconnect the selected microphone.'
        }
    } finally { $endpointKey.Dispose() }
    $device = [pscustomobject]@{ Name = 'selected microphone'; Path = "$root\$Endpoint\FxProperties" }
} else {
$devices = @(
    foreach ($dev in Get-PnpDevice -Class AudioEndpoint -PresentOnly -ErrorAction SilentlyContinue) {
        if ($dev.Status -ne 'OK' -or $dev.InstanceId -notmatch '^SWD\\MMDEVAPI\\\{0\.0\.1\.00000000\}\.\{([0-9a-f-]{36})\}$') { continue }
        $path = "$root\{$($Matches[1])}\FxProperties"
        $key = $hklm.OpenSubKey($path)
        try {
            $state = if (!$key) { 'No effects store' }
                elseif ($key.GetValue($idProperty) -eq $clsid) { 'DontAbuseSounds test attached' }
                elseif (($key.GetValueNames() | Where-Object { $_ -in $reservedProperties })) { 'Existing endpoint effect' }
                elseif ($key.GetValue($disableProperty, 0) -ne 0) { 'Audio enhancements disabled' }
                else { 'Candidate' }
            [pscustomobject]@{ Name = $dev.FriendlyName; State = $state; Path = $path }
        } finally {
            if ($key) { $key.Dispose() }
        }
    }
)
if (!$devices.Count) {
    Write-Output 'No active Windows recording endpoints.'
    if ($Action -eq 'List') { return }
    throw 'Connect and enable the microphone first.'
}
for ($i = 0; $i -lt $devices.Count; $i++) {
    Write-Host ("{0}. {1} [{2}]" -f ($i + 1), $devices[$i].Name, $devices[$i].State)
}
if ($Action -eq 'List') { return }

}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (!$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run Install or Remove from an administrator PowerShell.'
}
if (!$Endpoint) {
    $choice = 0
    if (![int]::TryParse((Read-Host 'Recording device number'), [ref]$choice) -or
        $choice -lt 1 -or $choice -gt $devices.Count) { throw 'Invalid selection.' }
    $device = $devices[$choice - 1]
}
$key = $hklm.OpenSubKey($device.Path, $true)
if (!$key) { throw 'Windows did not expose a writable effect store for this recording device.' }
try {
    if ($Action -eq 'Remove') {
        if ($key.GetValue($idProperty) -ne $clsid) { throw 'This endpoint has no DontAbuseSounds test effect.' }
        $key.DeleteValue($idProperty)
        if (($key.GetValue($modeProperty) -join ',') -eq ($modes -join ',')) {
            $key.DeleteValue($modeProperty)
        }
        if ((Test-Path -LiteralPath $feed) -and
            !((Get-Item -LiteralPath $feed).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            $marker = Join-Path $feed 'installed'
            if (Test-Path -LiteralPath $marker) { Remove-Item -LiteralPath $marker }
        }
        Write-Host 'Test effect detached. Reboot Windows before testing again.'
        return
    }

    # Never replace an existing effect or toggle Windows enhancements for the user.
    if (($key.GetValueNames() | Where-Object { $_ -in $reservedProperties }) -or
        $key.GetValue($disableProperty, 0) -ne 0) { throw "Windows microphone routing requires enabled audio enhancements and an available endpoint effect slot." }

    $capture = $hklm.OpenSubKey($root)
    try {
        foreach ($endpoint in $capture.GetSubKeyNames()) {
            $effects = $capture.OpenSubKey("$endpoint\FxProperties")
            if ($effects) {
                try {
                    if ($effects.GetValue($idProperty) -eq $clsid) {
                        throw 'Detach the existing DontAbuseSounds capture effect first.'
                    }
                } finally { $effects.Dispose() }
            }
        }
    } finally { $capture.Dispose() }

    $payload = if ($EmbeddedDll) {
        $encoded = [Console]::In.ReadToEnd()
        if (!$encoded -or $encoded.Length -gt 1500000) { throw 'Invalid embedded audio effect.' }
        [Convert]::FromBase64String($encoded)
    } else {
        $source = Join-Path $PSScriptRoot 'DontAbuseSoundsCaptureProbe.dll'
        [IO.File]::ReadAllBytes($source)
    }
    if ($payload.Length -lt 2 -or $payload[0] -ne 77 -or $payload[1] -ne 90) { throw 'Invalid audio effect DLL.' }
    $directory = Join-Path ([Environment]::GetFolderPath('ProgramFiles')) 'DontAbuseSounds Capture Probe'
    if ((Test-Path -LiteralPath $directory) -and
        ((Get-Item -LiteralPath $directory).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'The probe installation folder is a reparse point.'
    }
    $null = New-Item -ItemType Directory -Path $directory -Force
    $dll = Join-Path $directory 'DontAbuseSoundsCaptureProbe.dll'
    if (Test-Path -LiteralPath $dll) {
        $sha = [Security.Cryptography.SHA256]::Create()
        try { $expected = [BitConverter]::ToString($sha.ComputeHash([byte[]]$payload)).Replace('-', '') }
        finally { $sha.Dispose() }
        if ($expected -ne (Get-FileHash -LiteralPath $dll).Hash) {
            throw 'A different probe build is already installed. Detach it and remove that build first.'
        }
    } else {
        [IO.File]::WriteAllBytes($dll, $payload)
    }
    $com = $hklm.CreateSubKey("SOFTWARE\Classes\CLSID\$clsid\InprocServer32")
    try {
        $com.SetValue('', $dll)
        $com.SetValue('ThreadingModel', 'Both')
    } finally { $com.Dispose() }
    $apo = $hklm.CreateSubKey("SOFTWARE\Classes\AudioEngine\AudioProcessingObjects\$clsid")
    try {
        $apo.SetValue('FriendlyName', 'DontAbuseSounds Capture Probe')
        $apo.SetValue('Copyright', 'DontAbuseSounds contributors')
        $apo.SetValue('APOInterface0', '{FD7F2B29-24D0-4B5C-B177-592C39F9CA10}')
        foreach ($property in 'MajorVersion', 'MinInputConnections', 'MaxInputConnections', 'MinOutputConnections', 'MaxOutputConnections', 'NumAPOInterfaces') {
            $apo.SetValue($property, 1, 'DWord')
        }
        $apo.SetValue('MinorVersion', 0, 'DWord')
        $apo.SetValue('Flags', 14, 'DWord')
        $apo.SetValue('MaxInstances', -1, 'DWord')
    } finally { $apo.Dispose() }

    if (!$TestTone) {
        if (Test-Path -LiteralPath $feed) {
            throw 'The capture feed directory already exists. Remove the old test feed before reinstalling.'
        }
        $null = New-Item -ItemType Directory -Path $feed
        $acl = Get-Acl -LiteralPath $feed
        $acl.SetAccessRuleProtection($true, $false)
        $admins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
        $system = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
        $audio = [Security.Principal.SecurityIdentifier]::new('S-1-5-19')
        $acl.SetOwner($admins)
        foreach ($sid in $admins, $system) {
            $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($sid, 'FullControl', 'ObjectInherit,ContainerInherit', 'None', 'Allow'))
        }
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($audio, 'ReadAndExecute', 'ObjectInherit', 'None', 'Allow'))
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($identity.User, 'ReadAndExecute,CreateFiles', 'Allow'))
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($identity.User, 'Modify', 'ObjectInherit', 'InheritOnly', 'Allow'))
        Set-Acl -LiteralPath $feed -AclObject $acl
    } elseif (Test-Path -LiteralPath $feed) {
        throw 'Remove the old test feed before installing in test tone mode.'
    }

    $key.SetValue($modeProperty, $modes, 'MultiString')
    try {
        $key.SetValue($idProperty, $clsid, 'String')
        if (!$TestTone) { $null = New-Item -ItemType File -Path (Join-Path $feed 'installed') }
    } catch {
        if ($key.GetValue($idProperty) -eq $clsid) { $key.DeleteValue($idProperty) }
        $key.DeleteValue($modeProperty, $false)
        throw
    }
    Write-Host "Test effect attached to $($device.Name). Reboot Windows and open Discord's mic test."
} finally { $key.Dispose() }
