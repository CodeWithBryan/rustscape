package com.jogamp.opengl;

import java.nio.ByteBuffer;

/**
 * Oracle adapter: the handful of JOGL members the unchanged GlTile.renderTiles
 * names. The terrain oracle never renders, so nothing here is ever called.
 */
public class GL2 {
	public static final int GL_ARRAY_BUFFER = 0x8892;
	public static final int GL_ELEMENT_ARRAY_BUFFER = 0x8893;
	public static final int GL_FLOAT = 0x1406;
	public static final int GL_UNSIGNED_BYTE = 0x1401;
	public static final int GL_UNSIGNED_INT = 0x1405;
	public static final int GL_TEXTURE0 = 0x84C0;
	public static final int GL_TRIANGLES = 4;

	public void glBindBuffer(int target, int buffer) {
	}

	public void glVertexPointer(int size, int type, int stride, ByteBuffer pointer) {
	}

	public void glVertexPointer(int size, int type, int stride, long offset) {
	}

	public void glColorPointer(int size, int type, int stride, ByteBuffer pointer) {
	}

	public void glColorPointer(int size, int type, int stride, long offset) {
	}

	public void glNormalPointer(int type, int stride, ByteBuffer pointer) {
	}

	public void glNormalPointer(int type, int stride, long offset) {
	}

	public void glTexCoordPointer(int size, int type, int stride, ByteBuffer pointer) {
	}

	public void glTexCoordPointer(int size, int type, int stride, long offset) {
	}

	public void glClientActiveTexture(int unit) {
	}

	public void glDrawElements(int mode, int count, int type, ByteBuffer indices) {
	}
}
