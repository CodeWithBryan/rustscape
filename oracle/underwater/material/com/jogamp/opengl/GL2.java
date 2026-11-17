package com.jogamp.opengl;

import java.nio.Buffer;

/**
 * Oracle adapter: the members of JOGL's GL2 interface that the unchanged
 * UnderwaterMaterialRenderer, WaterMaterialRenderer, MaterialManager and
 * GlTile name, with OpenGL's own enumerant values. The oracle implements it
 * with a recording proxy; no OpenGL exists.
 */
public interface GL2 {
	int GL_TRIANGLES = 0x0004;
	int GL_ADD = 0x0104;
	int GL_SRC_COLOR = 0x0300;
	int GL_FOG_BIT = 0x0080;
	int GL_DEPTH_TEST = 0x0B71;
	int GL_FOG_START = 0x0B63;
	int GL_TEXTURE_GEN_S = 0x0C60;
	int GL_TEXTURE_GEN_T = 0x0C61;
	int GL_TEXTURE_GEN_R = 0x0C62;
	int GL_TEXTURE_GEN_Q = 0x0C63;
	int GL_TEXTURE_1D = 0x0DE0;
	int GL_TEXTURE_2D = 0x0DE1;
	int GL_COMPILE = 0x1300;
	int GL_UNSIGNED_BYTE = 0x1401;
	int GL_UNSIGNED_INT = 0x1405;
	int GL_FLOAT = 0x1406;
	int GL_TEXTURE = 0x1702;
	int GL_ALPHA = 0x1906;
	int GL_LUMINANCE_ALPHA = 0x190A;
	int GL_REPLACE = 0x1E01;
	int GL_S = 0x2000;
	int GL_T = 0x2001;
	int GL_R = 0x2002;
	int GL_Q = 0x2003;
	int GL_MODULATE = 0x2100;
	int GL_TEXTURE_ENV_MODE = 0x2200;
	int GL_TEXTURE_ENV_COLOR = 0x2201;
	int GL_TEXTURE_ENV = 0x2300;
	int GL_EYE_LINEAR = 0x2400;
	int GL_OBJECT_LINEAR = 0x2401;
	int GL_TEXTURE_GEN_MODE = 0x2500;
	int GL_OBJECT_PLANE = 0x2501;
	int GL_EYE_PLANE = 0x2502;
	int GL_LINEAR = 0x2601;
	int GL_TEXTURE_MAG_FILTER = 0x2800;
	int GL_TEXTURE_MIN_FILTER = 0x2801;
	int GL_TEXTURE_WRAP_S = 0x2802;
	int GL_TEXTURE_3D = 0x806F;
	int GL_COLOR_ARRAY = 0x8076;
	int GL_TEXTURE_COORD_ARRAY = 0x8078;
	int GL_CLAMP_TO_EDGE = 0x812F;
	int GL_TEXTURE0 = 0x84C0;
	int GL_TEXTURE1 = 0x84C1;
	int GL_TEXTURE2 = 0x84C2;
	int GL_COMBINE = 0x8570;
	int GL_COMBINE_RGB = 0x8571;
	int GL_COMBINE_ALPHA = 0x8572;
	int GL_RGB_SCALE = 0x8573;
	int GL_INTERPOLATE = 0x8575;
	int GL_CONSTANT = 0x8576;
	int GL_PREVIOUS = 0x8578;
	int GL_SRC0_RGB = 0x8580;
	int GL_SRC1_RGB = 0x8581;
	int GL_SRC2_RGB = 0x8582;
	int GL_SRC0_ALPHA = 0x8588;
	int GL_SRC2_ALPHA = 0x858A;
	int GL_OPERAND0_RGB = 0x8590;
	int GL_ARRAY_BUFFER = 0x8892;
	int GL_ELEMENT_ARRAY_BUFFER = 0x8893;

	void glActiveTexture(int texture);

	void glBindBuffer(int target, int buffer);

	void glBindTexture(int target, int texture);

	void glCallList(int list);

	void glClientActiveTexture(int texture);

	void glColor4fv(float[] v, int offset);

	void glColorPointer(int size, int type, int stride, Buffer pointer);

	void glColorPointer(int size, int type, int stride, long offset);

	void glDeleteTextures(int n, int[] textures, int offset);

	void glDisable(int cap);

	void glDisableClientState(int array);

	void glDrawElements(int mode, int count, int type, Buffer indices);

	void glEnable(int cap);

	void glEnableClientState(int array);

	void glEndList();

	void glFogf(int pname, float param);

	int glGenLists(int range);

	void glGenTextures(int n, int[] textures, int offset);

	void glLoadIdentity();

	void glNewList(int list, int mode);

	void glNormalPointer(int type, int stride, Buffer pointer);

	void glNormalPointer(int type, int stride, long offset);

	void glPopAttrib();

	void glPopMatrix();

	void glPushAttrib(int mask);

	void glPushMatrix();

	void glRotatef(float angle, float x, float y, float z);

	void glTexCoordPointer(int size, int type, int stride, Buffer pointer);

	void glTexCoordPointer(int size, int type, int stride, long offset);

	void glTexEnvf(int target, int pname, float param);

	void glTexEnvfv(int target, int pname, float[] params, int offset);

	void glTexEnvi(int target, int pname, int param);

	void glTexGenfv(int coord, int pname, float[] params, int offset);

	void glTexGeni(int coord, int pname, int param);

	void glTexImage1D(int target, int level, int internalFormat, int width, int border, int format, int type, Buffer pixels);

	void glTexImage2D(int target, int level, int internalFormat, int width, int height, int border, int format, int type, Buffer pixels);

	void glTexImage3D(int target, int level, int internalFormat, int width, int height, int depth, int border, int format, int type, Buffer pixels);

	void glTexParameteri(int target, int pname, int param);

	void glTranslatef(float x, float y, float z);

	void glVertexPointer(int size, int type, int stride, Buffer pointer);

	void glVertexPointer(int size, int type, int stride, long offset);
}