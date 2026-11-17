param(
    [Parameter(Mandatory)][string]$DriverDir,
    [Parameter(Mandatory)][string]$OutputDir,
    [Parameter(Mandatory)][string]$CertificateThumbprint
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot/../..").Path
$destination = New-Item -ItemType Directory -Force $OutputDir
$work = New-Item -ItemType Directory -Force "$repo/desktop/target/driver-install"
$kits = "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.26100.0\x64"
foreach ($file in 'SimpleAudioSample.inf', 'SimpleAudioSample.sys', 'SimpleAudioSample.cat') {
    Copy-Item "$DriverDir/$file" "$work/"
}
Copy-Item "$PSScriptRoot/LICENSE.txt" "$work/"
$timestamp = 'http://timestamp.digicert.com'
$signtool = "$kits/signtool.exe"
foreach ($file in 'SimpleAudioSample.sys', 'SimpleAudioSample.inf') {
    $policy = if ($file -like '*.sys') { '/kp' } else { '/pa' }
    & $signtool verify $policy /c "$work/SimpleAudioSample.cat" "$work/$file"
    if ($LASTEXITCODE) { throw "Windows kernel trust verification failed for $file" }
}
cl /nologo /std:c17 /W4 /O2 /MT "$PSScriptRoot/device.c" "/Fo$work/device.obj" "/Fe$work/device.exe" setupapi.lib newdev.lib
if ($LASTEXITCODE) { throw 'Device installation helper build failed' }
& $signtool sign /sha1 $CertificateThumbprint /fd SHA256 /td SHA256 /tr $timestamp "$work/device.exe"
if ($LASTEXITCODE) { throw 'Device installation helper signing failed' }
$config = @{
    bundle = @{
        resources = @{ 'target/driver-install/' = 'driver/' }
        windows = @{
            certificateThumbprint = $CertificateThumbprint
            digestAlgorithm = 'sha256'
            timestampUrl = $timestamp
            tsp = $true
            nsis = @{
                installMode = 'perMachine'
                installerHooks = '../drivers/windows/install.nsh'
            }
        }
    }
} | ConvertTo-Json -Depth 7
Push-Location "$repo/desktop"
try {
    Set-Content target/driver-bundle.json $config -Encoding utf8
    npx --yes '@tauri-apps/cli@2.12.5' build --bundles nsis --config target/driver-bundle.json -- --locked
    if ($LASTEXITCODE) { throw 'Windows installer build failed' }
    $installer = Get-ChildItem target/release/bundle/nsis -Filter '*-setup.exe' | Select-Object -First 1
    if (!$installer) { throw 'Missing Windows installer' }
    foreach ($file in 'target/release/dont-abuse-sounds.exe', $installer.FullName, "$work/device.exe") {
        & $signtool verify /pa /all /tw $file
        if ($LASTEXITCODE -ne 0) { throw "Authenticode verification failed for $file" }
    }
    Copy-Item $installer.FullName "$destination/DontAbuseSounds-Windows-x64-setup.exe"
} finally {
    Pop-Location
}
