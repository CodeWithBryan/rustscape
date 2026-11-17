package rt4;

import java.awt.*;
import java.io.PrintStream;
import java.util.Random;

/**
 * Observations of the unchanged rt4.DisplayMode: the window-mode state machine, the canvas it places and
 * the screen modes it lists.
 *
 * usage: DisplayOracle <seed> <cases>
 *
 * One JSON object per line on standard output: a start (the frame's size, what the screen and the
 * renderers answer, the preferences) and a sequence of steps. Each step is one call made the way the
 * client makes it, with everything DisplayMode then asked of the adapters in order ("log"), the state
 * afterwards and the rectangles GameShell.paintFrameLetterbox would fill ("bands").
 *
 * A state is: getWindowMode(), frameWidth, frameHeight, canvasWidth, canvasHeight, leftMargin,
 * topMargin, a full-screen frame exists, GlRenderer.enabled, DisplayMode.resizable,
 * Preferences.fullScreenWidth, Preferences.fullScreenHeight, the number of Preferences.write calls,
 * GameShell.replaceCanvas, DisplayMode.start_GLRenderer.
 *
 * The steps' own code is copied from the callers, which cannot be compiled alone:
 *   set      DisplayMode.setWindowMode(replaceCanvas, mode, width, height)
 *   redraw   client.mainRedraw's replaceCanvas half (client.java:817-819), game state 30
 *   resize   the frame is resized, then client.mainRedraw's size watch (client.java:820-842)
 *   leave    ScriptRunner opcode 5301 (ScriptRunner.java:4657-4662)
 *   5300     ScriptRunner opcode 5300 (4649-4656)
 *   5307     ScriptRunner opcode 5307 (4695-4704)
 *   modes    ScriptRunner opcodes 5302, 5303 and 5305 (4663-4690)
 *   flag     GameShell.replaceCanvas = true, as opcode 6016 does (5182-5184)
 *   remember Preferences.favoriteWorlds = value;  aa  Preferences.antiAliasingMode = value
 *
 * No window is shown: frames and the canvas are created and sized but never made visible.
 */
public class DisplayOracle {
	static final PrintStream out = System.out;

	static final int[][] WINDOWS = {{765, 503}, {1024, 768}, {1280, 800}, {700, 400}, {766, 503}, {767, 600},
		{300, 200}, {1920, 1080}, {764, 502}, {800, 600}, {1, 1}, {765, 1000}, {2000, 503}};
	static final int[][] SIZES = {{640, 480}, {800, 600}, {1024, 768}, {1280, 720}, {1280, 1024}, {1366, 768},
		{1600, 900}, {1200, 1200}, {1440, 1000}, {1000, 1440}, {1920, 1080}, {799, 600}, {800, 599}, {900, 1600}};
	static final int[] DEPTHS = {-1, 0, 8, 16, 24, 32};
	static final int[] RATES = {0, 60, 75};

	static int b(boolean value) {
		return value ? 1 : 0;
	}

