package com.jogamp.opengl;

/** Records the light state changes LightingManager issues; everything else is ignored. */
public class GL2 {
	public static final int GL_AMBIENT = 1, GL_LINEAR_ATTENUATION = 2, GL_CONSTANT_ATTENUATION = 3,
		GL_QUADRATIC_ATTENUATION = 4, GL_DIFFUSE = 5, GL_POSITION = 6, GL_DST_COLOR = 7, GL_ONE = 8,
		GL_FOG_COLOR = 9, GL_TEXTURE_ENV = 10, GL_SRC0_RGB = 11, GL_CONSTANT = 12, GL_OPERAND0_RGB = 13,
		GL_SRC_ALPHA = 14, GL_TEXTURE = 15, GL_SRC_COLOR = 16, GL_ONE_MINUS_SRC_ALPHA = 17,
		GL_TEXTURE_COORD_ARRAY = 18, GL_TEXTURE_ENV_COLOR = 19, GL_C4UB_V3F = 20, GL_TRIANGLES = 21,
		GL_UNSIGNED_INT = 22, GL_ARRAY_BUFFER = 23, GL_ELEMENT_ARRAY_BUFFER = 24, GL_COLOR_ARRAY = 25;
	public final StringBuilder log = new StringBuilder();

	public void glEnable(int cap) {
		this.log.append(" E").append(cap - 16388);
	}

	public void glDisable(int cap) {
		this.log.append(" D").append(cap - 16388);
	}

	public void glLightf(int light, int name, float value) {
		if (name == GL_QUADRATIC_ATTENUATION) {
			this.log.append(" S").append(light - 16388).append(':').append((int) value);
		}
	}

	public void glLightfv(int light, int name, float[] values, int offset) {
	}

	public void glDepthMask(boolean flag) {
	}

	public void glBlendFunc(int source, int destination) {
	}

	public void glFogfv(int name, float[] values, int offset) {
	}

	public void glTexEnvi(int target, int name, int value) {
	}

	public void glTexEnvfv(int target, int name, float[] values, int offset) {
	}

	public void glEnableClientState(int array) {
	}

	public void glInterleavedArrays(int format, int stride, long offset) {
	}

	public void glInterleavedArrays(int format, int stride, java.nio.Buffer pointer) {
	}

	public void glDrawElements(int mode, int count, int type, long offset) {
	}

	public void glDrawElements(int mode, int count, int type, java.nio.Buffer indices) {
	}

	public void glBindBuffer(int target, int buffer) {
	}

	public void glPushMatrix() {
	}

	public void glPopMatrix() {
	}

	public void glTranslatef(float x, float y, float z) {
	}
}