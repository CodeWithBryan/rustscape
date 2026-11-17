package refcompare;

import java.awt.Component;
import java.awt.event.InputEvent;
import java.awt.event.KeyEvent;
import java.awt.event.KeyListener;
import java.awt.event.MouseEvent;
import java.awt.event.MouseListener;
import java.awt.event.MouseMotionListener;
import java.io.File;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.Array;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardOpenOption;
import java.util.List;

/**
 * Launcher for the UNCHANGED original client used by the frame comparison.
 * It starts {@code rt4.client.main} and, from a second thread, runs commands
 * read from a file, so the original can be driven and inspected without
 * window focus and without touching its sources:
 *
 * <ul>
 * <li>input goes straight to the listeners the client registered on its
 * canvas ({@code rt4.Keyboard}, {@code rt4.Mouse}), the same calls AWT makes;
 * <li>{@code get}/{@code set} read and write the client's public static
 * fields by reflection (camera angles, colour jitter, game state).
 * </ul>
 *
 * Protocol: write {@code <dir>/cmd.txt} (one command per line; write it under
 * another name and rename). The driver runs the lines in order, appends what
 * they print to {@code <dir>/out.txt}, deletes {@code cmd.txt} and then
 * creates {@code <dir>/done.txt}. {@code <dir>} is the system property
 * {@code refcompare.dir}.
 *
 * <pre>
 * sleep MS
 * move X Y | press X Y [right] | release X Y [right] | click X Y [right]
 * keydown VK | keyup VK | tap VK [HOLD_MS]      VK: a KeyEvent.VK_ name (LEFT, ENTER, F1 ...) or number
 * type TEXT...                                  each char: pressed, typed, released
 * get Class.field ...                           e.g. get Camera.yawTarget client.gameState
 * aget Class.field I J ...                      an element of an array field, e.g. aget Camera.cameraPathData 3
 * oget Class.field STEP ...                     array indices and instance fields, e.g. oget SceneGraph.tiles 0 55 44 wall
 * call Class.method INT ...                     a static method taking ints, e.g. call LocTypeList.get 1902
 * set Class.field VALUE                         int, long, float, double or boolean fields
 * hold Class.field VALUE | unhold               re-apply a value every 5 ms until unhold
 * frame                                         the lines after it run on the client's own thread,
 *                                               between two frames (see FramePlugin)
 * tiles LEVEL X0 X1 Y0 Y1                       what SceneGraph.tiles holds on those tiles: walls,
 *                                               wall decor, ground decor and scenery with their
 *                                               location ids, shapes, rotations and extents
 * heights LEVEL X0 X1 Y0 Y1                     SceneGraph.tileHeights[LEVEL] there, one row per x
 * </pre>
 */
public final class Driver implements Runnable {
	private final File dir;
	private final java.util.Map<Field, String> held = new java.util.concurrent.ConcurrentHashMap<>();
	private static final java.util.concurrent.LinkedBlockingQueue<Runnable> frameWork = new java.util.concurrent.LinkedBlockingQueue<>();

	/**
	 * A plugin of the client's own plugin system ({@code plugin.PluginRepository},
	 * part of the reference sources): {@code client.mainRedraw} calls
	 * {@code PluginRepository.LateDraw()} at the end of every frame, on the
	 * thread that builds and draws the scene, so work queued here reads the
	 * scene between two frames instead of racing the renderer. It draws
	 * nothing and changes nothing; it is registered before {@code client.main}
	 * runs (so the plugin map is never written while the client iterates it)
	 * and stays across plugin reloads.
	 */
	public static final class FramePlugin extends plugin.Plugin {
		@Override
		public void LateDraw(long timeDelta) {
			Runnable work;
			while ((work = frameWork.poll()) != null) {
				work.run();
			}
		}

		@Override
		public boolean OnPluginsReloaded() {
			return true;
		}
	}

	private Driver(File dir) {
		this.dir = dir;
	}

	public static void main(String[] args) throws Exception {
		String dir = System.getProperty("refcompare.dir");
		if (dir != null) {
			plugin.PluginRepository.registerPlugin(new plugin.PluginInfo("refcompare", "reads between frames", 1.0), new FramePlugin());
			File folder = new File(dir);
			folder.mkdirs();
			Driver driver = new Driver(folder);
			Thread thread = new Thread(driver, "refcompare-driver");
			thread.setDaemon(true);
			thread.start();
			Thread holder = new Thread(driver::holdLoop, "refcompare-hold");
			holder.setDaemon(true);
			holder.start();
		}
		rt4.client.main(args);
	}

