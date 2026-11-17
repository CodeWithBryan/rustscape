package rt4;

/** Oracle adapter: records the one raster call Font makes. */
public class GlRaster {
	public static void drawHorizontalLine(int x, int y, int width, int color) {
		MinimapRecorder.events.add(new int[] {-1, x, y, width, color});
	}
}
