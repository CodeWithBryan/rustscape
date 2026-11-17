package core.cache.misc.buffer;

/** IoBuffer's string members are never reached by HintIcon. */
public class ByteBufferUtils {
	public static String getString(java.nio.ByteBuffer b) {
		throw new UnsupportedOperationException();
	}

	public static int packGJString2(int a, byte[] b, String s) {
		throw new UnsupportedOperationException();
	}
}
