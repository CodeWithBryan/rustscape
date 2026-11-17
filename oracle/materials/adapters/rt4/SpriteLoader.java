package rt4;

/**
 * Compile-only adapter for the materials oracle: the texture operations name
 * this class, and none of the oracle's calls reaches it. Not an oracle for the
 * original class.
 */
public final class SpriteLoader {
	public static SoftwareSprite loadSoftwareSpriteAutoDetect(Js5 archive, int id) {
		throw new UnsupportedOperationException("the materials oracle loads no sprite");
	}

	public static SoftwareSprite loadSoftwareSprite(int group, Js5 archive, int file) {
		throw new UnsupportedOperationException("the materials oracle loads no sprite");
	}
}
