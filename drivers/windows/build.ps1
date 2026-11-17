param([Parameter(Mandatory)][string]$OutputDir)
$ErrorActionPreference = 'Stop'
$sampleCommit = 'd49bb7d72f3651952b2b6e4ad28d14fe2247eb69'
$work = Join-Path $env:RUNNER_TEMP "das-wdk-${PID}"
New-Item -ItemType Directory -Force $work, $OutputDir | Out-Null
$zip = Join-Path $work 'upstream.zip'
Invoke-WebRequest "https://codeload.github.com/microsoft/Windows-driver-samples/zip/$sampleCommit" -OutFile $zip
Expand-Archive $zip $work
$sample = Join-Path $work "Windows-driver-samples-$sampleCommit/audio/simpleaudiosample"
python "$PSScriptRoot/prepare.py" $sample
if ($LASTEXITCODE) { throw 'Could not prepare the audio driver' }
msbuild "$sample/SimpleAudioSample.sln" /m /p:Configuration=Release /p:Platform=x64 /p:SignMode=Off /p:WindowsTargetPlatformVersion=10.0.26100.0
if ($LASTEXITCODE) { throw 'WDK driver build failed' }
$inf = Get-ChildItem $sample -Filter SimpleAudioSample.inf -Recurse | Select-Object -First 1
$sys = Get-ChildItem $sample -Filter SimpleAudioSample.sys -Recurse | Select-Object -First 1
if (!$inf -or !$sys) { throw 'Missing WDK build output' }
Copy-Item $inf.FullName, $sys.FullName "$OutputDir/"
Copy-Item "$PSScriptRoot/LICENSE.txt" "$OutputDir/"
$inf2cat = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.0.26100.0\x86\Inf2Cat.exe" -ErrorAction Stop
& $inf2cat.FullName "/driver:$OutputDir" '/os:10_CO_X64,10_NI_X64,10_GE_X64'
if ($LASTEXITCODE) { throw 'Driver catalog generation failed' }
