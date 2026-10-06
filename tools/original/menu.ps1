<#
.SYNOPSIS
Lists or invokes DOSBox-X menu commands by sending window messages.

.DESCRIPTION
DOSBox-X has a native Windows menu bar. This script walks it and posts a
WM_COMMAND for an item, so commands such as "Save state" and "Load state"
run without keyboard input, without bringing the window to the front, and
even while the desktop is locked. Nothing is typed into any window.

  menu.ps1 -List                 # print every menu item with its id
  menu.ps1 -Item 'Load state'    # invoke the first item whose text matches

Item matching ignores case, '&' accelerators and trailing shortcut text
after a tab.
#>
param(
    [switch]$List,
    [string]$Item
)
. (Join-Path $PSScriptRoot 'Lem3dCommon.ps1')

if (-not ('Lem3dMenu' -as [type])) {
    Add-Type -TypeDefinition @'
using System; using System.Collections.Generic; using System.Runtime.InteropServices; using System.Text;
public static class Lem3dMenu {
    [DllImport("user32.dll")] static extern IntPtr GetMenu(IntPtr h);
    [DllImport("user32.dll")] static extern int GetMenuItemCount(IntPtr m);
    [DllImport("user32.dll")] static extern IntPtr GetSubMenu(IntPtr m, int pos);
    [DllImport("user32.dll")] static extern uint GetMenuItemID(IntPtr m, int pos);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetMenuString(IntPtr m, uint item, StringBuilder s, int max, uint flags);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    const uint MF_BYPOSITION = 0x400, WM_COMMAND = 0x111;

    /// Every leaf menu item as "path<TAB>id".
    public static List<string> Items(IntPtr window) {
        var list = new List<string>();
        Walk(GetMenu(window), "", list);
        return list;
    }

    static void Walk(IntPtr menu, string path, List<string> list) {
        if (menu == IntPtr.Zero) return;
        int n = GetMenuItemCount(menu);
        for (int i = 0; i < n; i++) {
            var sb = new StringBuilder(256);
            GetMenuString(menu, (uint)i, sb, sb.Capacity, MF_BYPOSITION);
            string name = sb.ToString();
            IntPtr sub = GetSubMenu(menu, i);
            if (sub != IntPtr.Zero) Walk(sub, path + name + " > ", list);
            else if (name.Length > 0) list.Add(path + name + "\t" + GetMenuItemID(menu, i));
        }
    }

    public static void Invoke(IntPtr window, uint id) {
        PostMessage(window, WM_COMMAND, (IntPtr)id, IntPtr.Zero);
    }
}
'@
}

function Normalize([string]$s) { ($s -split "`t")[0].Replace('&', '').Trim().ToLowerInvariant() }

$h = Get-Lem3dWindow
$items = [Lem3dMenu]::Items($h) | ForEach-Object {
    $parts = $_ -split "`t"
    [pscustomobject]@{ Path = $parts[0]; Id = [uint32]$parts[-1]; Name = Normalize(($parts[0] -split ' > ')[-1]) }
}
if ($List) { $items | ForEach-Object { '{0,6}  {1}' -f $_.Id, $_.Path }; return }
if (-not $Item) { throw 'Pass -List or -Item <text>.' }
$want = Normalize $Item
$hit = $items | Where-Object { $_.Name -eq $want } | Select-Object -First 1
if (-not $hit) { $hit = $items | Where-Object { $_.Name -like "*$want*" } | Select-Object -First 1 }
if (-not $hit) { throw "No menu item matches '$Item' (use -List)." }
[Lem3dMenu]::Invoke($h, $hit.Id)
"invoked: $($hit.Path) (id $($hit.Id))"
