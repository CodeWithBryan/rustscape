package rt4;

import com.jogamp.opengl.GL2;

/** Oracle adapter: the GL renderer is "on" (LocType takes its GL branches) without a context or vertex buffer objects. */
public final class GlRenderer {
	public static boolean enabled = true;
	public static boolean arbVboSupported = false;
	public static boolean bigEndian = false;
	public static GL2 gl = null;
	public static void setTextureId(int id) {
		throw new UnsupportedOperationException();
	}
}