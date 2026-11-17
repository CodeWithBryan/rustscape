package rt4;

public class Font {
	public void renderCenter(JagString text, int x, int y, int color, int shadow) {
		Log.out.append(' ').append(text.text).append('=').append(x).append(',').append(y).append(',').append(color).append(',').append(shadow);
	}
}
