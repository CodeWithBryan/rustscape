package rt4;

/** Oracle adapter: frame data is never loaded; only the symbols SeqType names. */
public class AnimFrameset {
	public boolean isAlphaTransformed(int frame) {
		return false;
	}

	public boolean isColorTransformed(int frame) {
		return false;
	}
}
