# Window helpers for the frame comparison: find a process's windows, grab a
# window's pixels with PrintWindow (works while the window is covered), and
# post mouse/keyboard messages to a window handle. Dot-source this file:
#   . oracle\refcompare\win.ps1
# Only ever pass handles of windows that belong to a process you started.
Add-Type -AssemblyName System.Drawing
if (-not ('RefCompare.Win' -as [type])) {
    Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
namespace RefCompare {
public static class Win {
    delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent, EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr hwnd, StringBuilder sb, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder sb, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out RECT r);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hwnd, ref POINT p);
    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }

    static string Cls(IntPtr h) { var sb = new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
    static string Txt(IntPtr h) { var sb = new StringBuilder(256); GetWindowText(h, sb, 256); return sb.ToString(); }

    // "handle|class|title|visible|client w x h|screen x,y of client origin"
    public static string Describe(IntPtr h) {
        RECT c; GetClientRect(h, out c); POINT p = new POINT(); ClientToScreen(h, ref p);
        return h.ToInt64() + "|" + Cls(h) + "|" + Txt(h) + "|" + (IsWindowVisible(h) ? "visible" : "hidden") + "|" + c.R + "x" + c.B + "|" + p.X + "," + p.Y;
    }
    public static List<string> Windows(uint pid) {
        var res = new List<string>();
        EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) res.Add(Describe(h)); return true; }, IntPtr.Zero);
        return res;
    }
    public static List<string> Children(IntPtr parent) {
        var res = new List<string>();
        EnumChildWindows(parent, (h, l) => { res.Add(Describe(h)); return true; }, IntPtr.Zero);
        return res;
    }
    // flags: 2 = PW_RENDERFULLCONTENT (whole window), 3 = client area only.
    public static bool Grab(IntPtr h, string path, uint flags) {
        RECT r; if (flags == 3) GetClientRect(h, out r); else { GetWindowRect(h, out r); r.R -= r.L; r.B -= r.T; }
        if (r.R <= 0 || r.B <= 0) return false;
        using (var bmp = new Bitmap(r.R, r.B, PixelFormat.Format32bppArgb)) {
            bool ok;
            using (var g = Graphics.FromImage(bmp)) { IntPtr dc = g.GetHdc(); ok = PrintWindow(h, dc, flags); g.ReleaseHdc(dc); }
            using (var rgb = bmp.Clone(new Rectangle(0, 0, r.R, r.B), PixelFormat.Format24bppRgb)) rgb.Save(path, ImageFormat.Png);
            return ok;
        }
    }
    static IntPtr L(int x, int y) { return (IntPtr)((y << 16) | (x & 0xFFFF)); }
    public static void Move(IntPtr h, int x, int y) { PostMessage(h, 0x0200, IntPtr.Zero, L(x, y)); }
    public static void Down(IntPtr h, int x, int y, bool right) { PostMessage(h, right ? 0x0204u : 0x0201u, (IntPtr)(right ? 2 : 1), L(x, y)); }
    public static void Up(IntPtr h, int x, int y, bool right) { PostMessage(h, right ? 0x0205u : 0x0202u, IntPtr.Zero, L(x, y)); }
    [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint type);
    // miniquad reads the key from the scan code in lParam, so it must be right.
    // The arrow and navigation keys are extended keys (lParam bit 24); without
    // the bit the same scan codes are the numeric keypad's.
    static int Scan(int vk, int scan) { if (scan == 0) scan = (int)MapVirtualKey((uint)vk, 0); if ((vk >= 0x21 && vk <= 0x28) || vk == 0x2D || vk == 0x2E) scan |= 0x100; return scan; }
    public static void KeyDown(IntPtr h, int vk, int scan) { scan = Scan(vk, scan); PostMessage(h, 0x0100, (IntPtr)vk, (IntPtr)(1 | (scan << 16))); }
    public static void KeyUp(IntPtr h, int vk, int scan) { scan = Scan(vk, scan); PostMessage(h, 0x0101, (IntPtr)vk, (IntPtr)(1 | (scan << 16) | (1 << 30) | (1 << 31))); }
    public static void Char(IntPtr h, char c) { PostMessage(h, 0x0102, (IntPtr)c, (IntPtr)1); }
}
}
'@
}
[void][RefCompare.Win]::SetProcessDPIAware()
