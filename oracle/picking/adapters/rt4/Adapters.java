package rt4;

// Empty or recording stand-ins for what the extracted picking and scene-walk
// code touches but does not depend on: GL state, lighting, software raster
// calls, and the models themselves (an Entity only reports that it was drawn).

class Entity {
	final int id;

	Entity(int id) {
		this.id = id;
	}

	public void render(int yaw, int sinPitch, int cosPitch, int sinYaw, int cosYaw, int x, int z, int y, long key, int level, ParticleSystem particles) {
		ProbeScene.out.add("m" + this.id);
	}

	public int getMinY() {
		return 0;
	}

	public void updateModel(int a, int b, int c, int d, int e) {
	}
}

class ParticleSystem {
}

class PlainTile {
	int textureId = -1;
	int colorNE;
	int colorNW;
	int colorSE;
	int colorSW;
	boolean flat;
}

class ShapedTile {
}

class LocInfo {
	int interactable;
	int supportitems;
	boolean hasanimation;
}

class GlRenderer {
	static boolean enabled = true;

	static void setDepthLayer(float layer) {
	}
}

class LightingManager {
	static int visibleMinX;
	static int visibleMinY;
	static int visibleMaxX;
	static int visibleMaxY;

	static void updateLightsForTile(int... a) {
	}

	static void updateLightsForWallPiece(int... a) {
	}

	static void updateLightsForRegion(int... a) {
	}

	static void pruneInactiveLights(int... a) {
	}
}

class MaterialManager {
	static void setUnderwaterFogRange(int range) {
	}

	static void setMaterial(int a, int b) {
	}
}

class WaterMaterialRenderer {
	static void setUnderwaterColor(int color) {
	}

	static float[] getWaterFogColor() {
		return null;
	}
}

class FogManager {
	static void setFogColor(float[] color) {
	}
}

class TextureProvider {
	int getAverageColor(int texture) {
		return 0;
	}
}

class Rasteriser {
	static int screenLowerX;
	static int screenUpperX;
	static int screenLowerY;
	static int screenUpperY;
	static int centerX;
	static int centerY;
	static int alpha;
	static int width;
	static boolean testX;
	static TextureProvider textureProvider = new TextureProvider();

	static void fillGouraudTriangle(int... a) {
	}

	static void fillTexturedTriangle(int... a) {
	}
}

class ColorUtils {
	static int multiplyLightness3(int a, int b) {
		return 0;
	}
}

class Preferences {
	static boolean manyGroundTextures;
}

class MiniMenu {
	static boolean walkPending;
	static int walkDestPlane;
	static int walkDestTileX;
	static int walkDestTileY;
	static int clickTileX = -1;
	static int clickTileY = -1;
	static int pickResultCount;
}

class API {
	static boolean IsRoofVisibilityLocPickable(long key) {
		return false;
	}

	static void ReportRoofVisibilityLoc(long key, int plane) {
	}

	static boolean IsRoofVisibilityPicking() {
		return false;
	}

	static int GetRoofVisibilityPickScreenX() {
		return 0;
	}

	static int GetRoofVisibilityPickScreenY() {
		return 0;
	}

	static void ReportRoofVisibilityTile(int x, int y, int level) {
	}
}

class RawModel {
	static boolean allowInput;
	static int pickScreenY;
}

class Model {
	static long[] pickResults = new long[1000];
}