	private void holdLoop() {
		while (true) {
			try {
				for (java.util.Map.Entry<Field, String> entry : held.entrySet()) {
					assign(entry.getKey(), entry.getValue());
				}
				Thread.sleep(5L);
			} catch (Throwable ignored) {
			}
		}
	}

	@Override
	public void run() {
		File cmd = new File(dir, "cmd.txt");
		File out = new File(dir, "out.txt");
		File done = new File(dir, "done.txt");
		while (true) {
			try {
				Thread.sleep(20L);
				if (!cmd.exists()) {
					continue;
				}
				List<String> lines = Files.readAllLines(cmd.toPath(), StandardCharsets.UTF_8);
				StringBuilder result = new StringBuilder();
				for (int n = 0; n < lines.size(); n++) {
					String line = lines.get(n).replace("﻿", "").trim();
					if (line.isEmpty() || line.startsWith("#")) {
						continue;
					}
					if (line.equals("frame")) {
						// The rest of the file runs inside the client's next LateDraw.
						List<String> rest = lines.subList(n + 1, lines.size());
						StringBuilder inFrame = new StringBuilder();
						java.util.concurrent.CountDownLatch ran = new java.util.concurrent.CountDownLatch(1);
						frameWork.add(() -> {
							runLines(rest, inFrame);
							ran.countDown();
						});
						if (ran.await(20, java.util.concurrent.TimeUnit.SECONDS)) {
							result.append(inFrame);
						} else {
							result.append("ERROR frame: no frame within 20 s\n");
						}
						break;
					}
					runLines(java.util.Collections.singletonList(line), result);
				}
				Files.write(out.toPath(), result.toString().getBytes(StandardCharsets.UTF_8), StandardOpenOption.CREATE, StandardOpenOption.APPEND);
				cmd.delete();
				done.createNewFile();
			} catch (Throwable ex) {
				ex.printStackTrace();
			}
		}
	}

	private void runLines(List<String> lines, StringBuilder result) {
		for (String raw : lines) {
			String line = raw.replace("﻿", "").trim();
			if (line.isEmpty() || line.startsWith("#")) {
				continue;
			}
			try {
				String printed = execute(line);
				if (printed != null) {
					result.append(printed).append('\n');
				}
			} catch (Throwable ex) {
				StringWriter trace = new StringWriter();
				ex.printStackTrace(new PrintWriter(trace));
				result.append("ERROR ").append(line).append(": ").append(trace).append('\n');
			}
		}
	}

	/** The location id, shape and rotation a scene key carries (SceneGraph.addLoc). */
	private static String keyText(long key) {
		int id = (int) (key >>> 32) & 0x7fffffff;
		int shape = (int) (key >> 14) & 0x3f;
		int rotation = (int) (key >> 20) & 0x3;
		int x = (int) key & 0x7f;
		int y = (int) (key >> 7) & 0x7f;
		return "loc " + id + " shape " + shape + " rot " + rotation + " at " + x + "," + y;
	}

	private static String entityText(Object entity) {
		if (entity == null) {
			return "-";
		}
		String text = entity.getClass().getSimpleName();
		if (entity instanceof rt4.Model) {
			rt4.Model model = (rt4.Model) entity;
			text += " minY " + model.getMinY() + " lengthXZ " + model.getLengthXZ();
		}
		return text;
	}

	private static String tiles(int level, int x0, int x1, int y0, int y1) {
		StringBuilder sb = new StringBuilder();
		rt4.Tile[][][] tiles = rt4.SceneGraph.tiles;
		sb.append("tiles level ").append(level).append(" underwater ").append(rt4.SceneGraph.tiles == rt4.SceneGraph.underWaterGroundTiles).append('\n');
		for (int x = x0; x <= x1; x++) {
			for (int y = y0; y <= y1; y++) {
				rt4.Tile tile = tiles[level][x][y];
				if (tile == null) {
					continue;
				}
				StringBuilder t = new StringBuilder();
				if (tile.wall != null) {
					t.append(" wall[").append(keyText(tile.wall.key)).append(" flags ").append(tile.wall.primaryFlags).append('/').append(tile.wall.secondaryFlags)
						.append(" z ").append(tile.wall.zFine).append(' ').append(entityText(tile.wall.primary)).append(" | ").append(entityText(tile.wall.secondary)).append(']');
				}
				if (tile.wallDecor != null) {
					t.append(" decor[").append(keyText(tile.wallDecor.key)).append(" z ").append(tile.wallDecor.zFine).append(' ').append(entityText(tile.wallDecor.primary)).append(']');
				}
				if (tile.groundDecor != null) {
					t.append(" ground[").append(keyText(tile.groundDecor.key)).append(" z ").append(tile.groundDecor.zFine).append(' ').append(entityText(tile.groundDecor.entity)).append(']');
				}
				for (int i = 0; i < tile.sceneryLen; i++) {
					rt4.Scenery s = tile.scenery[i];
					if (s == null) {
						continue;
					}
					t.append(" scenery[").append(keyText(s.key)).append(" x ").append(s.xMin).append("..").append(s.xMax).append(" y ").append(s.yMin).append("..").append(s.yMax)
						.append(" fine ").append(s.xFine).append(',').append(s.yFine).append(',').append(s.zFine).append(' ').append(entityText(s.entity)).append(']');
				}
				t.append(tile.plainTile != null ? " plain" : "").append(tile.shapedTile != null ? " shaped" : "");
				if (tile.linkedTile != null) {
					t.append(" linked");
				}
				sb.append(x).append(',').append(y).append(':').append(t).append('\n');
			}
		}
		return sb.toString().trim();
	}

