package plugin;

import rt4.OracleLog;

/** Oracle adapter: no plugins are loaded; the call that ends DisplayMode.setWindowMode is recorded. */
public class PluginRepository {
	public static void reloadPlugins() {
		OracleLog.add("reloadPlugins");
	}
}
