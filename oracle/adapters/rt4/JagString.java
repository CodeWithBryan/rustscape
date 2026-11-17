package rt4;

import java.util.Arrays;

/**
 * Test-only byte storage so unmodified Buffer can compile in isolation.
 * This is NOT an oracle for the game's text encoding, collation, or JagString.
 */
public final class JagString {
    public final byte[] bytes;
    public JagString(byte[] bytes) { this.bytes = bytes.clone(); }
    public static JagString decodeString(byte[] source, int length, int offset) {
        return new JagString(Arrays.copyOfRange(source, offset, offset + length));
    }
    public int length() { return bytes.length; }
    public static void copy(byte[] source, int sourceOffset, byte[] destination, int destinationOffset, int count) {
        System.arraycopy(source, sourceOffset, destination, destinationOffset, count);
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