	private static String heights(int level, int x0, int x1, int y0, int y1) {
		StringBuilder sb = new StringBuilder("heights level ").append(level).append(" y ").append(y0).append("..").append(y1).append('\n');
		int[][] h = rt4.SceneGraph.tileHeights[level];
		for (int x = x0; x <= x1; x++) {
			sb.append(x).append(':');
			for (int y = y0; y <= y1; y++) {
				sb.append(' ').append(h[x][y]);
			}
			sb.append('\n');
		}
		return sb.toString().trim();
	}

	private static Component canvas() {
		return rt4.GameShell.canvas;
	}

	private String execute(String line) throws Exception {
		String[] p = line.split("\\s+");
		switch (p[0]) {
			case "sleep":
				Thread.sleep(Long.parseLong(p[1]));
				return null;
			case "move":
				mouse(MouseEvent.MOUSE_MOVED, Integer.parseInt(p[1]), Integer.parseInt(p[2]), false);
				return null;
			case "press":
				mouse(MouseEvent.MOUSE_PRESSED, Integer.parseInt(p[1]), Integer.parseInt(p[2]), p.length > 3);
				return null;
			case "release":
				mouse(MouseEvent.MOUSE_RELEASED, Integer.parseInt(p[1]), Integer.parseInt(p[2]), p.length > 3);
				return null;
			case "click": {
				int x = Integer.parseInt(p[1]);
				int y = Integer.parseInt(p[2]);
				boolean right = p.length > 3;
				mouse(MouseEvent.MOUSE_MOVED, x, y, false);
				Thread.sleep(60L);
				mouse(MouseEvent.MOUSE_PRESSED, x, y, right);
				Thread.sleep(60L);
				mouse(MouseEvent.MOUSE_RELEASED, x, y, right);
				return null;
			}
			case "keydown":
				key(KeyEvent.KEY_PRESSED, vk(p[1]), KeyEvent.CHAR_UNDEFINED);
				return null;
			case "keyup":
				key(KeyEvent.KEY_RELEASED, vk(p[1]), KeyEvent.CHAR_UNDEFINED);
				return null;
			case "tap": {
				int code = vk(p[1]);
				key(KeyEvent.KEY_PRESSED, code, KeyEvent.CHAR_UNDEFINED);
				Thread.sleep(p.length > 2 ? Long.parseLong(p[2]) : 60L);
				key(KeyEvent.KEY_RELEASED, code, KeyEvent.CHAR_UNDEFINED);
				return null;
			}
			case "type": {
				String text = line.substring(line.indexOf(' ') + 1);
				for (char c : text.toCharArray()) {
					int code = KeyEvent.getExtendedKeyCodeForChar(c);
					key(KeyEvent.KEY_PRESSED, code, c);
					key(KeyEvent.KEY_TYPED, KeyEvent.VK_UNDEFINED, c);
					Thread.sleep(45L);
					key(KeyEvent.KEY_RELEASED, code, c);
					Thread.sleep(45L);
				}
				return null;
			}
			case "get": {
				StringBuilder sb = new StringBuilder();
				for (int i = 1; i < p.length; i++) {
					sb.append(p[i]).append('=').append(read(p[i]));
					if (i + 1 < p.length) {
						sb.append(' ');
					}
				}
				return sb.toString();
			}
			case "aget": {
				// aget Class.field I J ...: one element of an array field,
				// nested arrays written out whole.
				Object value = field(p[1]).get(null);
				for (int i = 2; i < p.length && value != null; i++) {
					value = Array.get(value, Integer.parseInt(p[i]));
				}
				if (value instanceof Object[]) {
					return p[1] + "=" + java.util.Arrays.deepToString((Object[]) value);
				}
				if (value != null && value.getClass().isArray()) {
					return p[1] + "=" + java.util.Arrays.deepToString(new Object[] {value}).replaceAll("^\\[|\\]$", "");
				}
				return p[1] + "=" + value;
			}
			case "oget": {
				// oget Class.field STEP...: each step an array index or an
				// instance field name; prints what it reaches, an object as
				// its instance fields.
				Object value = field(p[1]).get(null);
				for (int i = 2; i < p.length && value != null; i++) {
					value = step(value, p[i]);
				}
				return p[1] + "=" + describe(value);
			}
			case "call": {
				// call Class.method INT...: a static method taking ints.
				int dot = p[1].lastIndexOf('.');
				Class<?> type = Class.forName("rt4." + p[1].substring(0, dot));
				for (java.lang.reflect.Method method : type.getDeclaredMethods()) {
					if (method.getName().equals(p[1].substring(dot + 1)) && method.getParameterCount() == p.length - 2) {
						Object[] args = new Object[p.length - 2];
						for (int i = 0; i < args.length; i++) {
							args[i] = Integer.parseInt(p[i + 2]);
						}
						method.setAccessible(true);
						return p[1] + "=" + describe(method.invoke(null, args));
					}
				}
				return "NO METHOD " + p[1];
			}
			case "set":
				assign(field(p[1]), p[2]);
				return "set " + p[1] + "=" + read(p[1]);
			case "hold":
				held.put(field(p[1]), p[2]);
				return "hold " + p[1] + "=" + p[2];
			case "unhold":
				held.clear();
				return "unhold";
			case "tiles":
				return tiles(Integer.parseInt(p[1]), Integer.parseInt(p[2]), Integer.parseInt(p[3]), Integer.parseInt(p[4]), Integer.parseInt(p[5]));
			case "heights":
				return heights(Integer.parseInt(p[1]), Integer.parseInt(p[2]), Integer.parseInt(p[3]), Integer.parseInt(p[4]), Integer.parseInt(p[5]));
			case "hide": {
				// hide LEVEL X0 X1 Y0 Y1: a diagnostic that CHANGES the scene: the
				// walls, decorations and scenery on those tiles are dropped until
				// the scene is built again (use inside `frame`).
				int level = Integer.parseInt(p[1]);
				int n = 0;
				for (int x = Integer.parseInt(p[2]); x <= Integer.parseInt(p[3]); x++) {
					for (int y = Integer.parseInt(p[4]); y <= Integer.parseInt(p[5]); y++) {
						rt4.Tile tile = rt4.SceneGraph.tiles[level][x][y];
						if (tile == null) {
							continue;
						}
						n += tile.sceneryLen + (tile.wall != null ? 1 : 0) + (tile.wallDecor != null ? 1 : 0) + (tile.groundDecor != null ? 1 : 0);
						tile.wall = null;
						tile.wallDecor = null;
						tile.groundDecor = null;
						for (int i = 0; i < tile.scenery.length; i++) {
							tile.scenery[i] = null;
						}
						tile.sceneryLen = 0;
						tile.allInteriorFlags = 0;
					}
				}
				return "hidden " + n;
			}
			default:
				return "UNKNOWN " + line;
		}
	}

