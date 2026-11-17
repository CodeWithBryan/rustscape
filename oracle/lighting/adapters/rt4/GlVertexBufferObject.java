package rt4;

import java.nio.ByteBuffer;

/** Keeps what LightMesh.upload hands over instead of creating a buffer object. */
public final class GlVertexBufferObject {
	public ByteBuffer data;

	public void setArrayBuffer(ByteBuffer data) {
		this.data = data;
	}

	public void setElementArrayBuffer(ByteBuffer data) {
		this.data = data;
	}

	public void bindArray() {
	}

	public void bindElementArray() {
	}
}