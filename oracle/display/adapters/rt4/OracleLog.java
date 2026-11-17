package rt4;

import java.util.ArrayList;
import java.util.List;

/** Oracle adapter: the ordered record of what the unchanged DisplayMode asked the rest of the client to do. */
public final class OracleLog {
	public static final List<String> events = new ArrayList<>();
	/** The rectangles GameShell.paintFrameLetterbox filled, in its order. */
	public static final List<int[]> fills = new ArrayList<>();

	public static void add(String event) {
		events.add(event);
	}

	public static void fill(int x, int y, int width, int height) {
		fills.add(new int[]{x, y, width, height});
	}
}
