package rt4;

// Adapter classes for the interface-fidelity oracle. The unchanged original
// classes compiled beside them (InterfaceList, Inv, Font, SoftwareFont,
// SoftwareRaster, JagString and the list and table classes) refer to these
// names; each adapter carries only the members those classes touch and no
// behaviour of its own beyond what is commented.

/** The fields of rt4.Component that InterfaceList reads and writes, with the original's defaults. */
class Component {
	static final ServerActiveProperties DEFAULT_SERVER_ACTIVE_PROPERTIES = new ServerActiveProperties(0, -1);
	int id = -1;
	int type;
	boolean if3;
	int overlayer = -1;
	int createdComponentId = -1;
	Component[] createdComponents;
	Component dragParent;
	ServerActiveProperties properties = DEFAULT_SERVER_ACTIVE_PROPERTIES;
	int rectangleLoop = -1;
	int rectangle = -1;
	JagString[] ops;
	int seqNextFrame = 1, seqFrame, seqCycle;
	int scrollMaxH, scrollMaxV, scrollX, scrollY;
	int x, y, width, height;
	int baseX, baseY, baseWidth, baseHeight;
	byte xMode, yMode, dynamicWidthValue, dynamicHeightValue;
	int invMarginX, invMarginY;
	int aspectWidth, aspectHeight;
	int clientCode;
	boolean hasEventHandlers, hidden, noClickThrough, dragging, mouseOver;
	byte[] keyCodes, keyModifiers;
	int[] keyRepeatDelays, keyRepeatTimers;
	Object[] onResize, onLoad, onDialogAbort, onWidgetsOpenClose, onOptionClick;
	Object[] onScroll, onClickRepeat, onDrag, onRelease, onHold, onMouseOver, onMouseRepeat, onMouseLeave;
	Object[] onTimer, onVarcTransmit, onVarcstrTransmit, onVarpTransmit, onInvTransmit, onStatTransmit;
	Object[] onMsg, onFriendTransmit, onClanTransmit, onStockTransmit, onMiscTransmit, onKey, onMinimapUnlock;
	int[] varcTriggers, varcstrTriggers, varpTriggers, inventoryTriggers, statTriggers;
	int updatedVarcsReaderIndex, updatedVarcstrsReaderIndex, updatedVarpsReaderIndex;
	int updatedInventoriesReaderIndex, updatedStatsReaderIndex;
	int lastTransmitTimer = -1;
	int hoverOverlayer = -1, overColor;
	int modelSeqId = -1, activeModelSeqId = -1, modelRotationSpeed, modelXAngle, modelYAngle;

	void decodeIf1(Buffer buffer) {
	}

	void decodeIf3(Buffer buffer) {
	}
}

class Buffer {
	Buffer(byte[] data) {
	}
}

/** No cache: every group the oracle stages is put into InterfaceList.components directly. */
class Js5 {
	int capacity() {
		return 0;
	}

	void discardUnpacked(int group) {
	}

	boolean isGroupReady(int group) {
		return false;
	}

	int getGroupCapacity(int group) {
		return 0;
	}

	byte[] fetchFile(int group, int file) {
		return null;
	}
}

class Cheat {
	static boolean qaOpTest = false;

	static void teleport(int x, int z, int plane) {
	}
}

class MiniMenu {
	static int size = 0;
	static final short[] actions = new short[500];
	static int clickOffsetY;
	static int defaultCursorId = -1;

	static void remove(int index) {
	}

	static JagString getOp(int index) {
		return JagString.EMPTY;
	}
}

/**
 * startComponentDrag and getDragParent are the original's bodies (Cs1ScriptRunner 1326-1345);
 * everything else is a field.
 */
class Cs1ScriptRunner {
	static boolean isMenuOpen;
	static Component dragParentComponent, draggedComponent;
	static int dragElapsedTicks, dragBoundsMinX, dragStartMouseX, dragStartMouseY, gameSceneTooltipX;
	static boolean isDragging;

