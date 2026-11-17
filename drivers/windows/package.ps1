param(
    [Parameter(Mandatory)][string]$DriverDir,
    [Parameter(Mandatory)][string]$OutputDir
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
foreach ($file in 'SimpleAudioSample.sys', 'SimpleAudioSample.inf') {
    $policy = if ($file -like '*.sys') { '/kp' } else { '/pa' }
    & "$kits/signtool.exe" verify $policy /c "$work/SimpleAudioSample.cat" "$work/$file"
    if ($LASTEXITCODE) { throw "Windows kernel trust verification failed for $file" }
}
cl /nologo /std:c17 /W4 /O2 /MT "$PSScriptRoot/device.c" "/Fo$work/device.obj" "/Fe$work/device.exe" setupapi.lib newdev.lib
if ($LASTEXITCODE) { throw 'Device installation helper build failed' }
$config = @{
    bundle = @{
        resources = @{ 'target/driver-install/' = 'driver/' }
        windows = @{ nsis = @{
            installMode = 'perMachine'
            installerHooks = '../drivers/windows/install.nsh'
        } }
    }
} | ConvertTo-Json -Depth 7
Push-Location "$repo/desktop"
try {
    Set-Content target/driver-bundle.json $config -Encoding utf8
    npx --yes '@tauri-apps/cli@2.12.5' build --bundles nsis --config target/driver-bundle.json -- --locked
    if ($LASTEXITCODE) { throw 'Windows installer build failed' }
    $installer = Get-ChildItem target/release/bundle/nsis -Filter '*-setup.exe' | Select-Object -First 1
    if (!$installer) { throw 'Missing Windows installer' }
    Copy-Item $installer.FullName "$destination/DontAbuseSounds-Windows-x64-setup.exe"
} finally {
    Pop-Location
}
