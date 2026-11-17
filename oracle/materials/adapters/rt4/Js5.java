package rt4;

/**
 * Compile-only adapter for the materials oracle: the texture operations name
 * this class, and none of the oracle's calls reaches it. Not an oracle for the
 * original class.
 */
public final class Js5 {
	public byte[] fetchFile(int group, int file) {
		throw new UnsupportedOperationException("the materials oracle reads no cache");
	}

	public boolean isFileReady(int group, int file) {
		throw new UnsupportedOperationException("the materials oracle reads no cache");
	}

	public boolean isFileReady(int group) {
		throw new UnsupportedOperationException("the materials oracle reads no cache");
	}
}
