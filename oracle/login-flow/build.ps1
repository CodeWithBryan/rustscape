# Builds and runs the login oracle (the JVM side of crates\client\tests\login_flow.rs).
# Usage, from anywhere:
#   powershell -ExecutionPolicy Bypass -File oracle\login-flow\build.ps1 crates\client\tests\login_flow\scenarios.txt crates\client\tests\login_flow\java_trace.txt
# (relative paths are taken from the workspace root). The generated LoginManager slice
# and the classes go to runs\login-flow-oracle\ (git-ignored).
param([string]$Scenarios, [string]$Trace)
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = (Resolve-Path (Join-Path $here '..\..')).Path
$out = Join-Path $root 'runs\login-flow-oracle'
New-Item -ItemType Directory -Force (Join-Path $out 'gen\rt4') | Out-Null
if ($Scenarios -and -not [IO.Path]::IsPathRooted($Scenarios)) { $Scenarios = Join-Path $root $Scenarios }
if ($Trace -and -not [IO.Path]::IsPathRooted($Trace)) { $Trace = Join-Path $root $Trace }
$java = "$root\reference\client\client\src\main\java\rt4"
$lines = [IO.File]::ReadAllLines("$java\LoginManager.java")
# 1-based line ranges of the unchanged original: buffer/ZAP/UNZAP, the login statics, clear..startLogin.
if ($lines[159] -notmatch 'public static void clear\(\)') { throw 'LoginManager.java layout changed (clear)' }
if ($lines[572] -notmatch 'public static void startLogin\(') { throw 'LoginManager.java layout changed (startLogin)' }
if ($lines[599].Trim() -ne '}' -or $lines[601] -notmatch 'OriginalMember') { throw 'LoginManager.java layout changed (end)' }
if ($lines[37] -notmatch 'client!rl' -or $lines[76] -notmatch 'parentalAdvertConsent') { throw 'LoginManager.java layout changed (fields)' }
$body = @('package rt4;','','import org.openrs2.deob.annotation.OriginalArg;','import org.openrs2.deob.annotation.OriginalMember;','import org.openrs2.deob.annotation.Pc;','','import java.io.IOException;','import java.net.Socket;','','// GENERATED: lines 12-17, 38-77 and 159-600 of the reference LoginManager.java, unchanged.','public class LoginManager {') + $lines[11..16] + $lines[37..76] + @('') + $lines[158..599] + @('}')
[IO.File]::WriteAllLines("$out\gen\rt4\LoginManager.java", $body, (New-Object Text.UTF8Encoding($false)))
$originals = 'Node','HashTable','StringNode','Buffer','Packet','IsaacRandom','JagString','StringInterface','BrowserControl','Base37','GlobalConfig','World','AbstractWorld','WorldInfo','WorldList','CreateManager' | ForEach-Object { "$java\$_.java" }
$sources = @($originals) + @("$root\reference\client\signlink\src\main\java\rt4\PrivilegedRequest.java") + (Get-ChildItem "$here\adapters\rt4\*.java" | ForEach-Object FullName) + @("$out\gen\rt4\LoginManager.java", "$here\rt4\LoginOracle.java", "$here\rt4\HopProbe.java")
if (Test-Path "$out\classes") { Remove-Item -Recurse -Force "$out\classes" }
New-Item -ItemType Directory -Force "$out\classes" | Out-Null
& javac --release 8 -encoding UTF-8 -nowarn -sourcepath "$root\reference\client\deob-annotations\src\main\java" -d "$out\classes" @sources
if ($LASTEXITCODE -ne 0) { throw 'javac failed' }
if ($Scenarios) {
    & java -cp "$out\classes" rt4.LoginOracle $Scenarios $Trace
    if ($LASTEXITCODE -ne 0) { throw 'oracle failed' }
}
