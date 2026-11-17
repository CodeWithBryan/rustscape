import java.awt.Canvas;
import java.awt.Component;
import java.awt.event.FocusEvent;
import java.awt.event.InputEvent;
import java.awt.event.KeyEvent;
import java.awt.event.MouseEvent;
import java.io.PrintStream;
import java.util.Arrays;
import java.util.Random;

import rt4.Keyboard;
import rt4.MonotonicClock;
import rt4.Mouse;
import rt4.SignLink;

/**
 * Drives the unchanged rt4.Keyboard and rt4.Mouse listeners with real AWT
 * event objects on a headless Canvas and prints every command with the
 * complete static state after it.
 *
 * Arguments: seed, keyboard scenarios, mouse scenarios, keyboard output,
 * mouse output.
 *
 * Keyboard lines: "S vendor" (0 modern, 1 no setFocusTraversalKeysEnabled,
 * 2 Microsoft), then "M" with the 521 CODE_MAP entries, then one line per
 * command: command, argument, nextKey result (or -1), pressedKeys as 28 hex
 * digits (key 0 is the lowest bit of the last digit), the two event-ring
 * indices, the three typed-ring indices, idleLoops, keyCode, keyChar and a
 * hash of the three rings.
 * Commands: P keyPressed(vk), R keyReleased(vk), T keyTyped(char),
 * F focusLost, G focusGained, L loop, N nextKey, X stop, Q quit.
 *
 * Mouse lines: command, five arguments, then clickButton clickX clickY
 * clickTime pressedButton lastMouseX lastMouseY pendingClickButton
 * pendingClickX pendingClickY pendingClickTime pendingPressedButton
 * currentMouseX currentMouseY idleLoops.
 * Commands (x y button mask time): S reset, M mouseMoved, D mouseDragged,
 * P mousePressed, U mouseReleased, C mouseClicked, E mouseEntered,
 * X mouseExited, F focusLost, G focusGained, L loop, I setIdleLoops(x),
 * T stop, Q quit. mask is the buttons down in getModifiersEx: 1 left,
 * 2 middle, 4 right.
 */
public final class InputOracle {
	private static final Canvas CANVAS = new Canvas();
	private static int[] pristine;
	private static Keyboard keys;
	private static Mouse mouse;
	private static PrintStream out;

	public static void main(String[] args) throws Exception {
		long seed = Long.parseLong(args[0]);
		try (PrintStream stream = new PrintStream(args[3], "US-ASCII")) {
			out = stream;
			keyboard(new Random(seed), Integer.parseInt(args[1]));
		}
		try (PrintStream stream = new PrintStream(args[4], "US-ASCII")) {
			out = stream;
			mouse(new Random(seed ^ 0x5DEECE66DL), Integer.parseInt(args[2]));
		}
	}

	// ---- Keyboard ----

	/** Every static field back to its initializer; the listener is new. */
	private static void resetKeyboard(int vendor) throws Exception {
		if (pristine == null) {
			pristine = Keyboard.CODE_MAP.clone();
		}
		System.arraycopy(pristine, 0, Keyboard.CODE_MAP, 0, pristine.length);
		Arrays.fill(Keyboard.eventQueue, 0);
		Arrays.fill(Keyboard.typedCodeQueue, 0);
		Arrays.fill(Keyboard.typedCharQueue, 0);
		Arrays.fill(Keyboard.pressedKeys, false);
		Keyboard.eventQueueWriterIndex = 0;
		Keyboard.eventQueueReaderIndex = 0;
		Keyboard.typedQueueWriterIndex = 0;
		Keyboard.typedQueueReaderIndex = 0;
		Keyboard.prevTypedQueueWriterIndex = 0;
		Keyboard.idleLoops = 0;
		Keyboard.keyCode = 0;
		Keyboard.keyChar = 0;
		Keyboard.instance = new Keyboard();
		keys = Keyboard.instance;
		SignLink.javaVendor = vendor == 2 ? "Microsoft Corp." : "Oracle Corporation";
		SignLink.setFocusTraversalKeysEnabled = vendor == 0
			? Component.class.getMethod("setFocusTraversalKeysEnabled", Boolean.TYPE)
			: null;
		Keyboard.init();
		Keyboard.start(CANVAS);
	}

