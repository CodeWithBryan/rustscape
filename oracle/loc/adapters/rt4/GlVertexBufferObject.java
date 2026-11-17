package rt4;

/** Oracle adapter: vertex buffer objects are reported unsupported, so GlModel keeps plain buffers. */
public final class GlVertexBufferObject {
	public GlVertexBufferObject() {
		throw new UnsupportedOperationException();
	}
	public GlVertexBufferObject(boolean stream) {
		throw new UnsupportedOperationException();
	}
	public void setArrayBuffer(java.nio.ByteBuffer buffer) {
		throw new UnsupportedOperationException();
	}
	public void updateArrayBuffer(java.nio.ByteBuffer buffer) {
		throw new UnsupportedOperationException();
	}
	public void setElementArrayBuffer(java.nio.ByteBuffer buffer) {
		throw new UnsupportedOperationException();
	}
	public void bindArray() {
		throw new UnsupportedOperationException();
	}
	public void bindElementArray() {
		throw new UnsupportedOperationException();
	}
}