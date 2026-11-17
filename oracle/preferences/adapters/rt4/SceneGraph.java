package rt4;

/** Oracle adapter: SceneGraph.java:187 and 200-202, with GlRenderer.enabled read from the GlRenderer adapter. */
public class SceneGraph {
	public static boolean levelsHidden = false;

	public static boolean allLevelsAreVisible() {
		return GlRenderer.enabled || Preferences.allLevelsVisible;
	}
}