<#
.SYNOPSIS
Grabs a timed series of screenshots of the emulated screen.

.DESCRIPTION
Saves frames as <Name>-0000.png, <Name>-0001.png, ... under LEM3D_SHOTS (or
-Dir) and writes <Name>.csv with each frame's grab time in milliseconds
after the first. Frames are kept in memory until the series ends, so the
interval holds down to roughly 30-50 ms per frame. Uses the print method, so
the window may be covered.

.EXAMPLE
./burst.ps1 -Name walk -Count 60 -IntervalMs 250
#>
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [int]$Count = 20,
    [int]$IntervalMs = 500,
    # Output width in pixels; 640 gives one pixel per game pixel.
    [int]$Width = 640,
    [string]$Dir
)
. (Join-Path $PSScriptRoot 'Lem3dCommon.ps1')
if (-not $Dir) { $Dir = $Lem3dShots }
New-Item -ItemType Directory -Force $Dir | Out-Null
$repoFull = [IO.Path]::GetFullPath($Lem3dRepo).TrimEnd('\') + '\'
$dirFull = [IO.Path]::GetFullPath($Dir).TrimEnd('\') + '\'
if ($dirFull.StartsWith($repoFull, [StringComparison]::OrdinalIgnoreCase) -and
    -not $dirFull.StartsWith($repoFull + 'temp\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to write screenshots of the original game inside the repo: $Dir"
}
$h = Get-Lem3dWindow
$prefix = Join-Path $Dir $Name
$times = [Lem3dWin32]::GrabSeries($h, $prefix, $Count, $IntervalMs, $Width)
$lines = @('frame,ms')
for ($i = 0; $i -lt $times.Length; $i++) { $lines += "$i,$($times[$i])" }
$lines | Set-Content -Encoding ascii ($prefix + '.csv')
Write-Output "$($times.Length) frames, last at $($times[-1]) ms: $prefix-*.png"
