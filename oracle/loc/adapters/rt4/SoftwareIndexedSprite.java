package rt4;

/** Oracle adapter: the shadow sprite's fields. The oracle never asks for a shadow. */
public final class SoftwareIndexedSprite {
	public int width, height, innerWidth, innerHeight, xOffset, yOffset;
	public byte[] pixels;
	public SoftwareIndexedSprite(int width, int height, int unused) {
		this.width = this.innerWidth = width;
		this.height = this.innerHeight = height;
		this.pixels = new byte[width * height];
	}
	public void clear() {
		java.util.Arrays.fill(this.pixels, (byte) 0);
	}
}