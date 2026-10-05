# Shared defaults and helpers for the original-game harness. Dot-source it.
#
# Every machine-specific path can be overridden by an environment variable:
#   LEM3D_DOSBOX   path to dosbox-x.exe
#   LEM3D_CUE      path to the Lemmings 3D .cue file (default: the .cue in the game-data directory)
#   LEM3D_CDRIVE   host folder mounted as DOS drive C:
#   LEM3D_SHOTS    folder for screenshots (outside the repo, or under the git-ignored temp/)
#   LEM3D_STATE    JSON file holding the running DOSBox-X process id

$ErrorActionPreference = 'Stop'

$Lem3dRepo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Get-Lem3dSetting([string]$envName, [string]$default) {
    $v = [Environment]::GetEnvironmentVariable($envName)
    if ([string]::IsNullOrWhiteSpace($v)) { return $default }
    return $v
}

# The game needs DOSBox-X's built-in DOS (MOUNT, MSCDEX). The "osfree" builds
# lack it, so the full portable build is preferred when present.
$Lem3dDosbox = Get-Lem3dSetting 'LEM3D_DOSBOX' (Join-Path $Lem3dRepo 'temp\dosbox-full\bin\ARM64\Release\dosbox-x.exe')
# The CD image lives in the game-data directory (docs/GROUNDRULES.md):
# $OPENLEM3D_DATA, else gamedata/ in the repo; it must hold exactly one .cue.
$Lem3dDataDir = Get-Lem3dSetting 'OPENLEM3D_DATA' (Join-Path $Lem3dRepo 'gamedata')
$Lem3dDefaultCue = @(Get-ChildItem -Path $Lem3dDataDir -Filter *.cue -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName)[0]
$Lem3dCue    = Get-Lem3dSetting 'LEM3D_CUE'    $Lem3dDefaultCue
$Lem3dCDrive = Get-Lem3dSetting 'LEM3D_CDRIVE' (Join-Path $Lem3dRepo 'temp\dosbox_c')
$Lem3dShots  = Get-Lem3dSetting 'LEM3D_SHOTS'  (Join-Path $Lem3dRepo 'temp\shots')
$Lem3dState  = Get-Lem3dSetting 'LEM3D_STATE'  (Join-Path $env:TEMP 'openlem3d-dosbox-state.json')

if (-not ('Lem3dWin32' -as [type])) {
    Add-Type -Path (Join-Path $PSScriptRoot 'Lem3dWin32.cs') -ReferencedAssemblies System.Drawing
}
[Lem3dWin32]::EnsureDpiAware()

# Returns the running DOSBox-X process started by launch.ps1, or $null.
function Get-Lem3dProcess {
    if (-not (Test-Path $Lem3dState)) { return $null }
    $s = Get-Content $Lem3dState -Raw | ConvertFrom-Json
    $p = Get-Process -Id $s.pid -ErrorAction SilentlyContinue
    if ($p -and $p.ProcessName -like 'dosbox*') { return $p }
    return $null
}

# Returns the DOSBox-X main window handle; throws if it is not running.
function Get-Lem3dWindow {
    $p = Get-Lem3dProcess
    if (-not $p) { throw 'DOSBox-X is not running (start it with launch.ps1).' }
    $h = [Lem3dWin32]::FindMainWindow($p.Id)
    if ($h -eq [IntPtr]::Zero) { throw 'DOSBox-X window not found.' }
    return $h
}

function Enter-Lem3dFocus {
    $h = Get-Lem3dWindow
    # Synthetic input is refused unless this window is in the foreground.
    [Lem3dWin32]::Target = $h
    if (-not [Lem3dWin32]::Focus($h)) {
        throw 'Could not bring the DOSBox-X window to the foreground; no input sent.'
    }
    return $h
}
