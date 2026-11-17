package oracle.worldtrace;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.List;
import java.util.Map;

/** Minimal JSON text and digests for trace records. Maps are written in their own iteration order. */
public final class Json {
    private static final char[] HEX = "0123456789abcdef".toCharArray();

    public static String hex(byte[] bytes) {
        char[] out = new char[bytes.length * 2];
        for (int i = 0; i < bytes.length; i++) {
            out[i * 2] = HEX[(bytes[i] >> 4) & 15];
            out[i * 2 + 1] = HEX[bytes[i] & 15];
        }
        return new String(out);
    }

    public static byte[] unhex(String text) {
        byte[] out = new byte[text.length() / 2];
        for (int i = 0; i < out.length; i++) out[i] = (byte) Integer.parseInt(text.substring(i * 2, i * 2 + 2), 16);
        return out;
    }

    public static String sha256(byte[] data) {
        try {
            return hex(MessageDigest.getInstance("SHA-256").digest(data));
        } catch (java.security.NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }

    public static String sha256(String text) { return sha256(text.getBytes(StandardCharsets.UTF_8)); }

    public static String write(Object value) {
        StringBuilder out = new StringBuilder();
        write(out, value);
        return out.toString();
    }

    private static void write(StringBuilder out, Object value) {
        if (value == null || value instanceof Boolean || value instanceof Integer || value instanceof Long) {
            out.append(value);
        } else if (value instanceof String) {
            String text = (String) value;
            out.append('"');
            for (int i = 0; i < text.length(); i++) {
                char c = text.charAt(i);
                if (c == '"' || c == '\\') out.append('\\').append(c);
                else if (c < 0x20 || c > 0x7e) out.append(String.format("\\u%04x", (int) c));
                else out.append(c);
            }
            out.append('"');
        } else if (value instanceof Map) {
            out.append('{');
            boolean first = true;
            for (Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet()) {
                if (!first) out.append(',');
                first = false;
                write(out, String.valueOf(entry.getKey()));
                out.append(':');
                write(out, entry.getValue());
            }
            out.append('}');
        } else if (value instanceof List) {
            out.append('[');
            boolean first = true;
            for (Object item : (List<?>) value) {
                if (!first) out.append(',');
                first = false;
                write(out, item);
            }
            out.append(']');
        } else {
            throw new IllegalArgumentException(value.getClass().getName());
        }
    }
}
