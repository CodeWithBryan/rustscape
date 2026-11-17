package rt4;

/** Oracle adapter: records Font's line calls and MiniMap's mask calls. */
public class SoftwareRaster {
	public static int masks = 0;

	public static void drawHorizontalLine(int x, int y, int width, int color) {
		MinimapRecorder.events.add(new int[] {-1, x, y, width, color});
	}

	public static void setLineMasks(int[] start, int[] width) {
		masks++;
	}

	public static void clearLineMasks() {
		masks--;
	}
}
