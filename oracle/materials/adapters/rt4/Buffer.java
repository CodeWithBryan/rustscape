package rt4;

/**
 * Compile-only adapter for the materials oracle: the texture operations name
 * this class, and none of the oracle's calls reaches it. Not an oracle for the
 * original class.
 */
public final class Buffer {
	public byte[] data;
	public int offset;

	private static UnsupportedOperationException unused() {
		return new UnsupportedOperationException("the materials oracle decodes nothing");
	}

	public int g1() {
		throw unused();
	}

	public byte g1b() {
		throw unused();
	}

	public int g2() {
		throw unused();
	}

	public int g2b() {
		throw unused();
	}

	public int g3() {
		throw unused();
	}

	public int g4() {
		throw unused();
	}
}
