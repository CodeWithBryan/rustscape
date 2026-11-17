package core.tools;

/** SystemLogger.logGE: a recorded service. */
public class SystemLogger {
	public static void logGE(String message) {
		oracle.Support.emit("logGE", message);
	}
}