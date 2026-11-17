package rt4;

/** Oracle adapter: JagString's browser bridge is never called. */
public class BrowserControl {
	public static Object call(String name, java.applet.Applet applet) {
		throw new UnsupportedOperationException();
	}

	public static void eval(java.applet.Applet applet, String script) {
		throw new UnsupportedOperationException();
	}
}
