package rt4;

/** Adapter: the two readers `SceneGraph.readTerrain` uses on its occlusion records. */
public class Buffer {
	public byte[] data;
	public int offset;

	public Buffer(byte[] src) {
		this.data = src;
		this.offset = 0;
	}

	public int g1() {
		return this.data[this.offset++] & 0xFF;
	}

	public byte g1b() {
		return this.data[this.offset++];
	}
}