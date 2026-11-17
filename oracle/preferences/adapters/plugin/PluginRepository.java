package plugin;

import rt4.JagString;
import rt4.OracleLog;

/** Oracle adapter: no plugins are loaded; the two calls are recorded. */
public class PluginRepository {
	public static void ProcessCommand(JagString command) {
		OracleLog.add("plugins " + command.toString());
	}

	public static void reloadPlugins() {
		OracleLog.add("reloadPlugins");
	}
}