	static String ints(int[] values) {
		StringBuilder sb = new StringBuilder("[");
		for (int i = 0; i < values.length; i++) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append(values[i]);
		}
		return sb.append(']').toString();
	}

	static int[] state() {
		return new int[]{DisplayMode.getWindowMode(), GameShell.frameWidth, GameShell.frameHeight, GameShell.canvasWidth,
			GameShell.canvasHeight, GameShell.leftMargin, GameShell.topMargin, b(GameShell.fullScreenFrame != null),
			b(GlRenderer.enabled), b(DisplayMode.resizable), Preferences.fullScreenWidth, Preferences.fullScreenHeight,
			Preferences.writes, b(GameShell.replaceCanvas), b(DisplayMode.start_GLRenderer)};
	}

	/** The record of one step: what was logged, the state and the letterbox. */
	static String observed(String op, String result) {
		StringBuilder sb = new StringBuilder("{\"op\":").append(op).append(",\"log\":[");
		for (int i = 0; i < OracleLog.events.size(); i++) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append('"').append(OracleLog.events.get(i)).append('"');
		}
		OracleLog.events.clear();
		sb.append("],\"state\":").append(ints(state()));
		OracleLog.fills.clear();
		GameShell.paintFrameLetterbox();
		OracleLog.events.clear();
		sb.append(",\"bands\":[");
		for (int i = 0; i < OracleLog.fills.size(); i++) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append(ints(OracleLog.fills.get(i)));
		}
		sb.append(']');
		if (result != null) {
			sb.append(",\"result\":").append(result);
		}
		return sb.append('}').toString();
	}

	static int[] size(Random r) {
		if (r.nextInt(4) == 0) {
			return new int[]{1 + r.nextInt(2200), 1 + r.nextInt(1400)};
		}
		return WINDOWS[r.nextInt(WINDOWS.length)];
	}

	/** A size to ask full screen for: one the screen has, more often than not. */
	static int[] fullScreenSize(Random r, SignLink signLink) {
		if (signLink.modes != null && signLink.modes.length >= 4 && r.nextInt(4) != 0) {
			int index = r.nextInt(signLink.modes.length >> 2) << 2;
			return new int[]{signLink.modes[index], signLink.modes[index + 1]};
		}
		return SIZES[r.nextInt(SIZES.length)];
	}

	public static void main(String[] args) {
		Random r = new Random(Long.parseLong(args[0]));
		int cases = Integer.parseInt(args[1]);
		for (int id = 0; id < cases; id++) {
			// GameShell.startApplication (GameShell.java:684-690) for a frame of this size.
			int[] window = size(r);
			SignLink signLink = new SignLink();
			signLink.fullScreenSupported = r.nextInt(8) != 0;
			if (r.nextInt(10) == 0) {
				signLink.modes = null;
			} else {
				int count = r.nextInt(9);
				signLink.modes = new int[count << 2];
				for (int i = 0; i < count; i++) {
					int[] s = SIZES[r.nextInt(SIZES.length)];
					signLink.modes[i << 2] = s[0];
					signLink.modes[(i << 2) + 1] = s[1];
					signLink.modes[(i << 2) + 2] = DEPTHS[r.nextInt(DEPTHS.length)];
					signLink.modes[(i << 2) + 3] = RATES[r.nextInt(RATES.length)];
				}
			}
			signLink.enter = r.nextInt(4) == 0 ? 1 + r.nextInt(2) : 0;
			GlRenderer.starts = r.nextInt(8) != 0;
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
			Preferences.favoriteWorlds = r.nextInt(3);
			Preferences.antiAliasingMode = r.nextInt(3);
			Preferences.writes = 0;
			if (r.nextInt(3) == 0) {
				int[] s = fullScreenSize(r, signLink);
				Preferences.fullScreenWidth = s[0];
				Preferences.fullScreenHeight = s[1];
			} else {
				Preferences.fullScreenWidth = 0;
				Preferences.fullScreenHeight = 0;
			}
			OracleLog.events.clear();

			StringBuilder line = new StringBuilder("{\"case\":\"display\",\"id\":").append(id);
			line.append(",\"window\":").append(ints(window));
			line.append(",\"supported\":").append(signLink.fullScreenSupported);
			line.append(",\"modes\":").append(signLink.modes == null ? "null" : ints(signLink.modes));
			line.append(",\"enter\":").append(signLink.enter);
			line.append(",\"starts\":").append(GlRenderer.starts);
			line.append(",\"remembered\":").append(Preferences.favoriteWorlds);
			line.append(",\"antiAliasing\":").append(Preferences.antiAliasingMode);
			line.append(",\"fullScreen\":").append(ints(new int[]{Preferences.fullScreenWidth, Preferences.fullScreenHeight}));
			line.append(",\"start\":").append(observed("[\"start\"]", null));
			line.append(",\"steps\":[");

			int steps = 4 + r.nextInt(11);
			for (int step = 0; step < steps; step++) {
				if (step > 0) {
					line.append(',');
				}
				int kind = r.nextInt(20);
				if (kind < 6) {
					boolean replace = r.nextInt(6) == 0;
					int mode = r.nextInt(4);
					int width = -1;
					int height = -1;
					if (mode == 3) {
						int[] s = fullScreenSize(r, signLink);
						width = s[0];
						height = s[1];
					}
					DisplayMode.setWindowMode(replace, mode, width, height);
					line.append(observed("[\"set\"," + b(replace) + "," + mode + "," + width + "," + height + "]", null));
				} else if (kind < 8) {
					// client.mainRedraw (client.java:817-819) with gameState 30; the timer's half is not run.
					if (GameShell.replaceCanvas) {
						DisplayMode.setWindowMode(GameShell.replaceCanvas, DisplayMode.getWindowMode(), Preferences.fullScreenWidth, Preferences.fullScreenHeight);
					}
					line.append(observed("[\"redraw\"]", null));
				} else if (kind < 12) {
					int[] s = size(r);
					GameShell.frame.setSize(s[0], s[1]);
					// client.mainRedraw (client.java:820-842).
					boolean timer = false;
					int w;
					int h;
					if (GameShell.fullScreenFrame == null) {
						Container container;
						if (GameShell.fullScreenFrame != null) {
							container = GameShell.fullScreenFrame;
						} else if (GameShell.frame == null) {
							container = GameShell.signLink.applet;
						} else {
							container = GameShell.frame;
						}
						w = container.getSize().width;
						h = container.getSize().height;
						if (container == GameShell.frame) {
							Insets insets = GameShell.frame.getInsets();
							w -= insets.right + insets.left;
							h -= insets.top + insets.bottom;
						}
						if (w != GameShell.frameWidth || h != GameShell.frameHeight) {
							GameShell.updateCanvasSize();
							timer = true;
						}
					}
					line.append(observed("[\"resize\"," + s[0] + "," + s[1] + "]", String.valueOf(b(timer))));
				} else if (kind < 13) {
					// ScriptRunner.java:4657-4662.
					if (GameShell.fullScreenFrame != null) {
						DisplayMode.setWindowMode(false, Preferences.favoriteWorlds, -1, -1);
					}
					line.append(observed("[\"leave\"]", null));
				} else if (kind < 15) {
					// ScriptRunner.java:4649-4656.
					int[] s = fullScreenSize(r, signLink);
					int int1 = s[0];
					int int3 = s[1];
					DisplayMode.setWindowMode(false, 3, int1, int3);
					int result = GameShell.fullScreenFrame == null ? 0 : 1;
					line.append(observed("[\"5300\"," + int1 + "," + int3 + "]", String.valueOf(result)));
				} else if (kind < 16) {
					// ScriptRunner.java:4695-4704.
					int argument = r.nextInt(6) - 1;
					int int1 = argument;
					if (int1 < 0 || int1 > 2) {
						int1 = 0;
					}
					DisplayMode.setWindowMode(false, int1, -1, -1);
					line.append(observed("[\"5307\"," + argument + "]", null));
				} else if (kind < 17) {
					// ScriptRunner.java:4663-4690: 5302 is the length, 5303 a mode's width and height,
					// 5305 the index of the mode of the saved size.
					DisplayMode[] displayModes = DisplayMode.getDisplayModes();
					StringBuilder result = new StringBuilder("{\"modes\":[");
					for (int i = 0; i < displayModes.length; i++) {
						if (i > 0) {
							result.append(',');
						}
						result.append(ints(new int[]{displayModes[i].width, displayModes[i].height, displayModes[i].bitDepth,
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
					result.append("],\"preferred\":").append(int2).append('}');
					line.append(observed("[\"modes\"]", result.toString()));
				} else if (kind < 18) {
					GameShell.replaceCanvas = true;
					line.append(observed("[\"flag\"]", null));
				} else if (kind < 19) {
					Preferences.favoriteWorlds = r.nextInt(3);
					line.append(observed("[\"remember\"," + Preferences.favoriteWorlds + "]", null));
				} else {
					Preferences.antiAliasingMode = r.nextInt(3);
					line.append(observed("[\"aa\"," + Preferences.antiAliasingMode + "]", null));
				}
			}
			out.println(line.append("]}"));
		}
		out.flush();
		// AWT may have started threads of its own.
		System.exit(0);
	}
}
