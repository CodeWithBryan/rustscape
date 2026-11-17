package rt4;

import java.util.ArrayList;
import java.util.List;

/** Adapter: records the black bars `calculateViewportBounds` fills. */
public class GlRaster {
	public static final List<int[]> fills = new ArrayList<>();

	public static void resetClipRegion() {
	}

	public static void fillRect(int x, int y, int width, int height, int color) {
		fills.add(new int[]{x, y, width, height, color});
	}
}