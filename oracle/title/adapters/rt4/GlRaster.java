package rt4;

public class GlRaster {
	public static void drawRect(int x, int y, int width, int height, int color) {
		Log.out.append(" drawRect=").append(x).append(',').append(y).append(',').append(width).append(',').append(height).append(',').append(color);
	}

	public static void fillRect(int x, int y, int width, int height, int color) {
		Log.out.append(" fillRect=").append(x).append(',').append(y).append(',').append(width).append(',').append(height).append(',').append(color);
	}
}
