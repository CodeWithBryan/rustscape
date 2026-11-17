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
 * </pre>
 */
public final class Driver implements Runnable {
	private final File dir;
	private final java.util.Map<Field, String> held = new java.util.concurrent.ConcurrentHashMap<>();

	private Driver(File dir) {
		this.dir = dir;
	}

	public static void main(String[] args) throws Exception {
		String dir = System.getProperty("refcompare.dir");
		if (dir != null) {
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
				for (String line : lines) {
					line = line.replace("﻿", "").trim();
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
				Files.write(out.toPath(), result.toString().getBytes(StandardCharsets.UTF_8), StandardOpenOption.CREATE, StandardOpenOption.APPEND);
				cmd.delete();
				done.createNewFile();
			} catch (Throwable ex) {
				ex.printStackTrace();
			}
		}
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
