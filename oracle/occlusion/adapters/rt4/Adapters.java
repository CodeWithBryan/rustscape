package rt4;

// Empty or recording stand-ins for what the extracted occluder and scene-walk
// code touches but does not depend on: GL state, lighting, and the models
// themselves (an Entity reports that it was drawn and how tall it is).

class Entity {
	final int id;
	final int minY;

	Entity(int id, int minY) {
		this.id = id;
		this.minY = minY;
	}

	public void render(int yaw, int sinPitch, int cosPitch, int sinYaw, int cosYaw, int x, int z, int y, long key, int level, ParticleSystem particles) {
		SceneGraph.out.add("m" + this.id);
	}

	public int getMinY() {
		return this.minY;
	}

	public void updateModel(int a, int b, int c, int d, int e) {
	}
}

class ParticleSystem {
}

class PlainTile {
	int colorNE;
}

class ShapedTile {
}

// The LocType fields addLoc's occlusion marks read.
class LocInfo {
	boolean render;
	boolean occlude;
	boolean active;
}

// The FloType field buildScene's occlusion mark reads.
class FloType {
	boolean occludeUnderlay;
}

class FloTypeList {
	static FloType[] types;

	static FloType get(int id) {
		return types[id];
	}
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

class MiniMenu {
	static boolean walkPending;
	static int walkDestPlane;
}
