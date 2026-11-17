# Functions for driving and capturing the two clients. Dot-source after
# setting $Copy (workspace root) and $Evidence (output folder):
#   $Copy = '...'; $Evidence = '...'; . $Copy\oracle\refcompare\session.ps1
. (Join-Path $Copy 'oracle\refcompare\win.ps1')
$global:Base = Join-Path $Copy 'runs\refcompare'
$global:Drive = Join-Path $global:Base 'drive'

# Runs driver commands in the original client (see adapter\refcompare\Driver.java)
# and returns what they printed.
function Send-Orig([string[]]$Commands, [int]$TimeoutSeconds = 120) {
    New-Item -ItemType Directory -Force $global:Drive | Out-Null
    $out = Join-Path $global:Drive 'out.txt'
    $done = Join-Path $global:Drive 'done.txt'
    $before = 0
    if (Test-Path $out) { $before = (Get-Item $out).Length }
    if (Test-Path $done) { Remove-Item $done }
    $tmp = Join-Path $global:Drive 'cmd.tmp'
    [System.IO.File]::WriteAllLines($tmp, $Commands, (New-Object System.Text.UTF8Encoding($false)))
    Move-Item $tmp (Join-Path $global:Drive 'cmd.txt') -Force
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (-not (Test-Path $done)) {
        if ((Get-Date) -gt $deadline) { throw 'driver did not answer' }
        Start-Sleep -Milliseconds 50
    }
    if (Test-Path $out) {
        $bytes = [System.IO.File]::ReadAllBytes($out)
        [System.Text.Encoding]::UTF8.GetString($bytes, $before, $bytes.Length - $before).TrimEnd()
    }
}

# The original's canvas window (class SunAwtCanvas inside its SunAwtFrame).
function Get-OrigCanvas([int]$ProcessId) {
    foreach ($w in [RefCompare.Win]::Windows($ProcessId)) {
        $f = $w.Split('|')
        if ($f[1] -eq 'SunAwtFrame') {
            foreach ($c in [RefCompare.Win]::Children([IntPtr][int64]$f[0])) {
                $g = $c.Split('|')
                if ($g[1] -eq 'SunAwtCanvas') { return [IntPtr][int64]$g[0] }
            }
        }
    }
    throw "no canvas for process $ProcessId"
}

# Grabs the original's canvas to <Evidence>\<Name>.png and logs it.
function Grab-Orig([int]$ProcessId, [string]$Name, [string]$Note) {
    $path = Join-Path $Evidence "$Name.png"
    $ok = [RefCompare.Win]::Grab((Get-OrigCanvas $ProcessId), $path, 3)
    Add-Content -Path (Join-Path $Evidence 'LOG.txt') -Value ("{0}  {1}.png  [original] {2}" -f (Get-Date -Format 'HH:mm:ss'), $Name, $Note) -Encoding UTF8
    "$Name.png ok=$ok"
}

# The Rust client's own frame dump: RS09_FRAME_DUMP=<Dump>; a file `take`
# there makes it write frame-N.png, which is moved to <Evidence>\<Name>.png.
function Grab-Rust([string]$Dump, [string]$Name, [string]$Note, [int]$TimeoutSeconds = 20) {
    $before = @(Get-ChildItem $Dump -Filter 'frame-*.png' | ForEach-Object { $_.Name + '|' + $_.LastWriteTimeUtc.Ticks })
    [System.IO.File]::WriteAllText((Join-Path $Dump 'take'), '')
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ($true) {
        $new = @(Get-ChildItem $Dump -Filter 'frame-*.png' | Where-Object { $before -notcontains ($_.Name + '|' + $_.LastWriteTimeUtc.Ticks) })
        if ($new.Count -gt 0) { break }
        if ((Get-Date) -gt $deadline) { throw 'no frame dump' }
        Start-Sleep -Milliseconds 100
    }
    Start-Sleep -Milliseconds 300
    $path = Join-Path $Evidence "$Name.png"
    Move-Item $new[0].FullName $path -Force
    Add-Content -Path (Join-Path $Evidence 'LOG.txt') -Value ("{0}  {1}.png  [rust] {2}" -f (Get-Date -Format 'HH:mm:ss'), $Name, $Note) -Encoding UTF8
    "$Name.png"
}

