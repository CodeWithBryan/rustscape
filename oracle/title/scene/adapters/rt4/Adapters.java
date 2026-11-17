package rt4;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;

// Stand-ins for what the title-scene code touches but does not depend on.
// The code under test is the unchanged Camera, MathUtils and CollisionMap,
// and the text copied from LoginManager, client and PathingEntity into the
// generated classes (see xtask/src/title_oracle.rs).

// Shadows java.lang.Math inside this package, so that the copied text's
// Math.random() reads a scripted sequence. Everything else is the JDK's.
class Math {
	static final ArrayList<Double> script = new ArrayList<Double>();
	static int next;

	static double random() {
		return script.get(next++);
	}

	static long round(double value) {
		return java.lang.Math.round(value);
	}

	static int abs(int value) {
		return java.lang.Math.abs(value);
	}

	static double sqrt(double value) {
		return java.lang.Math.sqrt(value);
	}

	static double atan2(double y, double x) {
		return java.lang.Math.atan2(y, x);
	}

	static double sin(double value) {
		return java.lang.Math.sin(value);
	}

	static double cos(double value) {
		return java.lang.Math.cos(value);
	}

	static int max(int a, int b) {
		return java.lang.Math.max(a, b);
	}

	static int min(int a, int b) {
		return java.lang.Math.min(a, b);
	}
}

class JagString {
	final String value;

	JagString(String value) {
		this.value = value;
	}

	static JagString parse(String value) {
		return new JagString(value);
	}

	static JagString parseInt(int value) {
		return new JagString(Integer.toString(value));
	}

	static JagString concatenate(JagString[] parts) {
		StringBuilder out = new StringBuilder();
		for (JagString part : parts) {
			out.append(part.value);
		}
		return new JagString(out.toString());
	}
}

// The map archive's name table: a group exists or it does not.
class Js5 {
	final HashSet<String> names = new HashSet<String>();

	int getGroupId(JagString name) {
		return this.names.contains(name.value) ? 1 : -1;
	}
}

class client {
	static Js5 js5Archive5 = new Js5();
	static int gameState = 10;
	static int loop;

	static void setGameState(int state) {
		gameState = state;
	}
}

class GlRenderer {
	static boolean enabled = true;
}

class Fonts {
	static void drawTextOnScreen(boolean swap, JagString text) {
	}
}

class LocalizedText {
	static JagString LOADING = new JagString("Loading - please wait.");
}

class Map {
}

class MapElementList {
}

class MapList {
	static Map getContainingSource(int x, int y) {
		return null;
	}
}

class NpcType {
	byte loginscreenproperties;
	int size = 1;
	byte spawndirection = 7;
	int rotationspeed = 32;
	int bastypeid = -1;
}

class NpcTypeList {
	static final HashMap<Integer, NpcType> types = new HashMap<Integer, NpcType>();

	static NpcType get(int id) {
		NpcType type = types.get(id);
		return type == null ? new NpcType() : type;
	}
}

class Npc extends PathingEntity {
	NpcType type;

	void setNpcType(NpcType type) {
		this.type = type;
	}
}

class NpcList {
	static int size;
	static Npc[] npcs = new Npc[32768];
	static int[] ids = new int[32768];

	// A stand-in for the walk: a queued step is taken at once. (The real
	// movement, facing and animation passes are the actors oracle's.)
	static void processMovement(Npc npc) {
		if (npc.movementQueueSize > 0) {
			npc.xFine = npc.movementQueueX[0] * 128 + npc.getSize() * 64;
			npc.yFine = npc.movementQueueY[0] * 128 + npc.getSize() * 64;
			npc.movementQueueSize = 0;
		}
	}

	static void processFacing(Npc npc) {
	}

	static void processAnimations(Npc npc) {
	}
}

class Player extends PathingEntity {
	static int plane;

	// Player.teleport, as in the client.
	public final void teleport(int tileX, boolean teleporting, int tileY) {
		super.teleport(this.getSize(), tileX, tileY, teleporting);
	}
}

class PlayerList {
	static Player[] players = new Player[2048];
	static Player self = new Player();
}

class SeqType {
	int movetype;
}

class SeqTypeList {
	static SeqType get(int id) {
		return new SeqType();
	}
}

class LinkedList {
	Object head() {
		return null;
	}

	Object next() {
		return null;
	}

	void clear() {
	}
}

class ChangeLocRequest {
	static LinkedList queue = new LinkedList();
	int x;
	int y;

	void unlink() {
	}
}

class SceneGraph {
	static int centralZoneX;
	static int centralZoneY;
	static int centralPlane;
	static final LinkedList[][][] objStacks = new LinkedList[4][104][104];
	static LinkedList spotanims = new LinkedList();
	static LinkedList projectiles = new LinkedList();
	static byte[][][] renderFlags = new byte[4][104][104];
	static byte[][][] occlusionHeights = new byte[4][104][104];
	static int[][][] tileHeights = new int[4][105][105];

	// The hardware renderer shows every level.
	static boolean allLevelsAreVisible() {
		return true;
	}

	// A sloping stand-in for the ground under a scene-local fine position.
	static int getTileHeight(int plane, int x, int y) {
		return -((x >> 5) + (y >> 6)) - 200;
	}
}

class FogManager {
	static int fades;

	static void setInstantFade() {
		fades++;
	}
}

class LightingManager {
	static int minimapRenderedPlane;
}

class SoundPlayer {
	static int size;
}

class Preferences {
	static boolean keyboardCameraEnabled;
}

class InterfaceList {
	static int keyQueueSize;
	static int[] keyCodes = new int[128];
}

class Keyboard {
	static final int KEY_UP = 98;
	static final int KEY_DOWN = 99;
	static final int KEY_LEFT = 96;
	static final int KEY_RIGHT = 97;
}

class ScriptRunner {
	static int maxViewDistance = 320;
	static int minViewDistance = 256;
}

class Buffer {
	final byte[] data;
	int offset;

	Buffer(byte[] data) {
		this.data = data;
	}

	int g2() {
		this.offset += 2;
		return ((this.data[this.offset - 2] & 0xFF) << 8) + (this.data[this.offset - 1] & 0xFF);
	}
}

class PathFinder {
	static CollisionMap[] collisionMaps = new CollisionMap[4];
}

class Flames {
	static void update() {
	}
}

class CreateManager {
	static int step;
}