	private static void keyboardState(String command, int argument, int result) {
		StringBuilder line = new StringBuilder();
		line.append(command).append(' ').append(argument).append(' ').append(result).append(' ');
		for (int nibble = 27; nibble >= 0; nibble--) {
			int value = 0;
			for (int bit = 0; bit < 4; bit++) {
				if (Keyboard.pressedKeys[nibble * 4 + bit]) {
					value |= 1 << bit;
				}
			}
			line.append(Character.forDigit(value, 16));
		}
		int hash = 0;
		for (int[] queue : new int[][]{Keyboard.eventQueue, Keyboard.typedCodeQueue, Keyboard.typedCharQueue}) {
			for (int value : queue) {
				hash = hash * 31 + value;
			}
		}
		line.append(' ').append(Keyboard.eventQueueWriterIndex)
			.append(' ').append(Keyboard.eventQueueReaderIndex)
			.append(' ').append(Keyboard.typedQueueWriterIndex)
			.append(' ').append(Keyboard.typedQueueReaderIndex)
			.append(' ').append(Keyboard.prevTypedQueueWriterIndex)
			.append(' ').append(Keyboard.getIdleLoops())
			.append(' ').append(Keyboard.keyCode)
			.append(' ').append(Keyboard.keyChar)
			.append(' ').append(hash);
		out.print(line.append('\n'));
	}

	private static final int[] ODD_KEYS = {
		0, 3, 12, 19, 20, 144, 145, 154, 300, 519, 520, 521, 522, 524, 525, 600, 65535, 0xF000,
		0xF00B, Integer.MAX_VALUE, -1, -129, Integer.MIN_VALUE
	};
	private static final int[] ODD_CHARS = {
		0, 1, 8, 9, 10, 13, 27, 127, 128, 160, 255, 256, 257, 8364, 8365, 0xD800, 0xDFFF, 0xFFFE
	};
	private static final int[] MODIFIERS = {
		0, 0, 0, InputEvent.SHIFT_DOWN_MASK, InputEvent.CTRL_DOWN_MASK, InputEvent.ALT_DOWN_MASK,
		InputEvent.CTRL_DOWN_MASK | InputEvent.ALT_DOWN_MASK, InputEvent.ALT_GRAPH_DOWN_MASK
	};

	private static int key(Random random, int[] mapped) {
		int kind = random.nextInt(20);
		if (kind < 15) {
			return mapped[random.nextInt(mapped.length)];
		}
		if (kind < 17) {
			return random.nextInt(Keyboard.CODE_MAP.length);
		}
		return ODD_KEYS[random.nextInt(ODD_KEYS.length)];
	}

	private static void keyEvent(Random random, int[] mapped, int[] held, boolean focus) {
		int modifiers = MODIFIERS[random.nextInt(MODIFIERS.length)];
		int kind = random.nextInt(100);
		if (kind < 40) {
			// Half of the presses repeat a key that is already down, as OS auto-repeat does.
			int vk = random.nextBoolean() && held[1] != 0 ? held[0] : key(random, mapped);
			held[0] = vk;
			held[1] = 1;
			keys.keyPressed(new KeyEvent(CANVAS, KeyEvent.KEY_PRESSED, 0L, modifiers, vk, KeyEvent.CHAR_UNDEFINED));
			keyboardState("P", vk, -1);
		} else if (kind < 65) {
			int vk = random.nextBoolean() && held[1] != 0 ? held[0] : key(random, mapped);
			keys.keyReleased(new KeyEvent(CANVAS, KeyEvent.KEY_RELEASED, 0L, modifiers, vk, KeyEvent.CHAR_UNDEFINED));
			keyboardState("R", vk, -1);
		} else if (kind < 98 || !focus) {
			int c = random.nextInt(4) == 0 ? ODD_CHARS[random.nextInt(ODD_CHARS.length)] : 32 + random.nextInt(95);
			keys.keyTyped(new KeyEvent(CANVAS, KeyEvent.KEY_TYPED, 0L, modifiers, KeyEvent.VK_UNDEFINED, (char) c));
			keyboardState("T", c, -1);
		} else if (kind < 99) {
			keys.focusLost(new FocusEvent(CANVAS, FocusEvent.FOCUS_LOST));
			keyboardState("F", 0, -1);
		} else {
			keys.focusGained(new FocusEvent(CANVAS, FocusEvent.FOCUS_GAINED));
			keyboardState("G", 0, -1);
		}
	}

