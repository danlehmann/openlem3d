<#
.SYNOPSIS
Sends a sequence of keyboard and mouse actions to the DOSBox-X window.

.DESCRIPTION
Each argument is one action, executed in order:

  KEY               tap a key (scan code), e.g. ENTER, ESC, F1, A, UP, KP8, SPACE
  KEY*N             tap a key N times, e.g. DOWN*3
  A+B               chord: hold A, tap B, release, e.g. LALT+Q, LSHIFT+A
  hold:KEY:MS       hold a key for MS milliseconds, e.g. hold:KP4:800
  down:KEY / up:KEY press or release only
  type:TEXT         type text (US layout, shifted punctuation handled), e.g. type:CD \L3D
  wait:MS           sleep MS milliseconds
  move:X,Y          move the game's pointer to screen pixel X,Y (homes first)
  click:X,Y         move there and left-click; rclick:/mclick: for right/middle
  lhold:X,Y,MS      move there and hold the left button for MS ms (rhold: too)
  ldown / lup / rdown / rup   press or release a mouse button where it is
  home              push the pointer into the top-left corner (0,0)
  mrel:DX,DY        move the pointer by DX,DY screen pixels without homing
                    (e.g. between ldown and lup for a drag)
  nudge:DX,DY       raw relative motion of DX,DY host pixels

The game reads relative mouse motion only, so DOSBox-X must hold the mouse
locked (captured). Mouse actions lock it with DOSBox-X's Ctrl+F10 hotkey when
it is not locked already. Absolute positions are reached by homing the pointer
into the top-left corner, where the game clamps it, and then moving a
calibrated distance. Coordinates are pixels of the game's 640x480 screen,
whatever the window size. -PxPerHostX and -PxPerHostY are the calibration:
game pixels moved per host pixel of motion, and -PxOffsetX/-PxOffsetY the
fitted offsets (see calibrate-mouse.ps1). They depend on the game's Mouse
speed setting (calibrated at the default 8/9) and DOSBox-X's sensitivity.

.PARAMETER TapMs
How long a tapped key or click is held down. DOS games poll the keyboard, so
very short taps can be missed.

.PARAMETER GapMs
Pause after every action.
#>
[CmdletBinding(PositionalBinding = $false)]
param(
    [Parameter(Mandatory = $true, Position = 0, ValueFromRemainingArguments = $true)][string[]]$Actions,
    [int]$TapMs = 80,
    [int]$GapMs = 120,
    [double]$PxPerHostX = 1.59,
    [double]$PxPerHostY = 1.905,
    [double]$PxOffsetX = -3,
    [double]$PxOffsetY = -4
)
. (Join-Path $PSScriptRoot 'Lem3dCommon.ps1')

$h = Enter-Lem3dFocus

