// Win32 helpers for driving the DOSBox-X window: focus, scan-code keyboard
// input, mouse input and window grabs. Compiled at run time by
// Lem3dCommon.ps1 via Add-Type.
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class Lem3dWin32
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int X, Y; }

    [StructLayout(LayoutKind.Sequential)]
    struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Sequential)]
    struct KEYBDINPUT { public ushort wVk, wScan; public uint dwFlags, time; public IntPtr dwExtraInfo; }

    [StructLayout(LayoutKind.Explicit)]
    struct INPUTUNION
    {
        [FieldOffset(0)] public MOUSEINPUT mi;
        [FieldOffset(0)] public KEYBDINPUT ki;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct INPUT { public uint type; public INPUTUNION u; }

    const uint INPUT_MOUSE = 0, INPUT_KEYBOARD = 1;
    const uint KEYEVENTF_EXTENDEDKEY = 0x1, KEYEVENTF_KEYUP = 0x2, KEYEVENTF_SCANCODE = 0x8;
    const uint MOUSEEVENTF_MOVE = 0x1, MOUSEEVENTF_LEFTDOWN = 0x2, MOUSEEVENTF_LEFTUP = 0x4,
               MOUSEEVENTF_RIGHTDOWN = 0x8, MOUSEEVENTF_RIGHTUP = 0x10,
               MOUSEEVENTF_MIDDLEDOWN = 0x20, MOUSEEVENTF_MIDDLEUP = 0x40,
               MOUSEEVENTF_ABSOLUTE = 0x8000, MOUSEEVENTF_VIRTUALDESK = 0x4000;

    [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr v);
    [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder sb, int max);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder sb, int max);
    delegate bool EnumProc(IntPtr h, IntPtr l);

    static bool dpiSet;
    /// Makes this process per-monitor DPI aware so that all coordinates are
    /// physical pixels.
    public static void EnsureDpiAware()
    {
        if (dpiSet) return;
        dpiSet = true;
        try { SetProcessDpiAwarenessContext(new IntPtr(-4)); } catch { }
    }

    /// The largest visible top-level window owned by the process, or zero.
    public static IntPtr FindMainWindow(int pid)
    {
        IntPtr best = IntPtr.Zero;
        long bestArea = 0;
        EnumWindows((h, l) =>
        {
            uint p;
            GetWindowThreadProcessId(h, out p);
            if (p == (uint)pid && IsWindowVisible(h))
            {
                RECT r;
                GetClientRect(h, out r);
                long area = (long)(r.Right - r.Left) * (r.Bottom - r.Top);
                if (area > bestArea) { bestArea = area; best = h; }
            }
            return true;
        }, IntPtr.Zero);
        return best;
    }

    public static string WindowTitle(IntPtr h)
    {
        var sb = new StringBuilder(512);
        GetWindowText(h, sb, sb.Capacity);
        return sb.ToString();
    }

    /// Brings the window to the foreground, working around the foreground lock.
    public static bool Focus(IntPtr h)
    {
        EnsureDpiAware();
        if (IsIconic(h)) ShowWindow(h, 9 /* SW_RESTORE */);
        if (GetForegroundWindow() == h) return true;
        uint dummy;
        uint fgThread = GetWindowThreadProcessId(GetForegroundWindow(), out dummy);
        uint me = GetCurrentThreadId();
        // A synthetic ALT tap lets this process take the foreground.
        keybd_event(0x12, 0, 0, IntPtr.Zero);
        keybd_event(0x12, 0, KEYEVENTF_KEYUP, IntPtr.Zero);
        AttachThreadInput(me, fgThread, true);
        BringWindowToTop(h);
        SetForegroundWindow(h);
        AttachThreadInput(me, fgThread, false);
        for (int i = 0; i < 20 && GetForegroundWindow() != h; i++) Thread.Sleep(25);
        return GetForegroundWindow() == h;
    }

    /// Client area in screen pixels: x, y, width, height.
    public static int[] ClientRectOnScreen(IntPtr h)
    {
        EnsureDpiAware();
        RECT r; GetClientRect(h, out r);
        POINT p = new POINT { X = 0, Y = 0 };
        ClientToScreen(h, ref p);
        return new int[] { p.X, p.Y, r.Right - r.Left, r.Bottom - r.Top };
    }

    // ---- keyboard -------------------------------------------------------

    /// Set-1 scan codes; values >= 0x100 are E0-extended keys.
    public static readonly Dictionary<string, int> Scan = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase)
    {
        {"ESC",0x01},{"1",0x02},{"2",0x03},{"3",0x04},{"4",0x05},{"5",0x06},{"6",0x07},{"7",0x08},{"8",0x09},{"9",0x0A},{"0",0x0B},
        {"MINUS",0x0C},{"-",0x0C},{"EQUALS",0x0D},{"=",0x0D},{"BACKSPACE",0x0E},{"BKSP",0x0E},{"TAB",0x0F},
        {"Q",0x10},{"W",0x11},{"E",0x12},{"R",0x13},{"T",0x14},{"Y",0x15},{"U",0x16},{"I",0x17},{"O",0x18},{"P",0x19},
        {"LBRACKET",0x1A},{"[",0x1A},{"RBRACKET",0x1B},{"]",0x1B},{"ENTER",0x1C},{"RETURN",0x1C},{"LCTRL",0x1D},{"CTRL",0x1D},
        {"A",0x1E},{"S",0x1F},{"D",0x20},{"F",0x21},{"G",0x22},{"H",0x23},{"J",0x24},{"K",0x25},{"L",0x26},
        {"SEMICOLON",0x27},{";",0x27},{"APOSTROPHE",0x28},{"'",0x28},{"GRAVE",0x29},{"`",0x29},{"LSHIFT",0x2A},{"SHIFT",0x2A},
        {"BACKSLASH",0x2B},{"\\",0x2B},{"Z",0x2C},{"X",0x2D},{"C",0x2E},{"V",0x2F},{"B",0x30},{"N",0x31},{"M",0x32},
        {"COMMA",0x33},{",",0x33},{"PERIOD",0x34},{".",0x34},{"SLASH",0x35},{"/",0x35},{"RSHIFT",0x36},{"KP*",0x37},
        {"LALT",0x38},{"ALT",0x38},{"SPACE",0x39},{"CAPSLOCK",0x3A},
        {"F1",0x3B},{"F2",0x3C},{"F3",0x3D},{"F4",0x3E},{"F5",0x3F},{"F6",0x40},{"F7",0x41},{"F8",0x42},{"F9",0x43},{"F10",0x44},
        {"NUMLOCK",0x45},{"SCROLLLOCK",0x46},
        {"KP7",0x47},{"KP8",0x48},{"KP9",0x49},{"KP-",0x4A},{"KP4",0x4B},{"KP5",0x4C},{"KP6",0x4D},{"KP+",0x4E},
        {"KP1",0x4F},{"KP2",0x50},{"KP3",0x51},{"KP0",0x52},{"KP.",0x53},{"F11",0x57},{"F12",0x58},
        {"KPENTER",0x11C},{"RCTRL",0x11D},{"KP/",0x135},{"RALT",0x138},
        {"HOME",0x147},{"UP",0x148},{"PGUP",0x149},{"LEFT",0x14B},{"RIGHT",0x14D},{"END",0x14F},{"DOWN",0x150},
        {"PGDN",0x151},{"INSERT",0x152},{"INS",0x152},{"DELETE",0x153},{"DEL",0x153},
    };

    static INPUT KeyInput(int sc, bool up)
    {
        var i = new INPUT { type = INPUT_KEYBOARD };
        i.u.ki.wScan = (ushort)(sc & 0xFF);
        i.u.ki.dwFlags = KEYEVENTF_SCANCODE | (up ? KEYEVENTF_KEYUP : 0) | ((sc & 0x100) != 0 ? KEYEVENTF_EXTENDEDKEY : 0);
        return i;
    }

    public static int ScanOf(string name)
    {
        int sc;
        if (!Scan.TryGetValue(name, out sc)) throw new ArgumentException("Unknown key name: " + name);
        return sc;
    }

    /// The window that synthetic input is meant for. When set, input is only
    /// sent while that window is in the foreground, so keystrokes never land
    /// in another application.
    public static IntPtr Target = IntPtr.Zero;

    /// Sends a press or move, but only while the target window has focus.
    static void Send(INPUT i)
    {
        if (Target != IntPtr.Zero && GetForegroundWindow() != Target && !Focus(Target))
            throw new InvalidOperationException("DOSBox-X is not the foreground window; input not sent.");
        SendInput(1, new[] { i }, Marshal.SizeOf(typeof(INPUT)));
    }

    /// Sends a release unconditionally: a release must never be withheld,
    /// or the key stays held system-wide and auto-repeats into whatever
    /// window has focus.
    static void SendRelease(INPUT i)
    {
        SendInput(1, new[] { i }, Marshal.SizeOf(typeof(INPUT)));
    }

    /// Keys and mouse buttons currently held down by this process.
    static readonly HashSet<string> HeldKeys = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
    static readonly HashSet<string> HeldButtons = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

    public static void KeyDown(string name)
    {
        Send(KeyInput(ScanOf(name), false));
        HeldKeys.Add(name);
    }

    public static void KeyUp(string name)
    {
        SendRelease(KeyInput(ScanOf(name), true));
        HeldKeys.Remove(name);
    }

    /// Releases every key and mouse button still held by this process.
    public static void ReleaseAll()
    {
        foreach (var k in new List<string>(HeldKeys)) KeyUp(k);
        foreach (var b in new List<string>(HeldButtons)) Button(b, false);
    }

    // ---- mouse ----------------------------------------------------------

    /// Moves the host cursor to an absolute screen pixel.
    public static void MoveTo(int x, int y)
    {
        EnsureDpiAware();
        SetCursorPos(x, y);
        // Follow up with a zero relative move so the target sees WM_MOUSEMOVE / raw input.
        var i = new INPUT { type = INPUT_MOUSE };
        i.u.mi.dwFlags = MOUSEEVENTF_MOVE;
        Send(i);
    }

    public static void Button(string which, bool down)
    {
        uint f;
        switch (which.ToLowerInvariant())
        {
            case "left": f = down ? MOUSEEVENTF_LEFTDOWN : MOUSEEVENTF_LEFTUP; break;
            case "right": f = down ? MOUSEEVENTF_RIGHTDOWN : MOUSEEVENTF_RIGHTUP; break;
            case "middle": f = down ? MOUSEEVENTF_MIDDLEDOWN : MOUSEEVENTF_MIDDLEUP; break;
            default: throw new ArgumentException("Unknown button: " + which);
        }
        var i = new INPUT { type = INPUT_MOUSE };
        i.u.mi.dwFlags = f;
        if (down)
        {
            Send(i);
            HeldButtons.Add(which);
        }
        else
        {
            SendRelease(i);
            HeldButtons.Remove(which);
        }
    }

    [DllImport("user32.dll")] static extern bool GetClipCursor(out RECT r);

    /// True when DOSBox-X's title bar offers "[Ctrl+F10 releases mouse]",
    /// i.e. its lock flag is set. The title can lag a toggle by seconds.
    public static bool TitleSaysLocked(IntPtr h)
    {
        return WindowTitle(h).IndexOf("releases mouse", StringComparison.OrdinalIgnoreCase) >= 0;
    }

    /// True when the host cursor is clipped to the client area, which is
    /// how a locked SDL window holds the mouse. Only meaningful while the
    /// window has focus; the clip follows a toggle immediately.
    public static bool ClipIsClient(IntPtr h)
    {
        EnsureDpiAware();
        RECT c; GetClipCursor(out c);
        int[] r = ClientRectOnScreen(h);
        return Math.Abs(c.Left - r[0]) <= 2 && Math.Abs(c.Top - r[1]) <= 2 &&
               Math.Abs(c.Right - (r[0] + r[2])) <= 2 && Math.Abs(c.Bottom - (r[1] + r[3])) <= 2;
    }

    /// Relative motion for a locked DOSBox-X mouse. SDL re-centres the host
    /// cursor after every motion event and reports the offset from the
    /// centre, so placing the cursor at centre+(dx,dy) yields an exact,
    /// unaccelerated delta of (dx,dy) host pixels. Large deltas are split
    /// into steps of at most maxStep pixels.
    public static void Nudge(IntPtr h, int dx, int dy, int maxStep, int stepMs)
    {
        int[] r = ClientRectOnScreen(h);
        int cx = r[0] + r[2] / 2, cy = r[1] + r[3] / 2;
        WaitCentred(cx, cy, 300);
        while (dx != 0 || dy != 0)
        {
            int sx = Math.Max(-maxStep, Math.Min(maxStep, dx));
            int sy = Math.Max(-maxStep, Math.Min(maxStep, dy));
            MoveTo(cx + sx, cy + sy);
            // A step only counts once SDL has consumed it and warped back;
            // moving again earlier would coalesce the two motion events.
            WaitCentred(cx, cy, 500);
            Thread.Sleep(stepMs);
            dx -= sx; dy -= sy;
        }
    }

    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);

    static bool WaitCentred(int cx, int cy, int timeoutMs)
    {
        var sw = Stopwatch.StartNew();
        POINT p;
        while (sw.ElapsedMilliseconds < timeoutMs)
        {
            GetCursorPos(out p);
            if (Math.Abs(p.X - cx) <= 1 && Math.Abs(p.Y - cy) <= 1) return true;
            Thread.Sleep(5);
        }
        return false;
    }

    /// Bounding box {minX, minY, maxX, maxY} of the pixels that differ
    /// between two same-sized images, ignoring the rectangle
    /// [0,ignoreW) x [0,ignoreH); null if none differ.
    public static int[] DiffBox(string a, string b, int ignoreW, int ignoreH)
    {
        using (var A = new Bitmap(a))
        using (var B = new Bitmap(b))
        {
            int w = A.Width, hgt = A.Height;
            var ra = A.LockBits(new Rectangle(0, 0, w, hgt), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            var rb = B.LockBits(new Rectangle(0, 0, w, hgt), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            int[] pa = new int[w * hgt], pb = new int[w * hgt];
            Marshal.Copy(ra.Scan0, pa, 0, pa.Length);
            Marshal.Copy(rb.Scan0, pb, 0, pb.Length);
            A.UnlockBits(ra); B.UnlockBits(rb);
            int minX = int.MaxValue, minY = int.MaxValue, maxX = -1, maxY = -1;
            for (int y = 0; y < hgt; y++)
                for (int x = 0; x < w; x++)
                {
                    if (x < ignoreW && y < ignoreH) continue;
                    if (pa[y * w + x] != pb[y * w + x])
                    {
                        if (x < minX) minX = x; if (x > maxX) maxX = x;
                        if (y < minY) minY = y; if (y > maxY) maxY = y;
                    }
                }
            return maxX < 0 ? null : new[] { minX, minY, maxX, maxY };
        }
    }

    // ---- capture --------------------------------------------------------

    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);

    /// Renders the window's client area into a PNG file. "print" asks the
    /// window to paint itself (PrintWindow with PW_RENDERFULLCONTENT), which
    /// works even when other windows cover it; "screen" copies the pixels
    /// from the desktop and needs the window on top. Returns the method used.
    public static string GrabClient(IntPtr h, string path, string method)
    {
        int[] r = ClientRectOnScreen(h);
        using (var bmp = new Bitmap(r[2], r[3], PixelFormat.Format32bppArgb))
        {
            bool ok = false;
            if (method != "screen")
            {
                using (var g = Graphics.FromImage(bmp))
                {
                    IntPtr hdc = g.GetHdc();
                    ok = PrintWindow(h, hdc, 0x1 | 0x2);
                    g.ReleaseHdc(hdc);
                }
                ok = ok && !IsBlank(bmp);
            }
            if (!ok)
            {
                if (method == "print") throw new InvalidOperationException("PrintWindow produced no image.");
                using (var g = Graphics.FromImage(bmp))
                    g.CopyFromScreen(r[0], r[1], 0, 0, new Size(r[2], r[3]), CopyPixelOperation.SourceCopy);
                method = "screen";
            }
            else method = "print";
            using (var rgb = bmp.Clone(new Rectangle(0, 0, bmp.Width, bmp.Height), PixelFormat.Format24bppRgb))
                rgb.Save(path, ImageFormat.Png);
        }
        return method;
    }

    public static void GrabClient(IntPtr h, string path) { GrabClient(h, path, "auto"); }

    static bool IsBlank(Bitmap b)
    {
        // A sparse sample: an all-black or all-transparent result means the
        // window did not paint (e.g. hardware-composited output).
        for (int y = 0; y < b.Height; y += Math.Max(1, b.Height / 24))
            for (int x = 0; x < b.Width; x += Math.Max(1, b.Width / 32))
                if ((b.GetPixel(x, y).ToArgb() & 0xFFFFFF) != 0) return false;
        return true;
    }
}
