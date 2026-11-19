param([switch]$StageOnly)
$ErrorActionPreference = 'Stop'
$project = Split-Path -Parent $PSScriptRoot
Push-Location $project
try {
    & cargo build --release --target i686-pc-windows-msvc --features launcher --bin MJLocalPreview --lib
    if ($LASTEXITCODE -ne 0) { throw 'Native engine build failed' }
    $framework = Join-Path $env:WINDIR 'Microsoft.NET\Framework\v4.0.30319'
    $csc = Join-Path $framework 'csc.exe'
    $output = Join-Path $project 'target\gui'
    $launcherExe = 'Majsoul-modLauncher.exe'
    $companionDll = 'MaJmods.dll'
    New-Item -ItemType Directory -Path $output -Force | Out-Null
    $references = @('System.dll','System.Core.dll','System.Xaml.dll','System.Xml.dll','System.IO.Compression.dll','System.IO.Compression.FileSystem.dll') | ForEach-Object { '/reference:' + (Join-Path $framework $_) }
    $references += @('WindowsBase.dll','PresentationCore.dll','PresentationFramework.dll') | ForEach-Object { '/reference:' + (Join-Path $framework ('WPF\' + $_)) }
    $engine = Join-Path $project 'target\i686-pc-windows-msvc\release\MJLocalPreview.exe'
    & $csc /nologo /target:winexe /platform:x86 /optimize+ /utf8output /codepage:65001 ('/out:' + (Join-Path $output $launcherExe)) ('/win32manifest:' + (Join-Path $PSScriptRoot 'app.manifest')) ('/win32icon:' + (Join-Path $PSScriptRoot 'app.ico')) ('/resource:' + (Join-Path $PSScriptRoot 'app.ico') + ',MJ.Icon') ('/resource:' + (Join-Path $PSScriptRoot 'assets.zip') + ',MJ.Assets') ('/resource:' + (Join-Path $PSScriptRoot 'MainWindow.xaml') + ',MJ.Window') ('/resource:' + $engine + ',MJ.Engine') @references (Join-Path $PSScriptRoot 'Launcher.cs')
    if ($LASTEXITCODE -ne 0) { throw 'Graphical launcher build failed' }
    Copy-Item -LiteralPath (Join-Path $project 'target\i686-pc-windows-msvc\release\version.dll') -Destination (Join-Path $output $companionDll) -Force
    if (-not $StageOnly) {
        foreach ($name in @($companionDll,$launcherExe)) {
            $source = Join-Path $output $name
            $destination = Join-Path $project $name
            if ((Test-Path -LiteralPath $destination) -and ((Get-FileHash -LiteralPath $source).Hash -eq (Get-FileHash -LiteralPath $destination).Hash)) { continue }
            Copy-Item -LiteralPath $source -Destination $destination -Force
        }
    }
    Write-Host ('Graphical launcher ready: ' + (Join-Path $output $launcherExe))
} finally { Pop-Location }
