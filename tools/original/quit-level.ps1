<#
.SYNOPSIS
Leaves the running level and returns to the main menu.

.DESCRIPTION
The game has no direct "abandon level" key (Esc restarts the level as a
replay). This script nukes the level (Alt+Q), waits for the results screen,
and picks "Menu" on it with the right mouse button. The wait covers the
bombers' countdown; raise -WaitSeconds for levels with many lemmings still
falling or walking far from the camera.
#>
param([int]$WaitSeconds = 15)
$send = Join-Path $PSScriptRoot 'send.ps1'
& $send 'LALT+Q' "wait:$($WaitSeconds * 1000)" 'rclick:320,240' 'wait:2500'
