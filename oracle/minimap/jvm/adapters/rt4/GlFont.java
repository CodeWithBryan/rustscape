package rt4;

/** Oracle adapter: never constructed; GlRenderer.enabled is false. */
public class GlFont extends Font {
	public GlFont(byte[] data, int[] xOffsets, int[] yOffsets, int[] innerWidths, int[] innerHeights, byte[][] pixels) {
		super(data, xOffsets, yOffsets, innerWidths, innerHeights);
	}

	public static void setLineMask(GlSprite mask) {
		throw new UnsupportedOperationException();
	}

	public static void clearLineMask() {
		throw new UnsupportedOperationException();
	}

	protected void renderGlyph(int glyph, int x, int y, int w, int h, int color) {
		throw new UnsupportedOperationException();
	}

	protected void renderGlyphTransparent(int glyph, int x, int y, int w, int h, int color, int alpha) {
		throw new UnsupportedOperationException();
	}
}
