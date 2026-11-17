package com.jogamp.opengl;

/** Oracle adapter: the constants and calls GlModel's draw path names. Nothing in the location oracle draws. */
public interface GL2 {
	int GL_ARRAY_BUFFER = 0x8892, GL_ELEMENT_ARRAY_BUFFER = 0x8893, GL_FLOAT = 0x1406, GL_TRIANGLES = 4, GL_UNSIGNED_BYTE = 0x1401, GL_UNSIGNED_INT = 0x1405;
	void glPushMatrix();
	void glPopMatrix();
	void glTranslatef(float x, float y, float z);
	void glRotatef(float angle, float x, float y, float z);
	void glVertexPointer(int size, int type, int stride, long pointer);
	void glVertexPointer(int size, int type, int stride, java.nio.Buffer pointer);
	void glColorPointer(int size, int type, int stride, long pointer);
	void glColorPointer(int size, int type, int stride, java.nio.Buffer pointer);
	void glNormalPointer(int type, int stride, long pointer);
	void glNormalPointer(int type, int stride, java.nio.Buffer pointer);
	void glTexCoordPointer(int size, int type, int stride, long pointer);
	void glTexCoordPointer(int size, int type, int stride, java.nio.Buffer pointer);
	void glBindBuffer(int target, int buffer);
	void glDrawElements(int mode, int count, int type, long offset);
	void glDrawElements(int mode, int count, int type, java.nio.Buffer indices);
}