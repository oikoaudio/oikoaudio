$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$mts = Join-Path $env:ProgramFiles "Common Files\MTS-ESP\libMTS.dll"
if (Test-Path $mts) {
    Write-Host "Keeping the existing $mts."
    exit 0
}

$installer = Resolve-Path "vendor\mts-esp\libMTS\Win\libMTSWin_v1.03.exe"
$process = Start-Process -FilePath $installer -Verb RunAs -Wait -PassThru
if ($process.ExitCode -ne 0) {
    throw "MTS-ESP installer exited with code $($process.ExitCode)."
}
Write-Host "Installed the official ODDsound MTS-ESP library. Restart your DAW."