	private static void keyboard(Random random, int scenarios) throws Exception {
		for (int scenario = 0; scenario < scenarios; scenario++) {
			int vendor = scenario % 3;
			resetKeyboard(vendor);
			out.print("S " + vendor + "\n");
			StringBuilder map = new StringBuilder("M");
			int count = 0;
			for (int code : Keyboard.CODE_MAP) {
				map.append(' ').append(code);
				if (code != -1) {
					count++;
				}
			}
			out.print(map.append('\n'));
			int[] mapped = new int[count];
			for (int i = 0, n = 0; i < Keyboard.CODE_MAP.length; i++) {
				if (Keyboard.CODE_MAP[i] != -1) {
					mapped[n++] = i;
				}
			}
			int[] held = {0, 0};
			// 0 typing, 1 bursts that fill both rings (no focus changes, which would empty the event ring first), 2 mostly idle cycles.
			int profile = scenario / 3 % 3;
			int cycles = profile == 1 ? 10 : 160;
			for (int cycle = 0; cycle < cycles; cycle++) {
				int events;
				if (profile == 0) {
					events = random.nextInt(7);
				} else if (profile == 1) {
					events = random.nextInt(4) == 0 ? 100 + random.nextInt(300) : random.nextInt(140);
				} else {
					events = random.nextInt(6) == 0 ? 1 + random.nextInt(3) : 0;
				}
				for (int event = 0; event < events; event++) {
					keyEvent(random, mapped, held, profile != 1);
				}
				Keyboard.loop();
				keyboardState("L", 0, -1);
				// Read all, some or none of the cycle's typed entries.
				int reads = random.nextInt(4) == 0 ? random.nextInt(4) : 200;
				for (int read = 0; read < reads; read++) {
					boolean more = Keyboard.nextKey();
					keyboardState("N", 0, more ? 1 : 0);
					if (!more && random.nextInt(3) != 0) {
						break;
					}
				}
			}
			if (scenario % 2 == 1) {
				Keyboard.stop(CANVAS);
				keyboardState("X", 0, -1);
				for (int event = 0; event < 6; event++) {
					keyEvent(random, mapped, held, profile != 1);
				}
				Keyboard.loop();
				keyboardState("L", 0, -1);
			}
			// After quit the listener ignores events; loop and nextKey cannot run.
			Keyboard.quit();
			keyboardState("Q", 0, -1);
			for (int event = 0; event < 12; event++) {
				keyEvent(random, mapped, held, true);
			}
		}
	}

	// ---- Mouse ----

	private static void resetMouse() {
		Mouse.clickX = 0;
		Mouse.clickY = 0;
		Mouse.lastMouseY = 0;
		Mouse.lastMouseX = 0;
		Mouse.instance = new Mouse();
		mouse = Mouse.instance;
		Mouse.idleLoops = 0;
		Mouse.clickButton = 0;
		Mouse.pressedButton = 0;
		Mouse.pendingClickTime = 0L;
		Mouse.pendingClickX = 0;
		Mouse.pendingPressedButton = 0;
		Mouse.clickTime = 0L;
		Mouse.pendingClickButton = 0;
		Mouse.currentMouseY = -1;
		Mouse.currentMouseX = -1;
		Mouse.pendingClickY = 0;
		Mouse.lastHandledClickX = 0;
		Mouse.prevClickTime = 0L;
		Mouse.lastHandledClickY = 0;
		MonotonicClock.now = 0L;
		Mouse.start(CANVAS);
	}

	private static void mouseState(String command, int x, int y, int button, int mask, long time) {
		out.print(command + ' ' + x + ' ' + y + ' ' + button + ' ' + mask + ' ' + time
			+ ' ' + Mouse.clickButton + ' ' + Mouse.clickX + ' ' + Mouse.clickY + ' ' + Mouse.clickTime
			+ ' ' + Mouse.pressedButton + ' ' + Mouse.lastMouseX + ' ' + Mouse.lastMouseY
			+ ' ' + Mouse.pendingClickButton + ' ' + Mouse.pendingClickX + ' ' + Mouse.pendingClickY
			+ ' ' + Mouse.pendingClickTime + ' ' + Mouse.pendingPressedButton
			+ ' ' + Mouse.currentMouseX + ' ' + Mouse.currentMouseY + ' ' + Mouse.idleLoops + '\n');
	}

	private static int extended(int mask) {
		return ((mask & 1) != 0 ? InputEvent.BUTTON1_DOWN_MASK : 0)
			| ((mask & 2) != 0 ? InputEvent.BUTTON2_DOWN_MASK : 0)
			| ((mask & 4) != 0 ? InputEvent.BUTTON3_DOWN_MASK : 0);
	}

