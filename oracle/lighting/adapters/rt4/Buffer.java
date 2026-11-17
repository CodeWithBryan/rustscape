package rt4;

/** The big-endian writers LightMesh.upload uses with GlRenderer.bigEndian set. */
public final class Buffer {
	public byte[] data;
	public int offset;

	public Buffer(int size) {
		this.data = new byte[size];
	}

	public void p1(int value) {
		this.data[this.offset++] = (byte) value;
	}

	public void p4(int value) {
		this.p1(value >> 24);
		this.p1(value >> 16);
		this.p1(value >> 8);
		this.p1(value);
	}

	public void ip4(int value) {
		this.p4(Integer.reverseBytes(value));
	}

	public void pFloat(float value) {
		this.p4(Float.floatToRawIntBits(value));
	}

	public void gFloat(float value) {
		this.ip4(Float.floatToRawIntBits(value));
	}
}