	static void startComponentDrag(int mouseY, int mouseX, Component component) {
		if (draggedComponent != null || isMenuOpen || (component == null || getDragParent(component) == null)) {
			return;
		}
		draggedComponent = component;
		dragParentComponent = getDragParent(component);
		dragStartMouseX = mouseX;
		isDragging = false;
		dragElapsedTicks = 0;
		dragStartMouseY = mouseY;
	}

	static Component getDragParent(Component component) {
		Component parent = InterfaceList.getDragRenderParent(component);
		if (parent == null) {
			parent = component.dragParent;
		}
		return parent;
	}

	static boolean isTrue(Component component) {
		return false;
	}

	static void renderTopLevelInterface() {
	}
}

class Fonts {
	static Font b12Full;
}

class LocalizedText {
	static JagString CHOOSE_OPTION = JagString.parse("Choose Option");
	static JagString MILLION = JagString.parse("M");
	static JagString THOUSAND = JagString.parse("K");
}

class SignLink {
	void setCursor(Object image, int width, Object canvas, java.awt.Point hotSpot, int height) {
	}
}

class GameShell {
	static int canvasWidth, canvasHeight;
	static SignLink signLink = new SignLink();
	static Object canvas;
}

class Mouse {
	static int lastMouseX, lastMouseY, pressedButton, clickButton, clickX, clickY;
}

class MouseWheel {
	static int wheelRotation;
}

class Keyboard {
	static final int KEY_SHIFT = 81, KEY_CTRL = 82, KEY_ALT = 86;
	static final boolean[] pressedKeys = new boolean[112];
}

class client {
	static int loop;
	static Js5 js5Archive8;

	static void setGameState(int state) {
	}
}

/** Records button clicks made by component key bindings. */
class ClientProt {
	static final java.util.List<int[]> clicks = new java.util.ArrayList<>();

	static void sendButtonClick(JagString opBase, int child, int op, int component) {
		clicks.add(new int[]{component, child, op});
	}

	static void closeWidget() {
	}
}

class WorldMap {
	static Component component;
	static float zoom = 1.0F, targetZoom = 1.0F;
	static int viewX, viewY, originX, originY, length;

	static void setViewX(int x) {
		viewX = x;
	}

	static void setViewY(int y) {
		viewY = y;
	}

	static void setViewFromMousePosition(int width, int mouseY, int mouseX, int height) {
	}
}

class LoginManager {
	static int staffModLevel, loginScreenId = -1;

	static void setupLoadingScreenRegion() {
	}
}

class GlRenderer {
	static boolean enabled = false;
}

class GlRaster {
	static void drawHorizontalLine(int x, int y, int length, int color) {
	}

	static void resetClipRegion() {
	}
}

class VarcDomain {
	static int updatedVarcsWriterIndex, updatedVarcstrsWriterIndex;
	static final int[] updatedVarcs = new int[32], updatedVarcstrs = new int[32];
}

class VarpDomain {
	static int updatedVarpsWriterIndex;
	static final int[] updatedVarps = new int[32];
}

class PlayerSkillXpTable {
	static int updatedStatsWriterIndex;
	static final int[] updatedStats = new int[32];
}

class Chat {
	static int transmitAt;
}

class FriendsList {
	static int transmitAt;
}

class ClanChat {
	static int transmitAt;
}

class StockMarketManager {
	static int transmitAt;
}

class Camera {
	static boolean splineJustFinished;
	static int cameraType, renderX, renderY, lockedTargetX, lockedTargetY;

	static void updateLoginScreenCamera() {
	}
}

class Protocol {
	static Component tooltipComponent;
	static int sceneDelta;
}

/** Scripts are not run; the oracle only observes what is queued. */
class ScriptRunner {
	static int scriptCursorId = -1;
	static boolean glSceneNeedsRender;
	static final java.util.List<HookRequest> ran = new java.util.ArrayList<>();

	static void run(int maxCycles, HookRequest request) {
		ran.add(request);
	}

	static void run(HookRequest request) {
		ran.add(request);
	}

	static void forceRedrawAllRectangles() {
	}
}

class Player {
	int xFine, yFine;
}

class PlayerList {
	static Player self;
}

class Flames {
	static void load(Js5 archive) {
	}
}

class FogManager {
	static void setInstantFade() {
	}
}

class SeqType {
	int[] frameDelay, frames;
	int replayoff;
}

