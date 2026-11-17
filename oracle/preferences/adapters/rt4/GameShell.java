package rt4;

/** Oracle adapter: the three GameShell members Preferences and Cheat read or write. */
public class GameShell {
	public static int maxMemory = 64;
	public static SignLink signLink;
	public static volatile boolean replaceCanvas = false;

	public static void setFpsTarget(int fps) {
		OracleLog.add("fpsTarget " + fps);
	}
}