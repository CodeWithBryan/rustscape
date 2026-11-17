package rt4;

/** Shadows java.lang.Math inside package rt4 so the unchanged originals draw scripted "random" numbers. */
public final class Math {
    public static final java.util.ArrayDeque<Double> queue = new java.util.ArrayDeque<>();
    public static double random() { Double d = queue.poll(); return d == null ? 0.0 : d; }
    public static int abs(int v) { return java.lang.Math.abs(v); }
    public static int min(int a, int b) { return java.lang.Math.min(a, b); }
    public static int max(int a, int b) { return java.lang.Math.max(a, b); }
    public static double sqrt(double v) { return java.lang.Math.sqrt(v); }
    public static double pow(double a, double b) { return java.lang.Math.pow(a, b); }
    public static long round(double v) { return java.lang.Math.round(v); }
}