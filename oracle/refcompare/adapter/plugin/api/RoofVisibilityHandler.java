package plugin.api;

/**
 * Java transcription of the reference client's only Kotlin source,
 * client/src/main/kotlin/plugin/api/RoofVisibilityHandler.kt, so that the
 * original client can be compiled with javac alone (no Kotlin compiler on
 * this machine). The interface has the same name, package and method
 * signatures as the Kotlin one; no plugin implements it in a plain run.
 */
public interface RoofVisibilityHandler {
	int getGroupLimit();

	boolean isActive();

	void disableIfExpired();

	void ensureBuffers();

	void applyRequests();

	boolean isPicking();

	int getPickScreenX();

	int getPickScreenY();

	void reportTile(int sceneX, int sceneY, int plane);

	boolean isLocPickable(long key);

	void reportLoc(long key, int plane);

	void beginGroup(int group);

	void addGroupTile(int group, int plane, int sceneX, int sceneY);

	void setDestinationTarget(int sceneX, int sceneY);

	void clearDestinationTarget();
}
