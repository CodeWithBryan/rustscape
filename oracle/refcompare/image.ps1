# Image helpers for the frame comparison (System.Drawing, no other tools):
#   [RefCompare.Img]::Crop(src, dst, x, y, w, h)
#   [RefCompare.Img]::Sheet(dst, columns, scale, labelled paths...)   contact sheet
#   [RefCompare.Img]::Pair(a, b, dst, zoom)      a | b | absolute difference x4
#   [RefCompare.Img]::Compare(a, b)              size, exact-equal pixels, mean abs diff, first/last differing row and column
#   [RefCompare.Img]::Mean(path, x, y, w, h)     mean R G B of a rectangle
#   [RefCompare.Img]::Grid(a, b, cols, rows)     mean colour per cell of both and the difference
#   [RefCompare.Img]::Pixel(path, x, y)
# Dot-source: . oracle\refcompare\image.ps1
Add-Type -AssemblyName System.Drawing
if (-not ('RefCompare.Img' -as [type])) {
    Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;
namespace RefCompare {
public static class Img {
    static int[] Load(string path, out int w, out int h) {
        using (var src = new Bitmap(path)) {
            w = src.Width; h = src.Height;
            using (var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb)) {
                using (var g = Graphics.FromImage(bmp)) g.DrawImage(src, 0, 0, w, h);
                var data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
                var px = new int[w * h];
                Marshal.Copy(data.Scan0, px, 0, px.Length);
                bmp.UnlockBits(data);
                return px;
            }
        }
    }
    static void Save(int[] px, int w, int h, string path) {
        using (var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb)) {
            var data = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
            Marshal.Copy(px, 0, data.Scan0, px.Length);
            bmp.UnlockBits(data);
            using (var rgb = bmp.Clone(new Rectangle(0, 0, w, h), PixelFormat.Format24bppRgb)) rgb.Save(path, ImageFormat.Png);
        }
    }
    public static string Size(string path) { using (var b = new Bitmap(path)) return b.Width + "x" + b.Height; }
    public static void Crop(string src, string dst, int x, int y, int w, int h) {
        using (var s = new Bitmap(src)) using (var d = new Bitmap(w, h, PixelFormat.Format24bppRgb)) {
            using (var g = Graphics.FromImage(d)) g.DrawImage(s, new Rectangle(0, 0, w, h), new Rectangle(x, y, w, h), GraphicsUnit.Pixel);
            d.Save(dst, ImageFormat.Png);
        }
    }
    // Enlarges a rectangle with nearest-neighbour sampling.
    public static void Zoom(string src, string dst, int x, int y, int w, int h, int factor) {
        using (var s = new Bitmap(src)) using (var d = new Bitmap(w * factor, h * factor, PixelFormat.Format24bppRgb)) {
            using (var g = Graphics.FromImage(d)) {
                g.InterpolationMode = InterpolationMode.NearestNeighbor; g.PixelOffsetMode = PixelOffsetMode.Half;
                g.DrawImage(s, new Rectangle(0, 0, w * factor, h * factor), new Rectangle(x, y, w, h), GraphicsUnit.Pixel);
            }
            d.Save(dst, ImageFormat.Png);
        }
    }
    // items: "label|path" each.
    public static void Sheet(string dst, int columns, double scale, string[] items) {
        int cw = 0, ch = 0;
        foreach (var it in items) { using (var b = new Bitmap(it.Substring(it.IndexOf('|') + 1))) { cw = Math.Max(cw, (int)(b.Width * scale)); ch = Math.Max(ch, (int)(b.Height * scale)); } }
        int rows = (items.Length + columns - 1) / columns;
        int lh = 16;
        using (var sheet = new Bitmap(columns * (cw + 4), rows * (ch + lh + 4), PixelFormat.Format24bppRgb))
        using (var g = Graphics.FromImage(sheet)) using (var font = new Font("Consolas", 9)) {
            g.Clear(Color.FromArgb(40, 40, 40));
            g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            for (int i = 0; i < items.Length; i++) {
                int bar = items[i].IndexOf('|');
                int x = (i % columns) * (cw + 4), y = (i / columns) * (ch + lh + 4);
                g.DrawString(items[i].Substring(0, bar), font, Brushes.White, x + 2, y);
                using (var b = new Bitmap(items[i].Substring(bar + 1))) g.DrawImage(b, x, y + lh, (int)(b.Width * scale), (int)(b.Height * scale));
            }
            sheet.Save(dst, ImageFormat.Png);
        }
    }
    public static void Pair(string a, string b, string dst, int gain) {
        int aw, ah, bw, bh; var pa = Load(a, out aw, out ah); var pb = Load(b, out bw, out bh);
        int w = Math.Max(aw, bw), h = Math.Max(ah, bh);
        var o = new int[w * 3 * h];
        for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
            int ca = (x < aw && y < ah) ? pa[y * aw + x] : 0, cb = (x < bw && y < bh) ? pb[y * bw + x] : 0;
            o[y * w * 3 + x] = ca | unchecked((int)0xFF000000); o[y * w * 3 + w + x] = cb | unchecked((int)0xFF000000);
            int dr = Math.Min(255, Math.Abs(((ca >> 16) & 255) - ((cb >> 16) & 255)) * gain);
            int dg = Math.Min(255, Math.Abs(((ca >> 8) & 255) - ((cb >> 8) & 255)) * gain);
            int db = Math.Min(255, Math.Abs((ca & 255) - (cb & 255)) * gain);
            o[y * w * 3 + 2 * w + x] = unchecked((int)0xFF000000) | (dr << 16) | (dg << 8) | db;
        }
        Save(o, w * 3, h, dst);
    }
    public static string Compare(string a, string b) {
        int aw, ah, bw, bh; var pa = Load(a, out aw, out ah); var pb = Load(b, out bw, out bh);
        if (aw != bw || ah != bh) return "sizes differ: " + aw + "x" + ah + " vs " + bw + "x" + bh;
        long equal = 0, sum = 0, within8 = 0; int x0 = aw, x1 = -1, y0 = ah, y1 = -1;
        for (int y = 0; y < ah; y++) for (int x = 0; x < aw; x++) {
            int ca = pa[y * aw + x], cb = pb[y * aw + x];
            int d = Math.Abs(((ca >> 16) & 255) - ((cb >> 16) & 255)) + Math.Abs(((ca >> 8) & 255) - ((cb >> 8) & 255)) + Math.Abs((ca & 255) - (cb & 255));
            if (d == 0) equal++; else { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; }
            if (d <= 24) within8++;
            sum += d;
        }
        long n = (long)aw * ah;
        return String.Format("{0}x{1}: identical pixels {2:F2}%, within 8/channel {3:F2}%, mean abs diff per channel {4:F2}, differing box x {5}..{6} y {7}..{8}",
            aw, ah, 100.0 * equal / n, 100.0 * within8 / n, sum / (3.0 * n), x0, x1, y0, y1);
    }
    public static string Mean(string path, int x, int y, int w, int h) {
        int iw, ih; var p = Load(path, out iw, out ih);
        long r = 0, g = 0, b = 0, n = 0;
        for (int j = y; j < y + h && j < ih; j++) for (int i = x; i < x + w && i < iw; i++) { int c = p[j * iw + i]; r += (c >> 16) & 255; g += (c >> 8) & 255; b += c & 255; n++; }
        if (n == 0) return "empty";
        return String.Format("{0:F1} {1:F1} {2:F1}", (double)r / n, (double)g / n, (double)b / n);
    }
    public static string Pixel(string path, int x, int y) { int w, h; var p = Load(path, out w, out h); int c = p[y * w + x]; return ((c >> 16) & 255) + " " + ((c >> 8) & 255) + " " + (c & 255); }
    public static string Grid(string a, string b, int cols, int rows) {
        int aw, ah, bw, bh; var pa = Load(a, out aw, out ah); var pb = Load(b, out bw, out bh);
        var sb = new StringBuilder();
        for (int r = 0; r < rows; r++) {
            for (int c = 0; c < cols; c++) {
                double[] ma = Cell(pa, aw, ah, c, r, cols, rows), mb = Cell(pb, bw, bh, c, r, cols, rows);
                sb.AppendFormat("[{0},{1}] orig {2,3:F0} {3,3:F0} {4,3:F0}  rust {5,3:F0} {6,3:F0} {7,3:F0}  d {8,4:F0} {9,4:F0} {10,4:F0}\n", c, r, ma[0], ma[1], ma[2], mb[0], mb[1], mb[2], mb[0] - ma[0], mb[1] - ma[1], mb[2] - ma[2]);
            }
        }
        return sb.ToString();
    }
    static double[] Cell(int[] p, int w, int h, int c, int r, int cols, int rows) {
        int x0 = w * c / cols, x1 = w * (c + 1) / cols, y0 = h * r / rows, y1 = h * (r + 1) / rows;
        double R = 0, G = 0, B = 0; int n = 0;
        for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) { int v = p[y * w + x]; R += (v >> 16) & 255; G += (v >> 8) & 255; B += v & 255; n++; }
        return new double[] { R / n, G / n, B / n };
    }
}
}
'@
}
