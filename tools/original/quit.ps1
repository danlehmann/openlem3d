<#
.SYNOPSIS
Closes the DOSBox-X instance started by launch.ps1.

.DESCRIPTION
Asks the window to close (WM_CLOSE; "quit warning" is off in lem3d.conf so
DOSBox-X exits at once) and kills the process if it is still alive after
-TimeoutSeconds. The game itself is not asked to save anything.
#>
param([int]$TimeoutSeconds = 5)
. (Join-Path $PSScriptRoot 'Lem3dCommon.ps1')

$p = Get-Lem3dProcess
if (-not $p) { Write-Output 'DOSBox-X is not running.'; Remove-Item $Lem3dState -ErrorAction SilentlyContinue; return }
$p.CloseMainWindow() | Out-Null
if (-not $p.WaitForExit($TimeoutSeconds * 1000)) {
    Stop-Process -Id $p.Id -Force
    Write-Output "DOSBox-X (pid $($p.Id)) killed."
} else {
    Write-Output "DOSBox-X (pid $($p.Id)) closed."
}
Remove-Item $Lem3dState -ErrorAction SilentlyContinue
