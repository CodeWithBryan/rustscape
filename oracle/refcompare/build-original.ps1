# Builds the ORIGINAL rev-530 client (reference\client, unchanged sources) with
# javac only: no Gradle, no Kotlin compiler, no network.
#
#   powershell -ExecutionPolicy Bypass -File oracle\refcompare\build-original.ps1 -Copy <workspace root>
#
# Output: <root>\runs\refcompare\orig\{lib,classes-signlink,classes-client}. The jars come out of the
# reference repository's own git objects (`lib/` is in its tree but outside the
# sparse checkout). The one Kotlin source (an interface) is replaced by the
# Java transcription in oracle\refcompare\adapter.
#
# -DriverOnly <orig folder>: compiles only adapter\refcompare\Driver.java
# against an existing build (which is only read) into
# <root>\runs\refcompare\driver-classes; run-original.ps1 puts that folder
# first on the class path when it exists.
param([Parameter(Mandatory = $true)][string]$Copy, [string]$DriverOnly = '')
$ErrorActionPreference = 'Stop'
$ref = Join-Path $Copy 'reference\client'
$out = Join-Path $Copy 'runs\refcompare\orig'
if ($DriverOnly) {
    $classes = Join-Path $Copy 'runs\refcompare\driver-classes'
    New-Item -ItemType Directory -Force $classes | Out-Null
    $jars = (Get-ChildItem (Join-Path $DriverOnly 'lib') -Filter *.jar | ForEach-Object { $_.FullName }) -join ';'
    $cp = (Join-Path $DriverOnly 'classes-client') + ';' + (Join-Path $DriverOnly 'classes-signlink') + ';' + $jars
    $src = Join-Path $Copy 'oracle\refcompare\adapter\refcompare\Driver.java'
    cmd /c "javac -nowarn -encoding UTF-8 --release 8 -proc:none -d `"$classes`" -cp `"$cp`" `"$src`" 2>&1"
    "driver: javac exit=$LASTEXITCODE"
    exit $LASTEXITCODE
}
New-Item -ItemType Directory -Force $out | Out-Null
if (-not (Test-Path (Join-Path $out 'lib\jogl-all.jar'))) {
    $zip = Join-Path $out 'libs.zip'
    git -C $ref archive --format=zip -o $zip HEAD lib
    if ($LASTEXITCODE -ne 0) { throw 'git archive failed' }
    Expand-Archive -Path $zip -DestinationPath $out -Force
}
# signlink and the client share the package rt4, and signlink's sources rely
# on java.awt.* wildcard imports that the client's rt4.Component and
# rt4.DisplayMode would shadow: compile in two steps, as the Gradle build does.
function Invoke-Javac([string]$name, [object[]]$sources, [string]$classes, [string]$cp) {
    $list = Join-Path $out "$name-sources.txt"
    $lines = $sources | ForEach-Object { '"' + ($_.FullName -replace '\\', '/') + '"' }
    [System.IO.File]::WriteAllLines($list, $lines, (New-Object System.Text.UTF8Encoding($false)))
    New-Item -ItemType Directory -Force $classes | Out-Null
    $log = Join-Path $out "$name-javac.log"
    cmd /c "javac -nowarn -encoding UTF-8 --release 8 -proc:none -d `"$classes`" -cp `"$cp`" @`"$list`" > `"$log`" 2>&1"
    "$name`: javac exit=$LASTEXITCODE, sources=$($sources.Count), classes=$((Get-ChildItem $classes -Recurse -Filter *.class).Count), log=$log"
    if ($LASTEXITCODE -ne 0) { exit 1 }
}
$jars = (Get-ChildItem (Join-Path $out 'lib') -Filter *.jar | ForEach-Object { $_.FullName }) -join ';'
$signlinkClasses = Join-Path $out 'classes-signlink'
$clientClasses = Join-Path $out 'classes-client'
$s1 = @()
$s1 += Get-ChildItem (Join-Path $ref 'deob-annotations\src\main\java') -Recurse -Filter *.java
$s1 += Get-ChildItem (Join-Path $ref 'signlink\src\main\java') -Recurse -Filter *.java
Invoke-Javac 'signlink' $s1 $signlinkClasses $jars
$s2 = @()
$s2 += Get-ChildItem (Join-Path $ref 'client\src\main\java') -Recurse -Filter *.java
$s2 += Get-ChildItem (Join-Path $Copy 'oracle\refcompare\adapter') -Recurse -Filter *.java
Invoke-Javac 'client' $s2 $clientClasses "$signlinkClasses;$jars"
Copy-Item (Join-Path $ref 'client\src\main\resources\*') $clientClasses -Force
exit 0