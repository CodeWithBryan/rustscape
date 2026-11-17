package rt4;

import com.jogamp.opengl.GL2;

import java.nio.ByteBuffer;
import java.util.HashMap;

// Oracle adapters for the terrain differential. Everything the unchanged
// GlTile, FluType, FloType, ColorUtils and the extracted SceneGraph and GlModel
// methods name but the oracle does not exercise is an empty stand-in here.
// LocType/LocTypeList serve the location facts and models the Rust side wrote
// into the oracle input; they contain no original logic except the tail of
// LocType.getStaticEntity (alignToTerrain, then the mergeable flag).

class GlRenderer {
	static GL2 gl;
	static boolean enabled = true;
	// uploadVertexData writes big-endian floats and hands them to the VBO stub.
	static boolean bigEndian = true;
	static boolean arbVboSupported = true;

	static void setTextureId(int id) {
	}

	static void setDepthLayer(float layer) {
	}

	static void disableDepthMask() {
	}

	static void enableDepthMask() {
	}
}

class GlVertexBufferObject {
	byte[] data;

	void setArrayBuffer(ByteBuffer buffer) {
		this.data = new byte[buffer.remaining()];
		buffer.duplicate().get(this.data);
	}

	void bindArray() {
	}
}

class GlBuffer {
	boolean valid;
}

class MaterialManager {
	static void setMaterial(int animation, int material) {
	}
}

class UnderwaterMaterialRenderer {
	static int getSecondaryTextureUnit() {
		return 0;
	}
}

class Preferences {
	static boolean highDetailLighting = true;
	static boolean showGroundDecorations = true;
	static boolean allLevelsVisible = true;
}

class FogManager {
	// Read by buildScene's tileLightness loop without high detail lighting.
	static final float[] light0Position = new float[4];

	static float getLightingModelAmbient() {
		return 0;
	}

	static float getLight0Diffuse() {
		return 0;
	}

	static int getLightColor() {
		return 0;
	}
}

class ParticleSystem {
}

class ObjStackEntity {
}

class SoftwareIndexedSprite {
}

class Npc {
}

class Player {
}

class CollisionMap {
	void flagWall(int angle, int shape, boolean blocksProjectiles, int y, int x) {
	}

	void flagScenery(int x, boolean blocksProjectiles, int y, int sizeX, int sizeY) {
	}

	void flagGroundDecor(int x, int y) {
	}
}

class LoginManager {
	static final int[] WALL_ROTATION_FLAGS = new int[]{16, 32, 64, 128};
}

class AreaSoundManager {
	static void add(int y, LocType loc, int rotation, Npc npc, int x, int level, Player player) {
	}
}

class ShadowManager {
	static void addObjectShadow(SoftwareIndexedSprite sprite, int fineX, int height, int fineZ) {
	}
}

class FluTypeList {
	static FluType[] types;

	static FluType get(int id) {
		return types[id];
	}
}

class FloTypeList {
	static FloType[] types;

	static FloType get(int id) {
		return types[id];
	}
}

/** The scene entity of a location that is not a static model. */
class Loc extends Entity {
	Loc(int locId, int shape, int orientation, int level, int tileX, int tileY, int seqId, boolean randomizeAnim, Entity previous) {
	}

	@Override
	public void updateModel(int pitch, int yaw, int roll, int x, int z) {
	}

	@Override
	public void render(int yaw, int sinCameraPitch, int cosCameraPitch, int sinCameraYaw, int cosCameraYaw, int sceneX, int sceneZ, int sceneY, long key, int plane, ParticleSystem particleSystem) {
	}

	@Override
	public int getMinY() {
		return 0;
	}
}

abstract class Model extends Entity {
	public abstract int getLengthXZ();

	// Hill skew type 3 (GlModel.alignToTerrain's third branch) is not exercised.
	protected final void alignToTerrain(int[][] heightMap, int xFine, int height, int yFine, int sizeX, int sizeY) {
		throw new UnsupportedOperationException();
	}
}

