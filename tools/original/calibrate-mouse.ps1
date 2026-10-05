<#
.SYNOPSIS
Measures how far the game's pointer moves per host pixel of mouse motion.

.DESCRIPTION
Run it on a static game screen (e.g. the Options screen, F12 on the main
menu) where nothing animates. For each test distance it homes the pointer,
grabs the screen, moves the pointer by a known number of host pixels, grabs
again, and locates the pointer from the difference of the two images. It
prints the fitted game-pixels-per-host-pixel factors (and offset) to pass to
send.ps1 as -PxPerHostX / -PxPerHostY.
#>
param([int[]]$Steps = @(60, 120, 200, 300))
. (Join-Path $PSScriptRoot 'Lem3dCommon.ps1')
Add-Type -AssemblyName System.Drawing

$h = Enter-Lem3dFocus
$send = Join-Path $PSScriptRoot 'send.ps1'
$tmp = [IO.Path]::GetTempPath()

function Grab([string]$name) {
    $p = Join-Path $tmp "lem3d-cal-$name.png"
    [Lem3dWin32]::GrabClient($h, $p)
    return $p
}

# Bounding box centre (in 640x480 game pixels) of pixels that differ.
function Diff-Centre([string]$imgA, [string]$imgB) {
    $r = [Lem3dWin32]::ClientRectOnScreen($h)
    $sx = $r[2] / 640.0; $sy = $r[3] / 480.0
    # Skip the top-left corner where the homed pointer sits in the first image.
    $box = [Lem3dWin32]::DiffBox($imgA, $imgB, [int](40 * $sx), [int](40 * $sy))
    if (-not $box) { return $null }
    return @(((($box[0] + $box[2]) / 2) / $sx), ((($box[1] + $box[3]) / 2) / $sy))
}

$rows = @()
foreach ($s in $Steps) {
    & $send home 'wait:300' | Out-Null
    $a = Grab 'a'
    & $send "nudge:$s,$([int]($s * 0.75))" 'wait:300' | Out-Null
    $b = Grab 'b'
    $c = Diff-Centre $a $b
    if ($c) {
        $rows += [pscustomobject]@{ hostX = $s; hostY = [int]($s * 0.75); gameX = [math]::Round($c[0], 1); gameY = [math]::Round($c[1], 1) }
    }
}
$rows | Format-Table | Out-String | Write-Output

function Fit($xs, $ys) {
    $n = $xs.Count; $mx = ($xs | Measure-Object -Average).Average; $my = ($ys | Measure-Object -Average).Average
    $num = 0; $den = 0
    for ($i = 0; $i -lt $n; $i++) { $num += ($xs[$i] - $mx) * ($ys[$i] - $my); $den += ($xs[$i] - $mx) * ($xs[$i] - $mx) }
    $k = $num / $den
    return @([math]::Round($k, 4), [math]::Round($my - $k * $mx, 1))
}
if ($rows.Count -ge 2) {
    $fx = Fit @($rows.hostX) @($rows.gameX); $fy = Fit @($rows.hostY) @($rows.gameY)
    Write-Output ("PxPerHostX = {0} (offset {1}),  PxPerHostY = {2} (offset {3})" -f $fx[0], $fx[1], $fy[0], $fy[1])
}
