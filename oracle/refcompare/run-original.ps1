# Starts the ORIGINAL client built by build-original.ps1 against a server on
# 127.0.0.1:<Port> (JS5, world list and login all on that one port, which is
# what the Rust example servers offer). Prints the java process id and writes
# it to runs\refcompare\orig.pid. Call it in-process (& script ...): a nested
# powershell whose output is captured never returns, because java inherits
# its pipe.
#
#   powershell -ExecutionPolicy Bypass -File oracle\refcompare\run-original.ps1 -Copy <root> -Port 48731 [-Mode 1] [-Fresh] [-Build <orig folder>]
#
# Nothing is read or written outside <root>\runs\refcompare:
#   home\   -DclientHomeOverride: cache copy (so SignLink.getFile finds
#           home\cache first and never probes c:\rscache etc. for writing),
#           preferences file, random.dat
#   tmp\    -Djava.io.tmpdir (JOGL unpacks its natives there)
#   run\    working directory with config.json; no plugins folder
#   drive\  command folder of oracle\refcompare\adapter\refcompare\Driver.java,
#           the launcher (it calls rt4.client.main and takes commands)
# -Mode is the preferred window mode stored in the preferences file
# (0 = SD software renderer, 1 = HD fixed 765x503, 2 = HD resizable).
# -Fresh rewrites the preferences file (otherwise an existing one is kept, so
# changes made through the client's own graphics options persist).
param(
    [Parameter(Mandatory = $true)][string]$Copy,
    [Parameter(Mandatory = $true)][int]$Port,
    [int]$Mode = 1,
    # Preferences.brightness (1 to 4) and highDetailLighting (0 or 1).
    [int]$Brightness = 3,
    [int]$HdLighting = 1,
    [int]$Fps = 50,
    [switch]$Fresh,
    # The folder build-original.ps1 wrote (default <root>\runs\refcompare\orig);
    # only read, so a build made in another copy can be used.
    [string]$Build = ''
)
$ErrorActionPreference = 'Stop'
$base = Join-Path $Copy 'runs\refcompare'
$orig = $Build
if (-not $orig) { $orig = Join-Path $base 'orig' }
$home_ = Join-Path $base 'home'
$tmp = Join-Path $base 'tmp'
$run = Join-Path $base 'run'
$drive = Join-Path $base 'drive'
foreach ($d in @($home_, $tmp, $run, (Join-Path $home_ 'cache'), (Join-Path $home_ 'cache\runescape'))) {
    New-Item -ItemType Directory -Force $d | Out-Null
}
# The cache: a private copy of the pinned one, so the original only has to
# validate it against the server's checksums instead of downloading it.
$cacheDst = Join-Path $home_ 'cache\runescape'
if (-not (Test-Path (Join-Path $cacheDst 'main_file_cache.dat2'))) {
    Copy-Item (Join-Path $Copy 'assets\cache\main_file_cache.*') $cacheDst
}
$random = Join-Path $home_ 'cache\random.dat'
if (-not (Test-Path $random)) { [System.IO.File]::WriteAllBytes($random, (New-Object byte[] 24)) }
# Preferences.encode(), version 11 (33 bytes after the version byte):
# brightness 3, every detail option on, sound and music volume 0 (silent),
# the byte the source calls favoriteWorlds is the preferred window mode,
# safeMode 0.
$prefs = Join-Path $home_ 'jagex_runescape_preferences.dat'
if ($Fresh -or -not (Test-Path $prefs)) {
    [byte[]]$b = @(11, $Brightness, 1, 1, 1, 1, 1, 1, 1, 1, 2, $HdLighting, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, $Mode, 0, 0, 0, 0, 1)
    [System.IO.File]::WriteAllBytes($prefs, $b)
}
# world 1 => JS5 and login go to server_port + 1; the world list to wl_port.
$config = @"
{
    "ip_management": "127.0.0.1",
    "ip_address": "127.0.0.1",
    "world": 1,
    "server_port": $($Port - 1),
    "wl_port": $Port,
    "js5_port": $Port,
    "mouseWheelZoom": true,
    "pluginsFolder": "plugins"
}
"@
[System.IO.File]::WriteAllText((Join-Path $run 'config.json'), $config, (New-Object System.Text.UTF8Encoding($false)))
$jars = (Get-ChildItem (Join-Path $orig 'lib') -Filter *.jar | ForEach-Object { $_.FullName }) -join ';'
$cp = (Join-Path $orig 'classes-client') + ';' + (Join-Path $orig 'classes-signlink') + ';' + $jars
# A newer Driver compiled by build-original.ps1 -DriverOnly comes first.
$driverClasses = Join-Path $base 'driver-classes'
if (Test-Path (Join-Path $driverClasses 'refcompare\Driver.class')) { $cp = $driverClasses + ';' + $cp }
$homeArg = ($home_ -replace '\\', '/') + '/'
$args_ = @(
    '--add-exports', 'java.base/java.lang=ALL-UNNAMED',
    '--add-exports', 'java.desktop/sun.awt=ALL-UNNAMED',
    '--add-exports', 'java.desktop/sun.java2d=ALL-UNNAMED',
    '--add-opens', 'java.desktop/sun.awt=ALL-UNNAMED',
    '--add-opens', 'java.desktop/sun.awt.windows=ALL-UNNAMED',
    '--add-opens', 'java.desktop/java.awt=ALL-UNNAMED',
    '--enable-native-access=ALL-UNNAMED',
    '-Dsun.java2d.uiScale=1',
    '-Dsun.java2d.d3d=false',
    "-DclientHomeOverride=$homeArg",
    "-Djava.io.tmpdir=$tmp",
    "-DclientFps=$Fps",
    "-Drefcompare.dir=$drive",
    '-Xmx1g',
    '-cp', $cp,
    'refcompare.Driver'
)
$p = Start-Process -FilePath 'java' -ArgumentList $args_ -WorkingDirectory $run -PassThru `
    -RedirectStandardOutput (Join-Path $run 'stdout.log') -RedirectStandardError (Join-Path $run 'stderr.log')
[System.IO.File]::WriteAllText((Join-Path $base 'orig.pid'), [string]$p.Id)
"PID=$($p.Id)"
