package rt4;

/** Oracle adapter: name icons are not installed on the oracle's fonts. */
public class IndexedSprite {
	public int innerWidth, innerHeight;

	public IndexedSprite(Object... ignored) {
	}

	public void renderTransparent(int x, int y) {
		throw new UnsupportedOperationException();
	}

	public void renderAlpha(int x, int y, int alpha) {
		throw new UnsupportedOperationException();
	}
}
