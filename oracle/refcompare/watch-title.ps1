# Samples both clients' title screens side by side for a while. The title
# interface's script flies the camera through a list of places; a sample
# records which place each client is in (original: SceneGraph.centralZone*,
# Rust: its last "title scene around zone" log line) and how long it has been
# there, so frames of the same place at the same point of the flight can be
# paired afterwards.
param(
    [Parameter(Mandatory = $true)][string]$Copy,
    [Parameter(Mandatory = $true)][string]$Evidence,
    [int]$Seconds = 240,
    [string]$Folder = 'title-watch',
    # The Rust client's folder (dump\ and stderr.log), as launch.ps1 makes it.
    [string]$RustDir = ''
)
$ErrorActionPreference = 'Stop'
. (Join-Path $Copy 'oracle\refcompare\session.ps1')
$origPid = [int](Get-Content (Join-Path $Copy 'runs\refcompare\orig.pid'))
$rustBase = $RustDir
if (-not $rustBase) { $rustBase = Join-Path $Copy 'runs\refcompare\rust' }
$out = Join-Path $Evidence $Folder
New-Item -ItemType Directory -Force $out | Out-Null
$csv = Join-Path $out 'samples.csv'
'index,ms,orig_zone,orig_state,orig_render,orig_pitch_yaw,orig_loop,rust_zone,rust_scene_count' | Set-Content $csv -Encoding UTF8
$canvas = Get-OrigCanvas $origPid
$sw = [Diagnostics.Stopwatch]::StartNew()
$i = 0
while ($sw.Elapsed.TotalSeconds -lt $Seconds) {
    $ms = [int]$sw.ElapsedMilliseconds
    $s = Send-Orig @('get SceneGraph.centralZoneX SceneGraph.centralZoneY client.gameState Camera.renderX Camera.renderY Camera.renderZ Camera.cameraPitch Camera.cameraYaw client.loop')
    [void][RefCompare.Win]::Grab($canvas, (Join-Path $out ('orig-{0:d3}.png' -f $i)), 3)
    $v = @{}
    foreach ($kv in $s.Split(' ')) { $p = $kv.Split('='); $v[$p[0]] = $p[1] }
    [System.IO.File]::WriteAllText((Join-Path $rustBase 'dump\take'), '')
    $deadline = (Get-Date).AddSeconds(5)
    $frame = $null
    while (-not $frame -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 60
        $frame = Get-ChildItem (Join-Path $rustBase 'dump') -Filter 'frame-*.png' | Select-Object -First 1
    }
    if ($frame) {
        Start-Sleep -Milliseconds 150
        Move-Item $frame.FullName (Join-Path $out ('rust-{0:d3}.png' -f $i)) -Force
    }
    $lines = @(Select-String -Path (Join-Path $rustBase 'stderr.log') -Pattern 'title scene around zone \[(\d+), (\d+)\]')
    $rz = ''
    if ($lines.Count -gt 0) { $m = $lines[-1].Matches[0]; $rz = $m.Groups[1].Value + ' ' + $m.Groups[2].Value }
    ('{0},{1},{2} {3},{4},{5} {6} {7},{8} {9},{10},{11},{12}' -f $i, $ms, $v['SceneGraph.centralZoneX'], $v['SceneGraph.centralZoneY'], $v['client.gameState'], $v['Camera.renderX'], $v['Camera.renderY'], $v['Camera.renderZ'], $v['Camera.cameraPitch'], $v['Camera.cameraYaw'], $v['client.loop'], $rz, $lines.Count) | Add-Content $csv -Encoding UTF8
    $i++
    Start-Sleep -Milliseconds 400
}
"samples=$i csv=$csv"