# The Rust client's window (miniquad class MINIQUADAPP).
function Get-RustWindow([int]$ProcessId) {
    foreach ($w in [RefCompare.Win]::Windows($ProcessId)) {
        $f = $w.Split('|')
        if ($f[1] -eq 'MINIQUADAPP') { return [IntPtr][int64]$f[0] }
    }
    throw "no window for process $ProcessId"
}

# Input for the Rust client, posted to its own window. Coordinates are window
# client pixels: add the canvas's left margin (129 in a 1024-wide frame) to a
# canvas coordinate.
function Rust-Move([IntPtr]$Window, [int]$X, [int]$Y) { [RefCompare.Win]::Move($Window, $X, $Y); Start-Sleep -Milliseconds 80 }
function Rust-Click([IntPtr]$Window, [int]$X, [int]$Y, [switch]$Right) {
    [RefCompare.Win]::Move($Window, $X, $Y); Start-Sleep -Milliseconds 80
    [RefCompare.Win]::Down($Window, $X, $Y, [bool]$Right); Start-Sleep -Milliseconds 80
    [RefCompare.Win]::Up($Window, $X, $Y, [bool]$Right); Start-Sleep -Milliseconds 80
}
function Rust-KeyDown([IntPtr]$Window, [int]$Vk) { [RefCompare.Win]::KeyDown($Window, $Vk, 0) }
function Rust-KeyUp([IntPtr]$Window, [int]$Vk) { [RefCompare.Win]::KeyUp($Window, $Vk, 0) }
function Rust-Type([IntPtr]$Window, [string]$Text) {
    foreach ($c in $Text.ToCharArray()) {
        $vk = [int][char]::ToUpper($c)
        # No WM_CHAR of our own: the client's message loop translates the posted key.
        [RefCompare.Win]::KeyDown($Window, $vk, 0); Start-Sleep -Milliseconds 45
        [RefCompare.Win]::KeyUp($Window, $vk, 0); Start-Sleep -Milliseconds 45
    }
}

function Rust-Tap([IntPtr]$Window, [int]$Vk, [int]$HoldMs = 60) {
    [RefCompare.Win]::KeyDown($Window, $Vk, 0); Start-Sleep -Milliseconds $HoldMs
    [RefCompare.Win]::KeyUp($Window, $Vk, 0); Start-Sleep -Milliseconds 60
}

# The camera the Rust client draws with (needs the pose instrument,
# oracle\refcompare\rust-pose-dump.patch, in the client that is running).
function Get-RustPose([string]$Dump) {
    $json = Join-Path $Dump 'pose.json'
    if (Test-Path $json) { Remove-Item $json }
    [System.IO.File]::WriteAllText((Join-Path $Dump 'pose'), '')
    $deadline = (Get-Date).AddSeconds(5)
    while (-not (Test-Path $json)) {
        if ((Get-Date) -gt $deadline) { throw 'no pose.json' }
        Start-Sleep -Milliseconds 40
    }
    Start-Sleep -Milliseconds 60
    Get-Content $json -Raw | ConvertFrom-Json
}

