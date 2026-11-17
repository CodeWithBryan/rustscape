# Starts the Rust client on the title screen again and again until its first
# title place (the zone its log names) is the place the running original is
# showing, so that Pair-Title can pin the original to the Rust camera. Both
# clients go through the same list of places in the same order with the same
# durations; the starting place is random in both.
# Dot-source session.ps1 and launch.ps1 first; $OrigPid must be set.
#   $r = & oracle\refcompare\title-match.ps1 -Port 48911 [-Tries 20]
param([int]$Port, [int]$Tries = 20, [string]$Name = 'r', [int]$Mode = 1)
# The zones a place's flight starts in (seen as the Rust client's first zone);
# while the original is in another zone (the middle of a flight that crosses
# zones) no start can match, so this waits.
$starts = @{}
$until = (Get-Date).AddMinutes(20)
for ($try = 1; $try -le $Tries -and (Get-Date) -lt $until; $try++) {
    if ($starts.Count -ge 4) {
        $o = Send-Orig @('get SceneGraph.centralZoneX SceneGraph.centralZoneY')
        $oz = ($o -replace 'SceneGraph.centralZoneX=', '' -replace ' SceneGraph.centralZoneY=', ' ').Trim()
        if (-not $starts.ContainsKey($oz)) { Start-Sleep -Seconds 2; $try--; continue }
    }
    $r = Start-RcRust -Name $Name -Port $Port -Mode $Mode
    $log = Join-Path $r.Dir 'stderr.log'
    $deadline = (Get-Date).AddSeconds(40)
    $zone = $null
    while (-not $zone -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 300
        $m = @(Select-String -Path $log -Pattern 'title scene around zone \[(\d+), (\d+)\]' -ErrorAction SilentlyContinue)
        if ($m.Count -gt 0) { $zone = $m[-1].Matches[0].Groups[1].Value + ' ' + $m[-1].Matches[0].Groups[2].Value }
    }
    $o = Send-Orig @('get SceneGraph.centralZoneX SceneGraph.centralZoneY')
    $oz = ($o -replace 'SceneGraph.centralZoneX=', '' -replace ' SceneGraph.centralZoneY=', ' ').Trim()
    "try $try`: rust $zone, original $oz"
    if ($zone) { $starts[$zone] = $true }
    if ($zone -and $zone -eq $oz) { return $r }
    Stop-Process -Id $r.Id -Force
    Start-Sleep -Milliseconds 500
}
$null
