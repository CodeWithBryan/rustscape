# Starts the processes of one comparison: an example server per client (the
# same program, arguments and spawn tile, so both clients see the same
# world; one server each so neither client sees the other's player), the
# original client (run-original.ps1) and the Rust client. Dot-source after
# setting $Copy:
#   $Copy = '...'; . $Copy\oracle\refcompare\launch.ps1
#   $s1 = Start-RcServer -Name a -Port 48901 -Extra @('--gameframe')
#   $o  = Start-RcOrig -Port 48901 -Build <orig folder> -Mode 1 -Fresh
#   $r  = Start-RcRust -Name r -Port 48911 -Mode 1 [-Login]
#   Stop-RcAll          # stops exactly the process ids started here
# Every process id is appended to runs\refcompare\pids.txt; Stop-RcAll stops
# only those.
$global:RcBase = Join-Path $Copy 'runs\refcompare'
New-Item -ItemType Directory -Force $global:RcBase | Out-Null
$global:RcPids = Join-Path $global:RcBase 'pids.txt'

function Add-RcPid([int]$ProcessId, [string]$What) {
    Add-Content -Path $global:RcPids -Value "$ProcessId $What" -Encoding UTF8
}

# An example server (default npc_session, built with
#   cargo build --release -p rs09-server --example npc_session)
# on 127.0.0.1:<Port> (TCP: JS5, world list and login) and <Port>+1 (WebSocket),
# started in the copy's root so its relative fixture paths resolve. Waits for
# its ready line.
function Start-RcServer([string]$Name, [int]$Port, [string[]]$Extra = @(), [string]$Example = 'npc_session') {
    $dir = Join-Path $global:RcBase "server-$Name"
    New-Item -ItemType Directory -Force $dir | Out-Null
    $exe = Join-Path $Copy "target\release\examples\$Example.exe"
    $a = @('--keys', (Join-Path $Copy 'runs\player-session\xteas.json'), '--cache', (Join-Path $Copy 'assets\cache'),
        '--tcp', "127.0.0.1:$Port", '--websocket', "127.0.0.1:$($Port + 1)") + $Extra
    $out = Join-Path $dir 'stdout.log'
    $p = Start-Process -FilePath $exe -ArgumentList $a -WorkingDirectory $Copy -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $out -RedirectStandardError (Join-Path $dir 'stderr.log')
    Add-RcPid $p.Id "server-$Name"
    $deadline = (Get-Date).AddSeconds(120)
    while (-not ((Test-Path $out) -and (Select-String -Path $out -Pattern 'TCP=' -Quiet))) {
        if ($p.HasExited) { throw "server $Name exited: $(Get-Content (Join-Path $dir 'stderr.log') -Raw)" }
        if ((Get-Date) -gt $deadline) { throw "server $Name not ready" }
        Start-Sleep -Milliseconds 250
    }
    $p.Id
}

# The original client (see run-original.ps1); returns its process id.
function Start-RcOrig([int]$Port, [string]$Build, [int]$Mode = 1, [switch]$Fresh, [int]$Brightness = 3, [int]$HdLighting = 1) {
    $line = & (Join-Path $Copy 'oracle\refcompare\run-original.ps1') -Copy $Copy -Port $Port -Mode $Mode -Fresh:$Fresh -Build $Build -Brightness $Brightness -HdLighting $HdLighting
    $id = [int]($line -replace 'PID=', '')
    Add-RcPid $id 'original'
    $id
}

# The Rust client from target\release, run from the copy's root with its own
# preferences file (the same bytes run-original.ps1 writes: mode <Mode>,
# brightness 3, all detail options on, silent) and a frame-dump folder
# runs\refcompare\rust-<Name>\dump. -Login logs in as `visual` at once;
# otherwise it stays on the title screen. Returns @{ Id; Dump; Window }.
function Start-RcRust([string]$Name, [int]$Port, [int]$Mode = 1, [switch]$Login, [string[]]$Extra = @(), [int]$Brightness = 3, [int]$HdLighting = 1) {
    $dir = Join-Path $global:RcBase "rust-$Name"
    $dump = Join-Path $dir 'dump'
    New-Item -ItemType Directory -Force $dump | Out-Null
    # Frame numbers start again with every process: old dumps would be overwritten unseen.
    Get-ChildItem $dump -Filter 'frame-*.png' | Remove-Item
    $prefs = Join-Path $dir 'preferences.dat'
    [byte[]]$b = @(11, $Brightness, 1, 1, 1, 1, 1, 1, 1, 1, 2, $HdLighting, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, $Mode, 0, 0, 0, 0, 1)
    [System.IO.File]::WriteAllBytes($prefs, $b)
    $a = @('--connect', "127.0.0.1:$Port", '--cache', 'assets/cache', '--world-keys', 'runs/player-session/xteas.json',
        '--preferences', $prefs, '--pinned-lights') + $Extra
    if ($Login) { $a += @('--username', 'visual') }
    $env:RS09_FRAME_DUMP = $dump
    $env:RS09_CLIENT_PASSWORD = 'testing'
    # Posted mouse messages count although the real pointer is elsewhere.
    $env:RS09_POSTED_POINTER = '1'
    try {
        $p = Start-Process -FilePath (Join-Path $Copy 'target\release\rs09-client.exe') -ArgumentList $a -WorkingDirectory $Copy -PassThru `
            -RedirectStandardOutput (Join-Path $dir 'stdout.log') -RedirectStandardError (Join-Path $dir 'stderr.log')
    } finally {
        Remove-Item Env:RS09_FRAME_DUMP, Env:RS09_CLIENT_PASSWORD, Env:RS09_POSTED_POINTER -ErrorAction SilentlyContinue
    }
    Add-RcPid $p.Id "rust-$Name"
    $deadline = (Get-Date).AddSeconds(30)
    $window = $null
    while (-not $window) {
        try { $window = Get-RustWindow $p.Id } catch { if ((Get-Date) -gt $deadline) { throw } ; Start-Sleep -Milliseconds 250 }
    }
    @{ Id = $p.Id; Dump = $dump; Window = $window; Dir = $dir }
}

# Stops the processes listed in pids.txt that are still running, by id, and
# empties the list.
function Stop-RcAll {
    if (-not (Test-Path $global:RcPids)) { return }
    foreach ($line in Get-Content $global:RcPids) {
        $id = [int]($line.Split(' ')[0])
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        # A recycled id belongs to someone else: only the programs started here.
        if ($p -and @('java', 'rs09-client', 'npc_session', 'movement_session', 'player_session') -notcontains $p.ProcessName) { $p = $null }
        if ($p) { try { Stop-Process -Id $id -Force -ErrorAction Stop; "stopped $line" } catch { "could not stop $line" } }
    }
    Remove-Item $global:RcPids
}

# Logs the original in as `visual` through its own title screen (Enter on the
# main menu, the two fields, Enter) and waits for game state 30.
function Login-RcOrig([int]$TimeoutSeconds = 90) {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Send-Orig @('get client.gameState')) -notmatch 'gameState=10') {
        if ((Get-Date) -gt $deadline) { throw 'original not at the title' }
        Start-Sleep -Milliseconds 500
    }
    Start-Sleep -Seconds 1
    [void](Send-Orig @('tap ENTER', 'sleep 300', 'type visual', 'tap ENTER', 'sleep 200', 'type testing', 'tap ENTER'))
    while ((Send-Orig @('get client.gameState')) -notmatch 'gameState=30') {
        if ((Get-Date) -gt $deadline) { throw 'original not in game' }
        Start-Sleep -Milliseconds 300
    }
}