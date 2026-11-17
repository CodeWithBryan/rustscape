package core.cache.misc.buffer;
import java.nio.ByteBuffer;
public final class ByteBufferUtils {
    public static int getMedium(ByteBuffer buffer) {
        return (buffer.get() & 255) << 16 | (buffer.get() & 255) << 8 | buffer.get() & 255;
    }
    public static String getString(ByteBuffer buffer) {throw new UnsupportedOperationException("Not a text oracle");}
    public static int packGJString2(int offset, byte[] bytes, String text) {throw new UnsupportedOperationException("Not a text oracle");}
}