# Pins the original's follow camera to a Rust pose: the same targets, drift,
# offsets and orbit point, re-applied every 5 ms by the driver. Returns the
# render position and angles the ORIGINAL then computes from them, to be
# compared with the Rust pose's own.
function Set-OrigPose($Pose) {
    $cmds = @(
        'unhold',
        "hold Camera.yawTarget $($Pose.yaw_target)",
        "hold Camera.yawDrift $($Pose.yaw_drift)",
        "hold Camera.pitchTarget $($Pose.pitch_target)",
        "hold Camera.cameraOffsetX $($Pose.offset[0])",
        "hold Camera.cameraOffsetY $($Pose.offset[1])",
        "hold Camera.cameraX $($Pose.orbit_point[0])",
        "hold Camera.cameraY $($Pose.orbit_point[1])",
        "hold Camera.maxPitchDistance $($Pose.max_pitch_distance)",
        "hold Camera.ZOOM $($Pose.zoom)",
        'sleep 400',
        'get Camera.renderX Camera.renderY Camera.renderZ Camera.cameraPitch Camera.cameraYaw ScriptRunner.viewportZoom PlayerList.self.xFine PlayerList.self.yFine'
    )
    $r = Send-Orig $cmds
    ($r -split "`n")[-1]
}
# One in-game pair at the same camera: reads the Rust pose, pins the original
# to it, grabs both, writes <Evidence>\pairs\<Name>-{orig,rust,pair}.png (the
# Rust frame cropped to its canvas; pair = original | rust | difference x4)
# and prints how the original's own render position compares and the pixel
# statistics of the whole canvas and of the scene viewport (fixed gameframe:
# 512x334 at 4,4). Needs image.ps1 loaded and $OrigPid, $RustDump set.
function Pair-Now([string]$Name, [string]$Note, [int]$Margin = 129, [int[]]$View = @(4, 4, 512, 334), [int[]]$Canvas = @(765, 503)) {
    $pose = Get-RustPose $RustDump
    $orig = Set-OrigPose $pose
    $want = "Camera.renderX=$($pose.position[0]) Camera.renderY=$($pose.position[1]) Camera.renderZ=$($pose.position[2]) Camera.cameraPitch=$($pose.pitch) Camera.cameraYaw=$($pose.yaw)"
    $same = $orig.StartsWith($want)
    $poseText = "eye $($pose.position -join ',') pitch $($pose.pitch) yaw $($pose.yaw)"
    [void](Grab-Orig $OrigPid "orig-$Name" "$Note; camera pinned to the Rust pose ($poseText); original's own render position equal: $same")
    [void](Grab-Rust $RustDump "rust-$Name" "$Note; $poseText")
    $pairs = Join-Path $Evidence 'pairs'
    $o = Join-Path $pairs "$Name-orig.png"; $r = Join-Path $pairs "$Name-rust.png"
    Copy-Item (Join-Path $Evidence "orig-$Name.png") $o -Force
    [RefCompare.Img]::Crop((Join-Path $Evidence "rust-$Name.png"), $r, $Margin, 0, $Canvas[0], $Canvas[1])
    [RefCompare.Img]::Pair($o, $r, (Join-Path $pairs "$Name-pair.png"), 4)
    $to = Join-Path $pairs 'tmp-o.png'; $tr = Join-Path $pairs 'tmp-r.png'
    [RefCompare.Img]::Crop($o, $to, $View[0], $View[1], $View[2], $View[3]); [RefCompare.Img]::Crop($r, $tr, $View[0], $View[1], $View[2], $View[3])
    "$Name  rust $poseText"
    "  original render position equal: $same  ($orig)"
    "  canvas   " + [RefCompare.Img]::Compare($o, $r)
    "  viewport " + [RefCompare.Img]::Compare($to, $tr)
    Add-Content -Path (Join-Path $Evidence 'LOG.txt') -Value ("          pairs\{0}-pair.png  pose equal: {1}; viewport {2}" -f $Name, $same, [RefCompare.Img]::Compare($to, $tr)) -Encoding UTF8
}

