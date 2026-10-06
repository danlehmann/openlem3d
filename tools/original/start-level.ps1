<#
.SYNOPSIS
From the main menu, starts a level and waits until it is playing.

.DESCRIPTION
Sequence (all verified in the running game):
  [Code]   F2, type the code, Return       (a valid code opens that level's
           briefing directly; rating and list are then skipped)
  rating   Up/Down on the menu            (Up: Practice > Fun > Tricky > Taxing > Mayhem > Practice)
  F1                                       (level list of that rating)
  click    the level's row                 (opens the level briefing)
  click    left button = Continue          (loads and starts the level)

The rating card on the main menu shows Practice after every fresh start of the
game and keeps the last choice afterwards; pass -CurrentRating when the menu
was left on another rating.

The level list only shows levels that are unlocked. Row 1 is at y=101 in the
640x480 screen and row 2 at y=135 (rows about 34 px apart); -RowY selects
another row.

.EXAMPLE
./start-level.ps1 -Rating Fun          # Fun 1 from a freshly booted menu
#>
param(
    # Practice opens a different screen and is not handled here.
    [ValidateSet('Fun', 'Tricky', 'Taxing', 'Mayhem')][string]$Rating = 'Fun',
    [ValidateSet('Practice', 'Fun', 'Tricky', 'Taxing', 'Mayhem')][string]$CurrentRating = 'Practice',
    [string]$Code,
    [int]$RowY = 101,
    [switch]$Paused
)
$send = Join-Path $PSScriptRoot 'send.ps1'
$order = @('Practice', 'Fun', 'Tricky', 'Taxing', 'Mayhem')

if ($Code) {
    $actions = @('F2', 'wait:1500', "type:$($Code.ToUpperInvariant())", 'ENTER', 'wait:3500',
        'click:320,240', 'wait:4000')
    if ($Paused) { $actions += @('P', 'wait:300') }
    & $send @actions
    return
}
# Up steps forward through the ratings, Down backward; both wrap around.
$steps = ($order.IndexOf($Rating) - $order.IndexOf($CurrentRating) + 5) % 5
$key = 'UP'
if ($steps -gt 2) { $steps = 5 - $steps; $key = 'DOWN' }
$actions = @()
for ($i = 0; $i -lt $steps; $i++) { $actions += $key; $actions += 'wait:600' }
$actions += @('F1', 'wait:2500', "click:120,$RowY", 'wait:3000')
$actions += @('click:320,240', 'wait:4000')
if ($Paused) { $actions += @('P', 'wait:300') }
& $send @actions
