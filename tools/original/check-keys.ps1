<#
.SYNOPSIS
Reports keys and mouse buttons Windows considers held, and releases them.

.DESCRIPTION
Synthetic input that loses its release leaves a key held system-wide, and
Windows then auto-repeats it into whatever window has focus. Run this after
any input sequence (send.ps1 does not need it for its own keys, but a killed
script can leave keys held). Prints "no keys held" or the virtual-key codes
it released.
#>
if (-not ('Lem3dKeyCheck' -as [type])) {
    Add-Type -TypeDefinition @'
using System; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class Lem3dKeyCheck {
    [DllImport("user32.dll")] static extern short GetAsyncKeyState(int vk);
    [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint type);
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] static extern void mouse_event(uint flags, int dx, int dy, uint data, IntPtr extra);
    /// Virtual-key codes currently reported as held.
    public static List<int> Held() {
        var l = new List<int>();
        for (int vk = 1; vk < 256; vk++) if ((GetAsyncKeyState(vk) & 0x8000) != 0) l.Add(vk);
        return l;
    }
    /// Releases one virtual key (or mouse button for codes 1, 2, 4).
    public static void Release(int vk) {
        if (vk == 1) { mouse_event(0x0004, 0, 0, 0, IntPtr.Zero); return; }
        if (vk == 2) { mouse_event(0x0010, 0, 0, 0, IntPtr.Zero); return; }
        if (vk == 4) { mouse_event(0x0040, 0, 0, 0, IntPtr.Zero); return; }
        keybd_event((byte)vk, (byte)MapVirtualKey((uint)vk, 0), 2, IntPtr.Zero);
    }
}
'@
}
$held = [Lem3dKeyCheck]::Held()
if ($held.Count -eq 0) { 'no keys held'; return }
foreach ($vk in $held) { [Lem3dKeyCheck]::Release($vk) }
Start-Sleep -Milliseconds 100
$still = [Lem3dKeyCheck]::Held()
"released: " + (($held | ForEach-Object { '0x{0:X2}' -f $_ }) -join ',')
if ($still.Count) { "STILL HELD: " + (($still | ForEach-Object { '0x{0:X2}' -f $_ }) -join ',') }