# The title screen's spline camera (CS2 5502). The Rust pose names its
# segment by its control points; the original's segment is found among the
# rows of Camera.cameraPathData, then the original is held on that segment
# at the Rust's progress with both speeds 0, so that it stands still exactly
# where the Rust camera was. Both clients must be showing the same title
# place (SceneGraph.centralZone* equal to the Rust log's zone). Returns a
# text that says what was matched.
function Find-OrigSegment([int]$PathId, $Points) {
    $text = Send-Orig @("aget Camera.cameraPathData $PathId")
    $rows = @([regex]::Matches($text, '\[(-?\d+), (-?\d+), (-?\d+)(?:, -?\d+)*\]') | ForEach-Object { , @([int]$_.Groups[1].Value, [int]$_.Groups[2].Value, [int]$_.Groups[3].Value) })
    for ($i = 0; $i + 3 -lt $rows.Count; $i += 2) {
        $ok = $true
        for ($k = 0; $k -lt 4; $k++) {
            for ($a = 0; $a -lt 3; $a++) { if ($rows[$i + $k][$a] -ne [int]$Points[$k][$a]) { $ok = $false } }
        }
        if ($ok) { return $i / 2 }
    }
    -1
}
function Set-OrigSpline($Pose) {
    $s = $Pose.spline
    if (-not $s) { throw 'the Rust camera is not on a spline' }
    $ids = Send-Orig @('unhold', 'get Camera.movePathId Camera.lookAtPathId')
    $v = @{}; foreach ($kv in ($ids -split "`n")[-1].Split(' ')) { $q = $kv.Split('='); $v[$q[0]] = [int]$q[1] }
    $move = Find-OrigSegment $v['Camera.movePathId'] $s.position
    $look = Find-OrigSegment $v['Camera.lookAtPathId'] $s.look
    if ($move -lt 0 -or $look -lt 0) { return "no matching segment (move $move, look $look)" }
    $r = Send-Orig @(
        'hold Camera.moveSplineSpeedStart 0', 'hold Camera.moveSplineSpeedEnd 0',
        "hold Camera.moveSplineIndex $move", "hold Camera.lookAtSplineIndex $look",
        "hold Camera.splineProgress $($s.progress)",
        'sleep 300',
        'get Camera.renderX Camera.renderY Camera.renderZ Camera.cameraPitch Camera.cameraYaw')
    "segments move $move look $look progress $($s.progress); original " + ($r -split "`n")[-1]
}
# A locked camera (CS2 camera position and look-at, cameraType 1): with
# every speed and minimum step 0, Camera.updateLockedCamera moves nothing, so
# the held render position and angles stay as given (shifted by the
# difference of the scene bases, if any).
function Set-OrigLocked($Pose) {
    $o = Send-Orig @('unhold', 'get Camera.originX Camera.originY')
    $v = @{}; foreach ($kv in ($o -split "`n")[-1].Split(' ')) { $q = $kv.Split('='); $v[$q[0]] = [int]$q[1] }
    $dx = ([int]$Pose.origin[0] - $v['Camera.originX']) * 128
    $dy = ([int]$Pose.origin[1] - $v['Camera.originY']) * 128
    $x = [int]$Pose.position[0] + $dx; $y = [int]$Pose.position[1] + $dy
    $r = Send-Orig @(
        'hold Camera.lockedMoveSpeed 0', 'hold Camera.lockedMinMoveStep 0', 'hold Camera.lockedAngleSpeed 0', 'hold Camera.lockedMinStep 0',
        "hold Camera.renderX $x", "hold Camera.renderY $y", "hold Camera.renderZ $($Pose.position[2])",
        "hold Camera.cameraPitch $($Pose.pitch)", "hold Camera.cameraYaw $($Pose.yaw)",
        'sleep 300',
        'get Camera.renderX Camera.renderY Camera.renderZ Camera.cameraPitch Camera.cameraYaw')
    "locked camera, base difference $dx,$dy; original " + ($r -split "`n")[-1]
}
# The Rust pose and the frame drawn with it: both requests are written at
# once, so the client answers them in the same frame.
function Get-RustPoseAndFrame([string]$Name, [string]$Note) {
    $json = Join-Path $RustDump 'pose.json'
    if (Test-Path $json) { Remove-Item $json }
    $before = @(Get-ChildItem $RustDump -Filter 'frame-*.png' | ForEach-Object { $_.Name + '|' + $_.LastWriteTimeUtc.Ticks })
    [System.IO.File]::WriteAllText((Join-Path $RustDump 'pose'), '')
    [System.IO.File]::WriteAllText((Join-Path $RustDump 'take'), '')
    $deadline = (Get-Date).AddSeconds(20)
    while ($true) {
        $new = @(Get-ChildItem $RustDump -Filter 'frame-*.png' | Where-Object { $before -notcontains ($_.Name + '|' + $_.LastWriteTimeUtc.Ticks) })
        if ($new.Count -gt 0 -and (Test-Path $json)) { break }
        if ((Get-Date) -gt $deadline) { throw 'no frame dump or pose' }
        Start-Sleep -Milliseconds 50
    }
    Start-Sleep -Milliseconds 300
    Move-Item $new[0].FullName (Join-Path $Evidence "rust-$Name.png") -Force
    $pose = Get-Content $json -Raw | ConvertFrom-Json
    Add-Content -Path (Join-Path $Evidence 'LOG.txt') -Value ("{0}  rust-{1}.png  [rust] {2}; eye {3} pitch {4} yaw {5}" -f (Get-Date -Format 'HH:mm:ss'), $Name, $Note, ($pose.position -join ','), $pose.pitch, $pose.yaw) -Encoding UTF8
    $pose
}
# One title-scene pair with the original pinned to the Rust spline camera.
function Pair-Title([string]$Name, [string]$Note, [int]$Margin = 129) {
    $pose = Get-RustPoseAndFrame $Name $Note
    if ($pose.spline) { $pin = Set-OrigSpline $pose } else { $pin = Set-OrigLocked $pose }
    $want = "Camera.renderX=$($pose.position[0]) Camera.renderY=$($pose.position[1]) Camera.renderZ=$($pose.position[2]) Camera.cameraPitch=$($pose.pitch) Camera.cameraYaw=$($pose.yaw)"
    $same = $pin.EndsWith($want)
    [void](Grab-Orig $OrigPid "orig-$Name" "$Note; spline pinned to the Rust pose ($pin); render position equal: $same")
    $pairs = Join-Path $Evidence 'pairs'
    $o = Join-Path $pairs "$Name-orig.png"; $r = Join-Path $pairs "$Name-rust.png"
    Copy-Item (Join-Path $Evidence "orig-$Name.png") $o -Force
    [RefCompare.Img]::Crop((Join-Path $Evidence "rust-$Name.png"), $r, $Margin, 0, 765, 503)
    [RefCompare.Img]::Pair($o, $r, (Join-Path $pairs "$Name-pair.png"), 4)
    $stats = [RefCompare.Img]::Compare($o, $r)
    "$Name  rust eye $($pose.position -join ',') pitch $($pose.pitch) yaw $($pose.yaw)"
    "  $pin  equal: $same"
    "  canvas $stats"
    Add-Content -Path (Join-Path $Evidence 'LOG.txt') -Value ("          pairs\{0}-pair.png  pose equal: {1}; canvas {2}" -f $Name, $same, $stats) -Encoding UTF8
}

