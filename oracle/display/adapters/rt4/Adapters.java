package rt4;

import java.awt.*;

/*
 * Oracle adapters for the unchanged rt4.DisplayMode: the classes it calls, reduced to what it touches.
 * Each records the call (OracleLog) or does nothing; none draws, opens a window or changes the screen.
 * What the real classes do with these calls is not observed.
 */

/** The canvas: its size and place are recorded. Never shown. */
class LogCanvas extends Canvas {
	@Override
	public void setSize(int width, int height) {
		OracleLog.add("setSize " + width + " " + height);
		super.setSize(width, height);
	}

	@Override
	public void setLocation(int x, int y) {
		OracleLog.add("setLocation " + x + " " + y);
		super.setLocation(x, y);
	}
}

/** GlRenderer: `enabled`, and whether init leaves it on, which the driver decides. */
class GlRenderer {
	static boolean enabled = false;
	/** Driver input: GlRenderer.init succeeds. */
	static boolean starts = true;

	static void quit() {
		OracleLog.add("quit");
		enabled = false;
	}

	static void setCanvasSize(int width, int height) {
		OracleLog.add("setCanvasSize " + width + " " + height);
	}

	static void createAndDestroyContext(Canvas canvas) {
		OracleLog.add("createAndDestroyContext");
	}

	static void init(Canvas canvas, int samples) {
		OracleLog.add("init " + samples);
		enabled = starts;
	}
}

/** SignLink: the three full-screen requests answer at once with what the driver set. */
class SignLink {
	Container applet;
	/** Driver inputs. */
	boolean fullScreenSupported = true;
	/** Width, height, bit depth and refresh rate of each mode; null fails the request (status 2). */
	int[] modes = new int[0];
	/** 0: a frame, status 1. 1: no frame. 2: a frame with status 2. */
	int enter = 0;

	boolean isFullScreenSupported() {
		return this.fullScreenSupported;
	}

	PrivilegedRequest getDisplayModes() {
		PrivilegedRequest request = new PrivilegedRequest();
		if (this.modes == null) {
			request.status = 2;
		} else {
			request.result = this.modes.clone();
			request.status = 1;
		}
		return request;
	}

	PrivilegedRequest enterFullScreen(int bitDepth, int height, int width) {
		OracleLog.add("enterFullScreen " + bitDepth + " " + height + " " + width);
		PrivilegedRequest request = new PrivilegedRequest();
		if (this.enter != 1) {
			// As FullScreenManager.enter leaves it: undecorated, at the mode's size. Never shown.
			Frame frame = new Frame("Jagex Full Screen");
			frame.setUndecorated(true);
			frame.setSize(width, height);
			request.result = frame;
		}
		request.status = this.enter == 2 ? 2 : 1;
		return request;
	}

	PrivilegedRequest exitFullScreen(Frame frame) {
		OracleLog.add("exitFullScreen");
		PrivilegedRequest request = new PrivilegedRequest();
		request.status = 1;
		return request;
	}
}

/** Preferences: the fields DisplayMode reads and writes; the write is recorded and counted. */
class Preferences {
	static int antiAliasingMode = 0;
	static int fullScreenHeight = 0;
	static int fullScreenWidth = 0;
	static int favoriteWorlds = 0;
	static int brightness = 3;
	static boolean highDetailLighting = true;
	static int writes = 0;

	static void write(SignLink signLink) {
		OracleLog.add("write");
		writes++;
	}
}

class Keyboard {
	static void stop(Component component) {
	}

	static void start(Component component) {
	}
}

class Mouse {
	static void stop(Component component) {
	}

	static void start(Component component) {
	}
}

class MouseWheel {
	void stop(Component component) {
	}

	void start(Component component) {
	}
}

/** client: addCanvas is recorded and clears GameShell.replaceCanvas, as GameShell.addCanvas does (line 353). */
class client {
	static client instance = new client();
	static MouseWheel mouseWheel = new MouseWheel();
	static int gameState = 30;

	void addCanvas() {
		OracleLog.add("addCanvas");
		GameShell.replaceCanvas = false;
	}

	static void reloadResourcesOnDisplayModeChange() {
		OracleLog.add("reload");
	}
}

class SceneGraph {
	static boolean levelsHidden = false;

	static void clear() {
	}

	static boolean allLevelsAreVisible() {
		return GlRenderer.enabled;
	}
}

class FrameBuffer {
	static FrameBuffer create(int height, int width, Component component) {
		return new FrameBuffer();
	}

	void draw(Graphics graphics) {
	}
}

class SoftwareRaster {
	static FrameBuffer frameBuffer;

	static void clear() {
	}
}

class Font {
}

class JagString {
}

class LocalizedText {
	static final JagString LOADING = new JagString();
}

/** The loading text of the hardware renderer's first start, whichever of the two draws it. */
class LoadingBar {
	static void render(boolean flag, Font font) {
		OracleLog.add("loading");
	}
}

class Fonts {
	static Font b12Full = new Font();

	static void drawTextOnScreen(boolean flag, JagString text) {
		OracleLog.add("loading");
	}
}

/** The two calls that only the blocks after a change of renderer make. */
class SoftwareModel {
	static void enableDepthSortMode() {
		OracleLog.add("hd");
	}

	static void enableBucketSortMode() {
		OracleLog.add("sd");
	}
}

class Js5GlTextureProvider {
	void setCapacity(int capacity) {
	}
}

class Rasteriser {
	static Object textureProvider = new Js5GlTextureProvider();

	static void setBrightness(float brightness) {
	}
}

class LoginManager {
	static void clearLoginScreenSprites() {
	}
}

class ParticleSystem {
	static void quit() {
	}
}

class GlTile {
	static void resetStaticBuffers() {
	}
}

class InterfaceList {
	/** An interface is open, so every setWindowMode lays it out. */
	static int topLevelInterface = 0;
	static boolean[] rectangleDirty = new boolean[100];

	static void layoutTopLevel(boolean forceResize) {
		OracleLog.add("layout " + forceResize);
	}
}

/** The outgoing buffer of ClientProt.sendWindowDetails: opcode and bytes are recorded. */
class RecordingBuffer {
	final StringBuilder bytes = new StringBuilder();

	void p1isaac(int opcode) {
		this.bytes.append(opcode);
	}

	void p1(int value) {
		this.bytes.append(' ').append(value & 0xFF);
	}

	void p2(int value) {
		this.bytes.append(' ').append(value >> 8 & 0xFF).append(' ').append(value & 0xFF);
	}
}

class Protocol {
	/** The game socket is open (and client.gameState is 30), so every setWindowMode reports. */
	static Object socket = new Object();
	static RecordingBuffer outboundBuffer = new RecordingBuffer();
}

/** ClientProt.sendWindowDetails, copied from ClientProt.java:190-196 (WINDOW_STATUS is 243). */
class ClientProt {
	static final int WINDOW_STATUS = 243;

	static void sendWindowDetails() {
		Protocol.outboundBuffer = new RecordingBuffer();
		Protocol.outboundBuffer.p1isaac(ClientProt.WINDOW_STATUS);
		Protocol.outboundBuffer.p1(DisplayMode.getWindowMode());
		Protocol.outboundBuffer.p2(GameShell.canvasWidth);
		Protocol.outboundBuffer.p2(GameShell.canvasHeight);
		Protocol.outboundBuffer.p1(Preferences.antiAliasingMode);
		OracleLog.add("status " + Protocol.outboundBuffer.bytes);
	}
}
