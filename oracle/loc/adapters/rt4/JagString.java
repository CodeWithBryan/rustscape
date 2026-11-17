package rt4;

import java.util.Arrays;

/**
 * Oracle adapter: byte storage so the unchanged Buffer and LocType compile.
 * This is NOT an oracle for the game's text encoding, collation or JagString.
 */
public final class JagString {
	public final byte[] bytes;
	public JagString(byte[] bytes) { this.bytes = bytes.clone(); }
	public static JagString parse(String text) {
		return new JagString(text.getBytes(java.nio.charset.StandardCharsets.ISO_8859_1));
	}
	public static JagString decodeString(byte[] source, int length, int offset) {
		return new JagString(Arrays.copyOfRange(source, offset, offset + length));
	}
	public int length() { return bytes.length; }
	public static void copy(byte[] source, int sourceOffset, byte[] destination, int destinationOffset, int count) {
		System.arraycopy(source, sourceOffset, destination, destinationOffset, count);
	}
	/** ASCII letters only; LocType uses it to drop the option "Hidden", which no observation depends on. */
	public boolean equalsIgnoreCase(JagString other) {
		if (other == null || other.bytes.length != bytes.length) return false;
		for (int i = 0; i < bytes.length; i++) {
			int a = bytes[i], b = other.bytes[i];
			if (a >= 'A' && a <= 'Z') a += 32;
			if (b >= 'A' && b <= 'Z') b += 32;
			if (a != b) return false;
		}
		return true;
	}
	public JagString toLowerCase() {
		throw new UnsupportedOperationException("Real JagString case folding is outside this adapter");
	}
	public int getHash() {
		throw new UnsupportedOperationException("Real JagString name hashing is outside this adapter");
	}
	public int encodeString(byte[] dest, int offset, int length) {
		System.arraycopy(bytes, 0, dest, offset, length);
		return length;
	}
}