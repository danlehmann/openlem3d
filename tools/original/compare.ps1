<#
.SYNOPSIS
Renders openlem3d next to captures of the original for side-by-side review.

.DESCRIPTION
For every capture named LEVEL_nnn_camK.png in -Dir (default
temp/shots/compare), renders openlem3d at 640x480 from the same level file
and preset camera (without HUD), and writes side_LEVEL_nnn_camK.png with the
original on the left and openlem3d on the right. All images stay in the
git-ignored temp folder.
#>
param(
    [string]$Dir = (Join-Path $PSScriptRoot '..\..\temp\shots\compare'),
    [string]$Filter = 'LEVEL_*_cam*.png'
)
# Native tools log to stderr; Windows PowerShell 5.1 would treat that as an error under 'Stop'.
$ErrorActionPreference = 'Continue'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$viewer = Join-Path $repo 'target\release\openlem3d.exe'
$tool = Join-Path $repo 'target\release\l3d-tool.exe'
if (-not (Test-Path $viewer) -or -not (Test-Path $tool)) {
    & cargo build --release -p openlem3d -p l3d-tool --manifest-path (Join-Path $repo 'Cargo.toml')
}
Get-ChildItem -Path $Dir -Filter $Filter | Where-Object { $_.Name -match '^LEVEL_(\d{3})_cam([1-4])\.png$' } | ForEach-Object {
    $level = [int]$Matches[1]; $cam = [int]$Matches[2]
    $ours = Join-Path $Dir ('ours_' + $_.Name)
    $side = Join-Path $Dir ('side_' + $_.Name)
    & $viewer --data (Join-Path $repo 'gamedata') --level $level --camera $cam --size 640x480 --wait 0.6 --no-hud --screenshot $ours 2>$null | Out-Null
    & $tool img-montage --rect 0,0,640,480 --per-row 2 --out $side $_.FullName $ours 2>$null | Out-Null
    $side
}
