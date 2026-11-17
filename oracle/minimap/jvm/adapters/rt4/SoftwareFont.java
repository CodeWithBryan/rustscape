package rt4;

/** Oracle adapter: the concrete Font. Glyph rasterising is replaced by a record of each call. */
public class SoftwareFont extends Font {
	/** Which label font this is; set by the oracle. */
	public int index = -1;
	public final int[] xOffsets, yOffsets, innerWidths, innerHeights;

	public SoftwareFont(byte[] data, int[] xOffsets, int[] yOffsets, int[] innerWidths, int[] innerHeights, byte[][] pixels) {
		super(data, xOffsets, yOffsets, innerWidths, innerHeights);
		this.xOffsets = xOffsets;
		this.yOffsets = yOffsets;
		this.innerWidths = innerWidths;
		this.innerHeights = innerHeights;
	}

	protected void renderGlyph(int glyph, int x, int y, int w, int h, int color) {
		MinimapRecorder.events.add(new int[] {this.index, glyph, x, y, w, h, color, 256});
	}

	protected void renderGlyphTransparent(int glyph, int x, int y, int w, int h, int color, int alpha) {
		MinimapRecorder.events.add(new int[] {this.index, glyph, x, y, w, h, color, alpha});
	}
}
