package rt4;

/** Oracle adapter: Font.load and createFont name these members; the oracle never calls them. */
public final class SpriteLoader {
	public static int[] xOffsets;
	public static int[] yOffsets;
	public static int[] innerWidths;
	public static int[] innerHeights;
	public static byte[][] pixels;

	public static boolean decode(Js5 archive, int file, int group) {
		throw new UnsupportedOperationException();
	}

	public static void clear() {
	}
}