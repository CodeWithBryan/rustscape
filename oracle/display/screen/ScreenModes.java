package screen;

/**
 * Prints the screen's modes as the original gets them: the unchanged signlink class
 * rt4.FullScreenManager (constructed, then getDisplayModes) over the default screen device. A
 * query only: nothing is shown and the screen's mode is not changed.
 *
 * Output: one JSON array of ints, width, height, bit depth and refresh rate of each mode in order.
 */
public class ScreenModes {
	public static void main(String[] args) throws Exception {
		int[] modes = new rt4.FullScreenManager().getDisplayModes();
		StringBuilder sb = new StringBuilder("[");
		for (int i = 0; i < modes.length; i++) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append(modes[i]);
		}
		System.out.println(sb.append(']'));
		System.out.flush();
		// AWT may have started threads of its own.
		System.exit(0);
	}
}
