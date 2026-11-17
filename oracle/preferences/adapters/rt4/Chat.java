package rt4;

/** Oracle adapter: chat lines are recorded with digits replaced, since the memory figures vary by run. */
public class Chat {
	public static void add(JagString name, int type, JagString message) {
		OracleLog.add("chat " + (name == null ? "null" : name.toString()) + " " + type + " " + message.toString().replaceAll("[0-9]+", "#"));
	}
}