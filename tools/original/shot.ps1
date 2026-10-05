<#
.SYNOPSIS
Saves a PNG of the DOSBox-X window's client area (the emulated screen).

.PARAMETER Name
File name without extension. Default: a timestamp. Saved under LEM3D_SHOTS.

.PARAMETER Path
Full output path; overrides -Name.

.PARAMETER Width
If set, scales the image down to this width (keeps the aspect ratio).
Handy for quick looks; full size is the default.

.PARAMETER Method
print: ask the window to render itself (works while covered, no focus
change). screen: copy the desktop pixels (focuses the window first). auto
(default): print, falling back to screen.
#>
param(
    [string]$Name,
    [string]$Path,
    [int]$Width = 0,
    [ValidateSet('auto', 'print', 'screen')][string]$Method = 'auto'
)
. (Join-Path $PSScriptRoot 'Lem3dCommon.ps1')

if (-not $Path) {
    if (-not $Name) { $Name = 'shot-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff') }
    New-Item -ItemType Directory -Force $Lem3dShots | Out-Null
    $Path = Join-Path $Lem3dShots ($Name + '.png')
}
$repoFull = [IO.Path]::GetFullPath($Lem3dRepo).TrimEnd('\') + '\'
if ([IO.Path]::GetFullPath($Path).StartsWith($repoFull, [StringComparison]::OrdinalIgnoreCase) -and
    -not [IO.Path]::GetFullPath($Path).StartsWith($repoFull + 'temp\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to write a screenshot of the original game inside the repo: $Path"
}

$h = Get-Lem3dWindow
$used = $null
if ($Method -ne 'screen') {
    try { $used = [Lem3dWin32]::GrabClient($h, $Path, 'print') } catch { if ($Method -eq 'print') { throw } }
}
if (-not $used) {
    # A screen copy needs the window on top.
    $h = Enter-Lem3dFocus
    Start-Sleep -Milliseconds 150
    $used = [Lem3dWin32]::GrabClient($h, $Path, 'screen')
}

if ($Width -gt 0) {
    Add-Type -AssemblyName System.Drawing
    $src = [System.Drawing.Image]::FromFile($Path)
    $hgt = [int]($src.Height * $Width / $src.Width)
    $dst = New-Object System.Drawing.Bitmap $Width, $hgt
    $g = [System.Drawing.Graphics]::FromImage($dst)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.DrawImage($src, 0, 0, $Width, $hgt)
    $g.Dispose(); $src.Dispose()
    $dst.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png); $dst.Dispose()
}
Write-Output $Path
