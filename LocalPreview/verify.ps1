param(
    [string]$Tolua = 'E:\MajSoul\GameClient\Jantama_MahjongSoul_Data\Plugins\x86\tolua.dll',
    [string]$Dump = 'E:\MajSoul\GameClient\MajsoulPreview\dump'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $root
try {
    & cargo test --target i686-pc-windows-msvc --lib
    if ($LASTEXITCODE -ne 0) { throw 'Unit tests failed' }
    & cargo build --release --target i686-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    $release = Join-Path $root 'target\i686-pc-windows-msvc\release'
    $runtime = Join-Path $release 'MaJmods'
    New-Item -ItemType Directory -Force -Path $runtime | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $root 'data') -File | Copy-Item -Destination $runtime -Force
    $checker = Join-Path $release 'preview-check.exe'
    $dll = Join-Path $release 'version.dll'
    & $checker --tolua $Tolua --data (Join-Path $root 'data') --dump $Dump --tests (Join-Path $root 'tests') --hook $dll
    if ($LASTEXITCODE -ne 0) { throw 'Interpreter/native integration tests failed' }
    try {
        # Corrupt only the PRIVATE test-host copy, to prove native compilation failure
        # falls back to the original module. The package/game files are not touched.
        [System.IO.File]::AppendAllText((Join-Path $runtime 'runtime.lua'), "`nthis is not valid Lua !!!")
        & $checker --tolua $Tolua --data (Join-Path $root 'data') --hook $dll --fallback-only
        if ($LASTEXITCODE -ne 0) { throw 'Native rollback test failed' }
    } finally {
        Copy-Item -LiteralPath (Join-Path $root 'data\runtime.lua') -Destination $runtime -Force
    }
    Write-Host 'All offline checks passed. Visual in-game acceptance is separate.'
} finally { Pop-Location }
