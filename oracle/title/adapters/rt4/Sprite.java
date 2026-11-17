package rt4;

public class Sprite {
	public void renderResized(int x, int y, int width, int height) {
		Log.out.append(" background=").append(x).append(',').append(y).append(',').append(width).append(',').append(height);
	}
}