class LocEntity {
	Entity model;
	SoftwareIndexedSprite sprite;
}

/** A model as the input describes it, before any hill skew. */
class ModelRecord {
	int[] x, y, z;
	short[] normalX, normalY, normalZ, normalMagnitude;
	int[] vertexOffsets;
	short[] vertexLookup;
	int triangleCount;

	GlModel build() {
		GlModel model = new GlModel();
		model.vertexCount = this.x.length;
		model.vertexX = this.x.clone();
		model.vertexY = this.y.clone();
		model.vertexZ = this.z.clone();
		model.normalX = this.normalX.clone();
		model.normalY = this.normalY.clone();
		model.normalZ = this.normalZ.clone();
		model.normalMagnitude = this.normalMagnitude.clone();
		model.uniqueVertexCount = this.normalX.length;
		model.vertexOffsets = this.vertexOffsets;
		model.vertexLookup = this.vertexLookup;
		model.triangleCount = this.triangleCount;
		return model;
	}
}

class LocType {
	static final int WALL_STRAIGHT = 0;
	static final int WALL_DIAGONALCORNER = 1;
	static final int WALL_L = 2;
	static final int WALL_SQUARECORNER = 3;
	static final int WALL_DIAGONAL = 9;
	static final int WALLDECOR_STRAIGHT_XOFFSET = 4;
	static final int WALLDECOR_STRAIGHT_YOFFSET = 5;
	static final int WALLDECOR_DIAGONAL_XOFFSET = 6;
	static final int WALLDECOR_DIAGONAL_YOFFSET = 7;
	static final int WALLDECOR_DIAGONAL_BOTH = 8;
	static final int ROOF_STRAIGHT = 12;
	static final int ROOF_DIAGONAL_WITH_ROOFEDGE = 13;
	static final int ROOF_FLAT = 17;
	static final int CENTREPIECE_STRAIGHT = 10;
	static final int CENTREPIECE_DIAGONAL = 11;
	static final int GROUNDDECOR = 22;

	static final LocEntity tempLocEntity = new LocEntity();

	int id;
	boolean render;
	int width = 1;
	int length = 1;
	int interactable;
	int supportitems;
	boolean hasanimation;
	boolean castshadow;
	int anim = -1;
	int[] multiLocs;
	boolean dynamic;
	boolean allowrandomizedanimation;
	boolean clipped;
	int blockwalk;
	boolean forcedecor;
	boolean active = true;
	boolean occlude;
	boolean blockrange;
	int walloff = 16;
	boolean computeVertexColors;
	byte hillskewType;
	short hillskewAmount;
	/** Models by shape * 8 + rotation, as addLoc asks for them. */
	final HashMap<Integer, ModelRecord> models = new HashMap<>();

	boolean hasAreaSound() {
		return false;
	}

	LocEntity getStaticEntity(int rotation, int tileX, int[][] heightmap, int shape, int tileY, int[][] overlayHeightmap, boolean shareLight, SoftwareIndexedSprite shadowSprite, boolean castShadow, int plane) {
		ModelRecord record = this.models.get(shape * 8 + rotation);
		if (record == null) {
			tempLocEntity.model = null;
			tempLocEntity.sprite = null;
			return tempLocEntity;
		}
		GlModel glModel = record.build();
		boolean mergeVertexColors = this.computeVertexColors & shareLight;
		GlModel copy = record.build();
		if (this.hillskewType != 0) {
			copy.alignToTerrain(this.hillskewType, this.hillskewAmount, glModel, heightmap, overlayHeightmap, tileX, tileY, plane);
		}
		tempLocEntity.model = copy;
		copy.mergeable = mergeVertexColors;
		tempLocEntity.sprite = null;
		return tempLocEntity;
	}
}

class LocTypeList {
	static final HashMap<Integer, LocType> types = new HashMap<>();

	static LocType get(int id) {
		LocType type = types.get(id);
		if (type == null) {
			throw new IllegalStateException("the oracle input has no location type " + id);
		}
		return type;
	}
}