	private static MouseEvent event(int id, int mask, int x, int y, int button) {
		return new MouseEvent(CANVAS, id, 0L, extended(mask), x, y, 1, button == MouseEvent.BUTTON3, button);
	}

	private static int coordinate(Random random) {
		return random.nextInt(12) == 0 ? -random.nextInt(60) : random.nextInt(2000);
	}

	/** One listener event. down[0] is the buttons currently held, as a mask. */
	private static void mouseStep(Random random, int[] down) {
		int x = coordinate(random);
		int y = coordinate(random);
		int kind = random.nextInt(100);
		if (kind < 45) {
			if (down[0] == 0) {
				mouse.mouseMoved(event(MouseEvent.MOUSE_MOVED, 0, x, y, MouseEvent.NOBUTTON));
				mouseState("M", x, y, 0, 0, 0L);
			} else {
				mouse.mouseDragged(event(MouseEvent.MOUSE_DRAGGED, down[0], x, y, MouseEvent.NOBUTTON));
				mouseState("D", x, y, 0, down[0], 0L);
			}
		} else if (kind < 65) {
			int button = 1 + random.nextInt(3);
			int mask = down[0] | 1 << (button - 1);
			// One press in ten carries a mask AWT would not send with that button.
			if (random.nextInt(10) == 0) {
				mask = random.nextInt(8);
			} else {
				down[0] = mask;
			}
			MonotonicClock.now = random.nextInt(8) == 0 ? random.nextLong() : MonotonicClock.now + random.nextInt(5000);
			mouse.mousePressed(event(MouseEvent.MOUSE_PRESSED, mask, x, y, button));
			mouseState("P", x, y, button, mask, MonotonicClock.now);
		} else if (kind < 83) {
			int button = 1 + random.nextInt(3);
			down[0] &= ~(1 << (button - 1));
			mouse.mouseReleased(event(MouseEvent.MOUSE_RELEASED, down[0], x, y, button));
			mouseState("U", x, y, button, down[0], 0L);
		} else if (kind < 87) {
			int button = 1 + random.nextInt(3);
			mouse.mouseClicked(event(MouseEvent.MOUSE_CLICKED, down[0], x, y, button));
			mouseState("C", x, y, button, down[0], 0L);
		} else if (kind < 91) {
			mouse.mouseExited(event(MouseEvent.MOUSE_EXITED, down[0], x, y, MouseEvent.NOBUTTON));
			mouseState("X", x, y, 0, down[0], 0L);
		} else if (kind < 95) {
			mouse.mouseEntered(event(MouseEvent.MOUSE_ENTERED, down[0], x, y, MouseEvent.NOBUTTON));
			mouseState("E", x, y, 0, down[0], 0L);
		} else if (kind < 98) {
			mouse.focusLost(new FocusEvent(CANVAS, FocusEvent.FOCUS_LOST));
			mouseState("F", 0, 0, 0, 0, 0L);
		} else {
			mouse.focusGained(new FocusEvent(CANVAS, FocusEvent.FOCUS_GAINED));
			mouseState("G", 0, 0, 0, 0, 0L);
		}
	}

	private static void mouse(Random random, int scenarios) {
		for (int scenario = 0; scenario < scenarios; scenario++) {
			resetMouse();
			mouseState("S", 0, 0, 0, 0, 0L);
			int[] down = {0};
			for (int cycle = 0; cycle < 150; cycle++) {
				int events = scenario % 2 == 0 ? random.nextInt(4) : random.nextInt(12) == 0 ? random.nextInt(30) : 0;
				for (int event = 0; event < events; event++) {
					mouseStep(random, down);
				}
				if (random.nextInt(40) == 0) {
					int value = random.nextInt(3) == 0 ? 14500 : random.nextInt(3) == 0 ? Integer.MAX_VALUE : 0;
					Mouse.setIdleLoops(value);
					mouseState("I", value, 0, 0, 0, 0L);
				}
				Mouse.loop();
				mouseState("L", 0, 0, 0, 0, 0L);
			}
			if (scenario % 2 == 1) {
				Mouse.stop(CANVAS);
				mouseState("T", 0, 0, 0, 0, 0L);
				for (int event = 0; event < 6; event++) {
					mouseStep(random, down);
				}
				Mouse.loop();
				mouseState("L", 0, 0, 0, 0, 0L);
			}
			// After quit the listener ignores events; loop and setIdleLoops cannot run.
			Mouse.quit();
			mouseState("Q", 0, 0, 0, 0, 0L);
			for (int event = 0; event < 20; event++) {
				mouseStep(random, down);
			}
		}
	}
}
