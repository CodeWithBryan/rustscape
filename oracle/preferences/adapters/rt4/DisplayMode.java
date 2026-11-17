package rt4;

/** Oracle adapter. */
public class DisplayMode {
	public static void setWindowMode(boolean replaceCanvas, int newMode, int width, int height) {
		OracleLog.add("windowMode " + replaceCanvas + " " + newMode + " " + width + " " + height);
	}
}