class SeqTypeList {
	static SeqType get(int id) {
		return null;
	}
}

class CursorType {
	int hotSpotX, hotSpotY;

	SoftwareSprite getSprite() {
		return null;
	}
}

class CursorTypeList {
	static CursorType get(int id) {
		return new CursorType();
	}
}

class Preferences {
	static boolean cursorsEnabled = true;
}

// --- Font, SoftwareRaster and JagString ---------------------------------

class SpriteLoader {
	static int[] xOffsets, yOffsets, innerWidths, innerHeights;
	static byte[][] pixels;

	static boolean decode(Js5 archive, int file, int group) {
		return false;
	}

	static void clear() {
	}
}

class GlFont extends Font {
	GlFont(byte[] data, int[] xOffsets, int[] yOffsets, int[] innerWidths, int[] innerHeights, byte[][] pixels) {
		super(data, xOffsets, yOffsets, innerWidths, innerHeights);
	}

	static void clearLineMask() {
	}

	@Override
	protected void renderGlyph(int glyph, int x, int y, int w, int h, int color) {
	}

	@Override
	protected void renderGlyphTransparent(int glyph, int x, int y, int w, int h, int color, int alpha) {
	}
}

class FrameBuffer {
}

class BrowserControl {
	static void eval(Object applet, Object script) {
	}

	static Object call(Object name, Object applet) {
		return null;
	}
}

class Rasteriser {
	static boolean textureHasTransparency = false, jagged = true;

	static int getOffsetRemainder() {
		return 0;
	}

	static int getOffset() {
		return 0;
	}

	static void testPoints(int a, int b, int c) {
	}

	static void fillTriangle(int ya, int yb, int yc, int xa, int xb, int xc, int color) {
	}

	static void prepare() {
	}

	static void setBounds(int right, int bottom) {
	}
}

// --- Inv.renderObjectSprite ----------------------------------------------

class Sprite extends SecondaryNode {
	int innerWidth, innerHeight;

	void render(int x, int y) {
	}
}

/** A bare canvas. The oracle stages no outline, shadow, note or lent item, so these do nothing. */
class SoftwareSprite extends Sprite {
	int[] pixels;

	SoftwareSprite(int width, int height) {
		this.pixels = new int[width * height];
		this.innerWidth = width;
		this.innerHeight = height;
	}

	void drawOutline(int rgb) {
	}

	void drawShadow(int rgb) {
	}

	Object toFullImage() {
		return null;
	}
}

class GlSprite extends Sprite {
	GlSprite(SoftwareSprite source) {
	}
}

/** Stands in for the item model: "rendering" copies a staged picture onto the 36 by 32 canvas. */
class SoftwareModel {
	static int[] picture;

	int getMinY() {
		return 0;
	}

	void setCamera(int yaw, int roll, int pitch, int offsetX, int offsetY, int offsetZ, long key) {
		System.arraycopy(picture, 0, SoftwareRaster.pixels, 0, picture.length);
	}
}

class ObjType {
	int[] countco, countobj;
	HashTable params;
	int certtemplate = -1, certlink = -1, lentTemplate = -1, lentLink = -1;
	int zoom2d = 2000, xAngle2D, yAngle2D, zAngle2D, xOffset2D, yOffset2D, stackable;

	SoftwareModel getInvModel() {
		return new SoftwareModel();
	}
}

/** A cache that never holds anything, so every request renders. */
class SoftLruHashTable {
	SoftLruHashTable(int capacity) {
	}

	Object get(long key) {
		return null;
	}

	void put(Object value, long key) {
	}
}

class ObjTypeList {
	static final SoftLruHashTable objectSpriteCache = new SoftLruHashTable(100);
	static final java.util.Map<Integer, ObjType> staged = new java.util.HashMap<>();
	static SoftwareFont font;
	static int capacity;

	static ObjType get(int id) {
		return staged.get(id);
	}
}

class InvType {
	int size;
}

class InvTypeList {
	static InvType get(int id) {
		return new InvType();
	}
}

class IntNode extends Node {
	int value;
}

class MathUtils {
	static final int[] sin = new int[2048], cos = new int[2048];
}