	private static int vk(String name) throws Exception {
		if (Character.isDigit(name.charAt(0))) {
			return Integer.parseInt(name);
		}
		return KeyEvent.class.getField("VK_" + name.toUpperCase()).getInt(null);
	}

	private static void key(int id, int code, char c) {
		Component canvas = canvas();
		KeyEvent event = new KeyEvent(canvas, id, System.currentTimeMillis(), 0, code, c);
		for (KeyListener listener : canvas.getKeyListeners()) {
			if (id == KeyEvent.KEY_PRESSED) {
				listener.keyPressed(event);
			} else if (id == KeyEvent.KEY_RELEASED) {
				listener.keyReleased(event);
			} else {
				listener.keyTyped(event);
			}
		}
	}

	@SuppressWarnings("deprecation")
	private static void mouse(int id, int x, int y, boolean right) {
		Component canvas = canvas();
		int modifiers = 0;
		int button = MouseEvent.NOBUTTON;
		if (id != MouseEvent.MOUSE_MOVED) {
			// The legacy masks, which AWT turns into the extended ones the
			// client's mousePressed reads (BUTTON3_DOWN_MASK).
			modifiers = right ? InputEvent.BUTTON3_MASK : InputEvent.BUTTON1_MASK;
			button = right ? MouseEvent.BUTTON3 : MouseEvent.BUTTON1;
		}
		MouseEvent event = new MouseEvent(canvas, id, System.currentTimeMillis(), modifiers, x, y, 1, false, button);
		if (id == MouseEvent.MOUSE_MOVED) {
			for (MouseMotionListener listener : canvas.getMouseMotionListeners()) {
				listener.mouseMoved(event);
			}
		} else {
			for (MouseListener listener : canvas.getMouseListeners()) {
				if (id == MouseEvent.MOUSE_PRESSED) {
					listener.mousePressed(event);
				} else {
					listener.mouseReleased(event);
				}
			}
		}
	}

