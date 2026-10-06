<#
.SYNOPSIS
Saves or restores a named DOSBox-X save state of the running original game.

.DESCRIPTION
Uses DOSBox-X's own save states through its menu bar (menu.ps1 posts window
messages), so no keys are typed and the desktop may be locked. DOSBox-X
always saves to and loads from temp/states/current.sav (configured by
launch.ps1); named states are copies of it in temp/states/<name>.sav.

  state.ps1 -Save fun1-paused    # snapshot the running game
  state.ps1 -Load fun1-paused    # restore it (game must already be running)
  state.ps1 -List                # show saved states

States are only valid for the same DOSBox-X build and configuration.
#>
param(
    [string]$Save,
    [string]$Load,
    [switch]$List
)
. (Join-Path $PSScriptRoot 'Lem3dCommon.ps1')
$dir = Join-Path $Lem3dRepo 'temp\states'
New-Item -ItemType Directory -Force $dir | Out-Null
$current = Join-Path $dir 'current.sav'
$menu = Join-Path $PSScriptRoot 'menu.ps1'

if ($List) {
    Get-ChildItem $dir -Filter *.sav | Where-Object Name -ne 'current.sav' |
        ForEach-Object { '{0,-30} {1:yyyy-MM-dd HH:mm}' -f $_.BaseName, $_.LastWriteTime }
    return
}
if ($Save) {
    $before = if (Test-Path $current) { (Get-Item $current).LastWriteTimeUtc } else { [datetime]::MinValue }
    & $menu -Item 'Save state' | Out-Null
    # Wait until DOSBox-X has finished writing the new state.
    for ($i = 0; $i -lt 100; $i++) {
        Start-Sleep -Milliseconds 100
        if ((Test-Path $current) -and (Get-Item $current).LastWriteTimeUtc -gt $before) { break }
    }
    Start-Sleep -Milliseconds 500
    if (-not (Test-Path $current) -or (Get-Item $current).LastWriteTimeUtc -le $before) { throw 'DOSBox-X did not write a save state.' }
    Copy-Item $current (Join-Path $dir "$Save.sav") -Force
    "saved state '$Save'"
    return
}
if ($Load) {
    $src = Join-Path $dir "$Load.sav"
    if (-not (Test-Path $src)) { throw "No saved state '$Load' (see state.ps1 -List)." }
    Copy-Item $src $current -Force
    & $menu -Item 'Load state' | Out-Null
    Start-Sleep -Milliseconds 1500
    "loaded state '$Load'"
    return
}
throw 'Pass -Save <name>, -Load <name> or -List.'
