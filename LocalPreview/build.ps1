$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'gui\build.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Launcher build failed' }