# US-layout characters typed with shift held.
$shifted = @{ ':' = ';'; '>' = '.'; '<' = ','; '_' = '-'; '+' = '='; '?' = '/'; '"' = "'";
              '!' = '1'; '@' = '2'; '#' = '3'; '$' = '4'; '%' = '5'; '*' = '8'; '(' = '9'; ')' = '0'; '|' = '\' }

function Tap([string]$k) {
    [Lem3dWin32]::KeyDown($k); Start-Sleep -Milliseconds $TapMs; [Lem3dWin32]::KeyUp($k)
}

function Lock-Mouse {
    # Ctrl+F10 toggles the lock, so press it only when sure it is off. After a
    # focus change SDL re-applies the clip of a lock with some delay, and
    # Alt chords (e.g. Alt+Q) drop the lock. The title is the fallback signal.
    for ($i = 0; $i -lt 6; $i++) {
        if ([Lem3dWin32]::ClipIsClient($h)) { return }
        Start-Sleep -Milliseconds 100
    }
    if ([Lem3dWin32]::TitleSaysLocked($h)) {
        # Flag set but no clip yet: a little motion makes SDL grab again.
        [Lem3dWin32]::Nudge($h, 0, 0, 1, 50)
        for ($i = 0; $i -lt 10; $i++) {
            if ([Lem3dWin32]::ClipIsClient($h)) { return }
            Start-Sleep -Milliseconds 100
        }
    }
    [Lem3dWin32]::KeyDown('LCTRL'); Start-Sleep -Milliseconds 30
    [Lem3dWin32]::KeyDown('F10'); Start-Sleep -Milliseconds 60
    [Lem3dWin32]::KeyUp('F10'); [Lem3dWin32]::KeyUp('LCTRL')
    for ($i = 0; $i -lt 30 -and -not [Lem3dWin32]::ClipIsClient($h); $i++) {
        Start-Sleep -Milliseconds 100
        if ($i -eq 5) { [Lem3dWin32]::Nudge($h, 0, 0, 1, 50) }
    }
    Start-Sleep -Milliseconds 200
    if (-not ([Lem3dWin32]::ClipIsClient($h) -or [Lem3dWin32]::TitleSaysLocked($h))) {
        Write-Warning 'Could not confirm that DOSBox-X locked the mouse; pointer motion may not reach the game.'
    }
}

function Move-Rel([double]$dx, [double]$dy) {
    Lock-Mouse
    [Lem3dWin32]::Nudge($h, [int][Math]::Round($dx / $PxPerHostX), [int][Math]::Round($dy / $PxPerHostY), 40, 30)
}

function Move-Home { Lock-Mouse; [Lem3dWin32]::Nudge($h, -1600, -1600, 100, 30) }

function Move-Abs([string]$xy) {
    $p = $xy.Split(',')
    Move-Home
    Move-Rel ([double]$p[0] - $PxOffsetX) ([double]$p[1] - $PxOffsetY)
}

function Click([string]$btn, [string]$xy, [int]$ms) {
    Move-Abs $xy; Start-Sleep -Milliseconds 60
    [Lem3dWin32]::Button($btn, $true); Start-Sleep -Milliseconds $ms
    [Lem3dWin32]::Button($btn, $false)
}

foreach ($a in $Actions) {
    # Bare words are key names (so DOWN is the arrow key), except the mouse
    # verbs below; everything else is verb:argument.
    if ($a.Contains(':') -or $a -match '^(ldown|lup|rdown|rup|home)$') {
        $verb, $rest = $a.Split(':', 2)
    } else {
        $verb, $rest = '', $null
    }
    switch -Regex ($verb.ToLowerInvariant()) {
        '^wait$'   { Start-Sleep -Milliseconds ([int]$rest); continue }
        '^type$'   {
            foreach ($c in $rest.ToCharArray()) {
                $s = [string]$c
                if ($c -eq ' ') { Tap 'SPACE' }
                elseif ($shifted.ContainsKey($s)) {
                    [Lem3dWin32]::KeyDown('LSHIFT'); Start-Sleep -Milliseconds 20
                    Tap $shifted[$s]; [Lem3dWin32]::KeyUp('LSHIFT')
                } else { Tap $s }
                Start-Sleep -Milliseconds $TapMs
            }
            continue
        }
        '^hold$'   { $k, $ms = $rest.Split(':'); [Lem3dWin32]::KeyDown($k); Start-Sleep -Milliseconds ([int]$ms); [Lem3dWin32]::KeyUp($k); continue }
        '^down$'   { [Lem3dWin32]::KeyDown($rest); continue }
        '^up$'     { [Lem3dWin32]::KeyUp($rest); continue }
        '^move$'   { Move-Abs $rest; continue }
        '^home$'   { Move-Home; continue }
        '^mrel$'   { $p = $rest.Split(','); Move-Rel ([double]$p[0]) ([double]$p[1]); continue }
        '^nudge$'  { $p = $rest.Split(','); Lock-Mouse; [Lem3dWin32]::Nudge($h, [int]$p[0], [int]$p[1], 40, 30); continue }
        '^click$'  { Click 'left' $rest $TapMs; continue }
        '^rclick$' { Click 'right' $rest $TapMs; continue }
        '^mclick$' { Click 'middle' $rest $TapMs; continue }
        '^lhold$'  { $p = $rest.Split(','); Click 'left' "$($p[0]),$($p[1])" ([int]$p[2]); continue }
        '^rhold$'  { $p = $rest.Split(','); Click 'right' "$($p[0]),$($p[1])" ([int]$p[2]); continue }
        '^ldown$'  { Lock-Mouse; [Lem3dWin32]::Button('left', $true); continue }
        '^lup$'    { [Lem3dWin32]::Button('left', $false); continue }
        '^rdown$'  { Lock-Mouse; [Lem3dWin32]::Button('right', $true); continue }
        '^rup$'    { [Lem3dWin32]::Button('right', $false); continue }
        default {
            if ($a -match '^(.+)\*(\d+)$') {
                for ($i = 0; $i -lt [int]$Matches[2]; $i++) { Tap $Matches[1]; Start-Sleep -Milliseconds $GapMs }
            } elseif ($a.Length -gt 1 -and $a.Contains('+') -and -not $a.StartsWith('KP')) {
                $keys = $a.Split('+')
                $mods = $keys[0..($keys.Length - 2)]
                foreach ($m in $mods) { [Lem3dWin32]::KeyDown($m); Start-Sleep -Milliseconds 30 }
                Tap $keys[-1]
                [array]::Reverse($mods)
                foreach ($m in $mods) { [Lem3dWin32]::KeyUp($m); Start-Sleep -Milliseconds 30 }
            } else {
                Tap $a
            }
        }
    }
    Start-Sleep -Milliseconds $GapMs
}
