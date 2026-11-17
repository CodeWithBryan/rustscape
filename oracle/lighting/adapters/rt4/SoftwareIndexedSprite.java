package rt4;

public final class SoftwareIndexedSprite {
	public int xOffset, yOffset, width, height;
	public byte[] pixels;

	public SoftwareIndexedSprite(int width, int height, int colours) {
		this.width = width;
		this.height = height;
		this.pixels = new byte[width * height];
	}

	public void clear() {
		java.util.Arrays.fill(this.pixels, (byte) 0);
	}
}