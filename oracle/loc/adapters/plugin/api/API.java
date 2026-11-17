package plugin.api;

/** Oracle adapter: the 2009scape plugin hooks GlModel.render calls. Nothing in the location oracle renders. */
public final class API {
	public static boolean IsRoofVisibilityLocPickable(long key) {
		throw new UnsupportedOperationException();
	}
	public static void ReportRoofVisibilityLoc(long key, int plane) {
		throw new UnsupportedOperationException();
	}
}