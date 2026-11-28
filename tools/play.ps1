<#
.SYNOPSIS
Play the native live world locally: rs09-server --world plus the visual client.

.DESCRIPTION
Each step is skipped when its result already exists:
  1. cargo xtask bootstrap     pinned reference/server and reference/client (partial clones)
  2. cargo xtask cache-fetch   pinned cache into assets/cache
  3. cargo xtask play-world    world folder (pinned Server/data + worldprops/default.conf)
  4. cargo build --release     rs09-server and rs09-client
Then it starts the server in the background (log in runs/play/server.log), waits for its
port, and runs the client in the foreground with audio off (--mute; -Sound keeps it on).
Closing the client stops the server unless -KeepServer is given. See docs/play-locally.md.

.EXAMPLE
pwsh tools/play.ps1
.EXAMPLE
pwsh tools/play.ps1 -Port 43600 -KeepServer
#>
param(
    [int]$Port = 43594,
    [string]$Data = "runs/play/world-data",
    [string]$Saves = "runs/play/saves",
    [string]$Cache = "assets/cache",
    [string]$Python = "",
    [switch]$SkipBuild,
    [Alias("NoClient")][switch]$ServerOnly,
    [switch]$KeepServer,
    [switch]$Sound
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Invoke-Step([string]$Name, [scriptblock]$Body) {
    Write-Host "==> $Name" -ForegroundColor Cyan
    & $Body
    if ($LASTEXITCODE) { throw "$Name failed (exit $LASTEXITCODE)" }
}

function Resolve-Here([string]$Path) {
    if ([System.IO.Path]::IsPathRooted($Path)) { return $Path }
    return Join-Path $Root $Path
}

$Data = Resolve-Here $Data
$Saves = Resolve-Here $Saves
$Cache = Resolve-Here $Cache
$Play = Join-Path $Root "runs/play"
$exe = if ($IsWindows -or $env:OS -eq "Windows_NT") { ".exe" } else { "" }
$Server = Join-Path $Root "target/release/rs09-server$exe"
$Client = Join-Path $Root "target/release/rs09-client$exe"

if (-not ((Test-Path "reference/server/.git") -and (Test-Path "reference/client/.git"))) {
    Invoke-Step "Pinned references (cargo xtask bootstrap)" { cargo xtask bootstrap }
}
if (-not (Test-Path (Join-Path $Cache "main_file_cache.dat2"))) {
    if ($Cache -ne (Resolve-Here "assets/cache")) { throw "No cache in $Cache (cache-fetch only fills assets/cache)" }
    Invoke-Step "Pinned cache (cargo xtask cache-fetch)" { cargo xtask cache-fetch }
}
$worldArgs = @("xtask", "play-world", "--output", $Data)
if ($Python) { $worldArgs += @("--python", $Python) }
Invoke-Step "World folder (cargo xtask play-world)" { cargo @worldArgs }
if (-not $SkipBuild) {
    Invoke-Step "Release build" { cargo build --release -p rs09-server -p rs09-client }
}
foreach ($binary in $Server, $Client) {
    if (-not (Test-Path $binary)) { throw "Missing $binary; run without -SkipBuild" }
}

$probe = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
try { $probe.Start() } catch { throw "Port $Port on 127.0.0.1 is in use; pass -Port N for another one" } finally { $probe.Stop() }

New-Item -ItemType Directory -Force $Saves | Out-Null
$log = Join-Path $Play "server.log"
$errorLog = Join-Path $Play "server.err.log"
$address = "127.0.0.1:$Port"
Write-Host "==> Server on $address (log: $log)" -ForegroundColor Cyan
$serverProcess = Start-Process -FilePath $Server -PassThru -NoNewWindow `
    -RedirectStandardOutput $log -RedirectStandardError $errorLog `
    -ArgumentList @("--world", "--cache", "`"$Cache`"", "--saves", "`"$Saves`"", "--data", "`"$Data`"", "--tcp", $address)
try {
    $deadline = (Get-Date).AddMinutes(3)
    while ($true) {
        if ($serverProcess.HasExited) {
            Get-Content $log, $errorLog -Tail 30 -ErrorAction SilentlyContinue
            throw "The server exited (code $($serverProcess.ExitCode)); see $log and $errorLog"
        }
        $socket = [System.Net.Sockets.TcpClient]::new()
        try { $socket.Connect("127.0.0.1", $Port); break } catch { } finally { $socket.Dispose() }
        if ((Get-Date) -gt $deadline) { throw "The server did not open $address within 3 minutes; see $log" }
        Start-Sleep -Milliseconds 500
    }
    Write-Host "Server is up. Log in with any name and password." -ForegroundColor Green
    if ($ServerOnly) {
        Write-Host "Server only: press Ctrl+C to stop it."
        Wait-Process -Id $serverProcess.Id
        return
    }
    $keys = Join-Path $Data "data/configs/xteas.json"
    Write-Host "==> Client" -ForegroundColor Cyan
    $clientArgs = @("--connect", $address, "--cache", $Cache, "--world-keys", $keys)
    # Audio starts off (the client's --mute development switch); -Sound keeps it.
    if (-not $Sound) { $clientArgs += "--mute" }
    & $Client @clientArgs
}
finally {
    if ($KeepServer -and -not $serverProcess.HasExited) {
        Write-Host "Server still running (pid $($serverProcess.Id)); stop it with: Stop-Process -Id $($serverProcess.Id)"
    } elseif (-not $serverProcess.HasExited) {
        Write-Host "Stopping the server (pid $($serverProcess.Id))"
        Stop-Process -Id $serverProcess.Id
    }
}