	private static Field field(String name) throws Exception {
		int dot = name.lastIndexOf('.');
		String cls = name.substring(0, dot);
		Class<?> type = Class.forName(cls.indexOf('.') < 0 ? "rt4." + cls : cls);
		Field field = type.getDeclaredField(name.substring(dot + 1));
		field.setAccessible(true);
		return field;
	}

	private static String read(String name) throws Exception {
		// Class.field or Class.field.field (one level into an object).
		String[] parts = name.split("\\.");
		Field first = field(parts[0] + "." + parts[1]);
		Object value = first.get(null);
		for (int i = 2; i < parts.length && value != null; i++) {
			Class<?> type = value.getClass();
			Field next = null;
			while (type != null && next == null) {
				try {
					next = type.getDeclaredField(parts[i]);
				} catch (NoSuchFieldException ex) {
					type = type.getSuperclass();
				}
			}
			if (next == null) {
				return "?";
			}
			next.setAccessible(true);
			value = next.get(value);
		}
		if (value != null && value.getClass().isArray()) {
			int n = Array.getLength(value);
			StringBuilder sb = new StringBuilder("[");
			for (int i = 0; i < Math.min(n, 64); i++) {
				sb.append(i == 0 ? "" : ",").append(Array.get(value, i));
			}
			return sb.append(n > 64 ? ",...]" : "]").toString();
		}
		return String.valueOf(value);
	}

	private static Object step(Object value, String step) throws Exception {
		if (Character.isDigit(step.charAt(0))) {
			return Array.get(value, Integer.parseInt(step));
		}
		for (Class<?> type = value.getClass(); type != null; type = type.getSuperclass()) {
			try {
				Field next = type.getDeclaredField(step);
				next.setAccessible(true);
				return next.get(value);
			} catch (NoSuchFieldException ex) {
				// look in the superclass
			}
		}
		throw new NoSuchFieldException(step);
	}

	private static String shortValue(Object value) {
		if (value == null || !value.getClass().isArray()) {
			return String.valueOf(value);
		}
		int n = Array.getLength(value);
		StringBuilder sb = new StringBuilder("[");
		for (int i = 0; i < Math.min(n, 16); i++) {
			Object element = Array.get(value, i);
			sb.append(i == 0 ? "" : ",").append(element != null && element.getClass().isArray() ? "[.." + Array.getLength(element) + "]" : String.valueOf(element));
		}
		return sb.append(n > 16 ? ",..(" + n + ")]" : "]").toString();
	}

	private static String describe(Object value) throws Exception {
		if (value == null || value instanceof Number || value instanceof Boolean || value instanceof String || value.getClass().isArray()) {
			return shortValue(value);
		}
		StringBuilder sb = new StringBuilder(value.getClass().getSimpleName()).append('{');
		for (Class<?> type = value.getClass(); type != null && type != Object.class; type = type.getSuperclass()) {
			for (Field f : type.getDeclaredFields()) {
				if (java.lang.reflect.Modifier.isStatic(f.getModifiers())) {
					continue;
				}
				f.setAccessible(true);
				sb.append(f.getName()).append('=').append(shortValue(f.get(value))).append(' ');
			}
		}
		return sb.append('}').toString();
	}

	private static void assign(Field field, String value) throws Exception {
		Class<?> type = field.getType();
		if (type == int.class) {
			field.setInt(null, Integer.parseInt(value));
		} else if (type == long.class) {
			field.setLong(null, Long.parseLong(value));
		} else if (type == double.class) {
			field.setDouble(null, Double.parseDouble(value));
		} else if (type == float.class) {
			field.setFloat(null, Float.parseFloat(value));
		} else if (type == boolean.class) {
			field.setBoolean(null, Boolean.parseBoolean(value));
		} else {
			throw new IllegalArgumentException("unsupported field type " + type);
		}
	}
}
