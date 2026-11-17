package rt4;

/** Oracle adapter: Font.createFont names this class; the oracle never calls it. */
public final class SoftwareFont extends Font {
	public SoftwareFont(byte[] data, int[] xOffsets, int[] yOffsets, int[] innerWidths, int[] innerHeights, byte[][] pixels) {
		super(data, xOffsets, yOffsets, innerWidths, innerHeights);
		throw new UnsupportedOperationException();
	}

	protected void renderGlyph(int glyph, int x, int y, int w, int h, int color) {
	}

	protected void renderGlyphTransparent(int glyph, int x, int y, int w, int h, int color, int alpha) {
	}
}