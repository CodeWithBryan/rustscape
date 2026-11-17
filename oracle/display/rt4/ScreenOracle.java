package rt4;

import java.awt.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;

/**
 * The unchanged rt4.DisplayMode over one real screen's mode list: the list a monitor gave
 * FullScreenManager.getDisplayModes (recorded by `cargo run -p rs09-client --example screen_modes --
 * --record` into fixtures/display/screen-modes.json, member "jvm"), filtered and sorted by
 * getDisplayModes, and full screen entered at every size it lists, the way CS2 5300 does.
 *
 * usage: ScreenOracle <screen-modes.json>
 *
 * Output: one JSON object per line in DisplayOracle's format (the same "start" and "steps" records,
 * made by DisplayOracle.observed), so the same replay reads both. Each case starts from
 * GameShell.startApplication for one frame size and remembered mode and runs: mainLoad 150's
 * setWindowMode, 5302/5303/5305, then for every listed mode 5300 at its size followed by the list
 * again, with 5301 after every third, one 5300 at a size the screen lacks, ::wm3's 1024 by 768 and a
 * last 5301. The screen answers every request at once and the hardware renderer starts.
 */
public class ScreenOracle {

	/** The int array of the member "jvm": width, height, bit depth and refresh rate of each mode. */
	static int[] recorded(String path) throws Exception {
		String text = new String(Files.readAllBytes(Paths.get(path)), StandardCharsets.UTF_8);
		int key = text.indexOf("\"jvm\"");
		int open = text.indexOf('[', key);
		int close = text.indexOf(']', open);
		String[] parts = text.substring(open + 1, close).split(",");
		int count = 0;
		int[] values = new int[parts.length];
		for (String part : parts) {
			String trimmed = part.trim();
			if (!trimmed.isEmpty()) {
				values[count++] = Integer.parseInt(trimmed);
			}
		}
		int[] result = new int[count];
		System.arraycopy(values, 0, result, 0, count);
		return result;
	}

	static String modes() {
		// ScriptRunner.java:4663-4690, as DisplayOracle's "modes" step.
		DisplayMode[] displayModes = DisplayMode.getDisplayModes();
		StringBuilder result = new StringBuilder("{\"modes\":[");
		for (int i = 0; i < displayModes.length; i++) {
			if (i > 0) {
				result.append(',');
			}
			result.append(DisplayOracle.ints(new int[]{displayModes[i].width, displayModes[i].height, displayModes[i].bitDepth,
				displayModes[i].refreshRate}));
		}
		int int3 = Preferences.fullScreenHeight;
		int int1 = Preferences.fullScreenWidth;
		int int2 = -1;
		for (int c = 0; c < displayModes.length; c++) {
			DisplayMode displayMode = displayModes[c];
			if (int1 == displayMode.width && displayMode.height == int3) {
				int2 = c;
				break;
			}
		}
		return result.append("],\"preferred\":").append(int2).append('}').toString();
	}

	static String fullScreen(int width, int height) {
		// ScriptRunner.java:4649-4656.
		DisplayMode.setWindowMode(false, 3, width, height);
		int result = GameShell.fullScreenFrame == null ? 0 : 1;
		return DisplayOracle.observed("[\"5300\"," + width + "," + height + "]", String.valueOf(result));
	}

	static String leave() {
		// ScriptRunner.java:4657-4662.
		if (GameShell.fullScreenFrame != null) {
			DisplayMode.setWindowMode(false, Preferences.favoriteWorlds, -1, -1);
		}
		return DisplayOracle.observed("[\"leave\"]", null);
	}

	public static void main(String[] args) throws Exception {
		int[] screen = recorded(args[0]);
		int[][] windows = {{1024, 768}, {765, 503}, {1920, 1080}};
		int id = 0;
		for (int[] window : windows) {
			for (int remembered = 0; remembered < 3; remembered++) {
				SignLink signLink = new SignLink();
				signLink.modes = screen.clone();
				GlRenderer.starts = true;
				GlRenderer.enabled = false;
				DisplayMode.cachedDisplayModes = null;
				DisplayMode.start_GLRenderer = false;
				DisplayMode.resizable = false;
				DisplayMode.canvasReplaceTime = 0L;
				GameShell.signLink = signLink;
				GameShell.frame = new Frame();
				GameShell.frame.setSize(window[0], window[1]);
				GameShell.fullScreenFrame = null;
				GameShell.canvas = new LogCanvas();
				GameShell.thread = new Thread();
				GameShell.leftMargin = 0;
				GameShell.topMargin = 0;
				GameShell.frameWidth = window[0];
				GameShell.frameHeight = window[1];
				GameShell.canvasWidth = GameShell.frameWidth;
				GameShell.canvasHeight = GameShell.frameHeight;
				GameShell.replaceCanvas = false;
				Preferences.favoriteWorlds = remembered;
				Preferences.antiAliasingMode = remembered;
				Preferences.writes = 0;
				// The first case of each frame has a size saved that the screen lists.
				Preferences.fullScreenWidth = remembered == 0 ? 1024 : 0;
				Preferences.fullScreenHeight = remembered == 0 ? 768 : 0;
				OracleLog.events.clear();

				StringBuilder line = new StringBuilder("{\"case\":\"screen\",\"id\":").append(id++);
				line.append(",\"window\":").append(DisplayOracle.ints(window));
				line.append(",\"supported\":").append(signLink.fullScreenSupported);
				line.append(",\"modes\":").append(DisplayOracle.ints(signLink.modes));
				line.append(",\"enter\":").append(signLink.enter);
				line.append(",\"starts\":").append(GlRenderer.starts);
				line.append(",\"remembered\":").append(Preferences.favoriteWorlds);
				line.append(",\"antiAliasing\":").append(Preferences.antiAliasingMode);
				line.append(",\"fullScreen\":").append(DisplayOracle.ints(new int[]{Preferences.fullScreenWidth, Preferences.fullScreenHeight}));
				line.append(",\"start\":").append(DisplayOracle.observed("[\"start\"]", null));
				line.append(",\"steps\":[");
				// client.mainLoad state 150.
				DisplayMode.setWindowMode(false, Preferences.favoriteWorlds, -1, -1);
				line.append(DisplayOracle.observed("[\"set\",0," + remembered + ",-1,-1]", null));
				line.append(',').append(DisplayOracle.observed("[\"modes\"]", modes()));
				DisplayMode[] listed = DisplayMode.getDisplayModes();
				for (int i = 0; i < listed.length; i++) {
					line.append(',').append(fullScreen(listed[i].width, listed[i].height));
					line.append(',').append(DisplayOracle.observed("[\"modes\"]", modes()));
					if (i % 3 == 2) {
						line.append(',').append(leave());
					}
				}
				line.append(',').append(fullScreen(1000, 700));
				DisplayMode.setWindowMode(false, 3, 1024, 768);
				line.append(',').append(DisplayOracle.observed("[\"set\",0,3,1024,768]", null));
				line.append(',').append(leave());
				ScreenOracle.out.println(line.append("]}"));
			}
		}
		ScreenOracle.out.flush();
		System.exit(0);
	}

	static final java.io.PrintStream out = System.out;
}
