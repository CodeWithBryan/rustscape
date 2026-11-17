package rt4;

/**
 * Compile-only adapter for the materials oracle: the texture operations name
 * this class, and none of the oracle's calls reaches it. Not an oracle for the
 * original class.
 */
public final class SoftwareSprite {
	public int width;
	public int height;
	public int[] pixels;

	public void trim() {
		throw new UnsupportedOperationException("the materials oracle loads no sprite");
	}
}
