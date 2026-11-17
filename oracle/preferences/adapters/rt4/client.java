package rt4;

/** Oracle adapter: the client members Cheat uses, recorded. */
public class client {
	public static int modeWhere = 0;
	public static final Js5NetQueue js5NetQueue = new Js5NetQueue();

	public static void unloadSoft() {
		OracleLog.add("unloadSoft");
	}

	public static void setGameState(int state) {
		OracleLog.add("gameState " + state);
	}

	public static final class Js5NetQueue {
		public void quit() {
			OracleLog.add("js5.quit");
		}

		public void drop() {
			OracleLog.add("js5.drop");
		}

		public void breakConnection() {
			OracleLog.add("js5.breakConnection");
		}
	}
}