# A pair of whatever both clients show now, without touching the camera
# (title screen, login form, interfaces): grabs both, writes the same three
# files as Pair-Now and logs the statistics of the whole canvas.
function Pair-Ui([string]$Name, [string]$Note, [int]$Margin = 129, [int[]]$Canvas = @(765, 503)) {
    [void](Grab-Orig $OrigPid "orig-$Name" $Note)
    [void](Grab-Rust $RustDump "rust-$Name" $Note)
    $pairs = Join-Path $Evidence 'pairs'
    $o = Join-Path $pairs "$Name-orig.png"; $r = Join-Path $pairs "$Name-rust.png"
    Copy-Item (Join-Path $Evidence "orig-$Name.png") $o -Force
    [RefCompare.Img]::Crop((Join-Path $Evidence "rust-$Name.png"), $r, $Margin, 0, $Canvas[0], $Canvas[1])
    [RefCompare.Img]::Pair($o, $r, (Join-Path $pairs "$Name-pair.png"), 4)
    $stats = [RefCompare.Img]::Compare($o, $r)
    "$Name  canvas $stats"
    Add-Content -Path (Join-Path $Evidence 'LOG.txt') -Value ("          pairs\{0}-pair.png  canvas {1}" -f $Name, $stats) -Encoding UTF8
}
