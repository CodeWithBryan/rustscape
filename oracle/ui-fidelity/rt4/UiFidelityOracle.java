package rt4;

import java.io.PrintStream;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

/**
 * Observations of the unchanged InterfaceList, Inv, Font, SoftwareFont, SoftwareRaster and
 * JagString over seeded inputs, one JSON object per line on standard output.
 *
 * <p>Arguments: seed, walk scenarios, kerned fonts, count-text cases.
 */
public final class UiFidelityOracle {
	private static final StringBuilder out = new StringBuilder();
	private static PrintStream stream;

	public static void main(String[] args) throws Exception {
		long seed = Long.parseLong(args[0]);
		int walks = Integer.parseInt(args[1]);
		int fonts = Integer.parseInt(args[2]);
		int texts = Integer.parseInt(args[3]);
		stream = new PrintStream(System.out, false, "UTF-8");
		numbers(new Random(seed));
		kerning(new Random(seed + 1), fonts);
		countText(new Random(seed + 2), texts);
		for (int i = 0; i < walks; i++) {
			walk(new Random(seed * 31 + 1000 + i), i);
		}
		stream.flush();
	}

	private static void line() {
		stream.print(out);
		stream.print('\n');
		out.setLength(0);
	}

	// ---- JagString.parseHexString and parseInt -----------------------------

	private static void numbers(Random random) {
		List<String> inputs = new ArrayList<>();
		String[] fixed = {"", "0", "ff", "FF", "ffffff", "ff9040", "7fffffff", "80000000", "ffffffff", "+1f", "-1", "-", "+",
			"-80000000", "-80000001", "g", "1g", "12 ", " 12", "1-2", "0x10", "00ff00", "z", "2147483647", "2147483648",
			"-2147483648", "-2147483649", "128", "0128", "256", "+256", "1e3", "A", "a", "10000000000"};
		for (String s : fixed) {
			inputs.add(s);
		}
		String alphabet = "0123456789abcdefABCDEFgGzZ+- x";
		for (int i = 0; i < 400; i++) {
			int length = random.nextInt(11);
			StringBuilder s = new StringBuilder();
			for (int j = 0; j < length; j++) {
				// Mostly digits, so that many inputs parse.
				s.append(alphabet.charAt(random.nextInt(random.nextInt(4) == 0 ? alphabet.length() : 22)));
			}
			inputs.add(s.toString());
		}
		for (String input : inputs) {
			out.append("{\"case\":\"number\",\"text\":\"").append(input).append("\"");
			for (int radix : new int[]{16, 10}) {
				out.append(radix == 16 ? ",\"hex\":" : ",\"decimal\":");
				try {
					// Not JagString.parse: that treats ( ) * + , - as escape prefixes.
					JagString text = JagString.allocate(input.length());
					for (int i = 0; i < input.length(); i++) {
						text.append(input.charAt(i));
					}
					out.append(radix == 16 ? text.parseHexString(16) : text.parseInt());
				} catch (NumberFormatException e) {
					out.append("null");
				}
			}
			out.append('}');
			line();
		}
	}

	// ---- Font.decode with kerning tables ------------------------------------

	private static void kerning(Random random, int fonts) {
		for (int f = 0; f < fonts; f++) {
			int[] advances = new int[256], heights = new int[256], tops = new int[256];
			int total = 768;
			for (int i = 0; i < 256; i++) {
				advances[i] = random.nextInt(14);
				heights[i] = random.nextInt(7);
				tops[i] = random.nextInt(9);
				total += heights[i] * 2;
			}
			byte[] data = new byte[total];
			int pos = 0;
			for (int i = 0; i < 256; i++) {
				data[pos++] = (byte) advances[i];
			}
			for (int i = 0; i < 256; i++) {
				data[pos++] = (byte) heights[i];
			}
			for (int i = 0; i < 256; i++) {
				data[pos++] = (byte) tops[i];
			}
			while (pos < total) {
				// Small steps with the odd large one, so running sums wrap a byte now and then.
				data[pos++] = (byte) (random.nextInt(12) == 0 ? random.nextInt(256) : random.nextInt(7) - 3);
			}
			Font font = new SoftwareFont(data);
			out.append("{\"case\":\"kerning\",\"data\":\"").append(hex(data)).append("\",\"lineHeight\":").append(font.lineHeight);
			out.append(",\"widths\":[");
			for (int i = 0; i < 600; i++) {
				int length = 1 + random.nextInt(4);
				byte[] chars = new byte[length];
				for (int j = 0; j < length; j++) {
					int ch;
					do {
						ch = 1 + random.nextInt(255);
					} while (ch == 60 || ch == 62);
					chars[j] = (byte) ch;
				}
				JagString text = JagString.allocate(length);
				for (byte ch : chars) {
					text.append(ch & 0xFF);
				}
				if (i > 0) {
					out.append(',');
				}
				out.append("[\"").append(hex(chars)).append("\",").append(font.getStringWidth(text)).append(']');
			}
			out.append("]}");
			line();
		}
	}

	// ---- Inv.renderObjectSprite's count text --------------------------------

	static int[] picture(int a, int b, int c) {
		int[] pixels = new int[36 * 32];
		for (int i = 0; i < pixels.length; i++) {
			pixels[i] = (i * 7 + c) % 5 == 0 ? 0 : (i * a + b) & 0xFFFFFF;
		}
		return pixels;
	}

	private static void countText(Random random, int cases) throws Exception {
		int[] stacks = {-1, 0, 1, 2, 5, 10, 99, 4096, 99999, 100000, 123456, 9999999, 10000000, 87654321, 2147483647};
		SoftwareFont font = null;
		int fontId = -1;
		for (int i = 0; i < cases; i++) {
			if (i % 40 == 0) {
				fontId++;
				byte[] data = new byte[257];
				int[] xOffsets = new int[256], yOffsets = new int[256], widths = new int[256], heights = new int[256];
				byte[][] pixels = new byte[256][];
				int lineHeight = 8 + random.nextInt(6);
				data[256] = (byte) lineHeight;
				out.append("{\"case\":\"font\",\"id\":").append(fontId).append(",\"glyphs\":[");
				for (int g = 0; g < 256; g++) {
					data[g] = (byte) (random.nextInt(10));
					xOffsets[g] = random.nextInt(3);
					yOffsets[g] = random.nextInt(lineHeight) - (random.nextInt(8) == 0 ? 3 : 0);
					widths[g] = random.nextInt(9);
					heights[g] = random.nextInt(12);
					pixels[g] = new byte[widths[g] * heights[g]];
					for (int p = 0; p < pixels[g].length; p++) {
						pixels[g][p] = (byte) (random.nextInt(3) == 0 ? 0 : 1 + random.nextInt(200));
					}
					if (g > 0) {
						out.append(',');
					}
					out.append('[').append(xOffsets[g]).append(',').append(yOffsets[g]).append(',').append(widths[g]).append(',')
						.append(heights[g]).append(",\"").append(hex(pixels[g])).append("\"]");
				}
				out.append("],\"data\":\"").append(hex(data)).append("\"}");
				line();
				font = new SoftwareFont(data, xOffsets, yOffsets, widths, heights, pixels);
				ObjTypeList.font = font;
			}
			int stack = i < stacks.length * 2 ? stacks[i % stacks.length] : random.nextInt(3) == 0 ? random.nextInt() : stacks[random.nextInt(stacks.length)] + random.nextInt(3) - 1;
			ObjType type = new ObjType();
			type.stackable = i < stacks.length ? 0 : random.nextInt(2);
			boolean drawText = random.nextInt(6) != 0;
			int a = 1 + random.nextInt(100000), b = random.nextInt(1 << 24), c = random.nextInt(5);
			SoftwareModel.picture = picture(a, b, c);
			ObjTypeList.staged.put(i, type);
			// Something to restore, as the original saves and restores the active raster.
			SoftwareRaster.setSize(new int[4], 2, 2);
			Sprite sprite = Inv.renderObjectSprite(0, false, i, drawText, 0, stack, false);
			int[] pixels = ((SoftwareSprite) sprite).pixels;
			MessageDigest digest = MessageDigest.getInstance("SHA-256");
			int lit = 0;
			for (int pixel : pixels) {
				digest.update(new byte[]{(byte) (pixel >> 24), (byte) (pixel >> 16), (byte) (pixel >> 8), (byte) pixel});
				if (pixel != 0) {
					lit++;
				}
			}
			out.append("{\"case\":\"countText\",\"font\":").append(fontId).append(",\"stack\":").append(stack).append(",\"stackable\":")
				.append(type.stackable).append(",\"drawText\":").append(drawText).append(",\"picture\":[").append(a).append(',').append(b)
				.append(',').append(c).append("],\"text\":\"").append(text(Inv.formatObjAmount(stack))).append("\",\"lit\":").append(lit)
				.append(",\"sha256\":\"").append(hex(digest.digest())).append("\"}");
			line();
		}
	}

	private static String text(JagString value) {
		StringBuilder s = new StringBuilder();
		for (int i = 0; i < value.length(); i++) {
			s.append((char) (value.charAt(i) & 0xFF));
		}
		return s.toString();
	}

	private static String hex(byte[] bytes) {
		StringBuilder s = new StringBuilder();
		for (byte b : bytes) {
			s.append(Character.forDigit((b >> 4) & 15, 16)).append(Character.forDigit(b & 15, 16));
		}
		return s.toString();
	}

	// ---- InterfaceList.processComponents and the layout ----------------------

	private static final String[] HOOKS = {"onScroll", "onClickRepeat", "onDrag", "onRelease", "onHold", "onMouseOver", "onMouseRepeat",
		"onMouseLeave", "onTimer", "onVarcTransmit", "onVarcstrTransmit", "onVarpTransmit", "onInvTransmit", "onStatTransmit", "onMsg",
		"onFriendTransmit", "onClanTransmit", "onStockTransmit", "onMiscTransmit", "onKey", "onMinimapUnlock", "onResize"};

	private static Object[] get(Component c, int hook) {
		switch (hook) {
			case 0: return c.onScroll;
			case 1: return c.onClickRepeat;
			case 2: return c.onDrag;
			case 3: return c.onRelease;
			case 4: return c.onHold;
			case 5: return c.onMouseOver;
			case 6: return c.onMouseRepeat;
			case 7: return c.onMouseLeave;
			case 8: return c.onTimer;
			case 9: return c.onVarcTransmit;
			case 10: return c.onVarcstrTransmit;
			case 11: return c.onVarpTransmit;
			case 12: return c.onInvTransmit;
			case 13: return c.onStatTransmit;
			case 14: return c.onMsg;
			case 15: return c.onFriendTransmit;
			case 16: return c.onClanTransmit;
			case 17: return c.onStockTransmit;
			case 18: return c.onMiscTransmit;
			case 19: return c.onKey;
			case 20: return c.onMinimapUnlock;
			default: return c.onResize;
		}
	}

	private static void set(Component c, int hook) {
		Object[] value = {hook + 1};
		switch (hook) {
			case 0: c.onScroll = value; break;
			case 1: c.onClickRepeat = value; break;
			case 2: c.onDrag = value; break;
			case 3: c.onRelease = value; break;
			case 4: c.onHold = value; break;
			case 5: c.onMouseOver = value; break;
			case 6: c.onMouseRepeat = value; break;
			case 7: c.onMouseLeave = value; break;
			case 8: c.onTimer = value; break;
			case 9: c.onVarcTransmit = value; break;
			case 10: c.onVarcstrTransmit = value; break;
			case 11: c.onVarpTransmit = value; break;
			case 12: c.onInvTransmit = value; break;
			case 13: c.onStatTransmit = value; break;
			case 14: c.onMsg = value; break;
			case 15: c.onFriendTransmit = value; break;
			case 16: c.onClanTransmit = value; break;
			case 17: c.onStockTransmit = value; break;
			case 18: c.onMiscTransmit = value; break;
			case 19: c.onKey = value; break;
			case 20: c.onMinimapUnlock = value; break;
			default: c.onResize = value; break;
		}
	}

	private static int[] triggers(Random random) {
		if (random.nextBoolean()) {
			return null;
		}
		int[] ids = new int[1 + random.nextInt(3)];
		for (int i = 0; i < ids.length; i++) {
			ids[i] = random.nextInt(6);
		}
		return ids;
	}

	private static void list(String name, int[] values) {
		out.append(",\"").append(name).append("\":");
		if (values == null) {
			out.append("null");
			return;
		}
		out.append('[');
		for (int i = 0; i < values.length; i++) {
			if (i > 0) {
				out.append(',');
			}
			out.append(values[i]);
		}
		out.append(']');
	}

	/**
	 * Whether this scenario has draggable components. A drag begun part-way through the walk
	 * switches the IF1 tooltip and scrollbar handling off for the rest of it; scenarios with
	 * drags therefore stage no IF1 tooltips or scroll layers, and the others no drags.
	 */
	private static boolean drags;

	/** Every staged component in a fixed order: per group, its static array, then each created array. */
	private static final List<Component> all = new ArrayList<>();

	private static int ordinal(Component component) {
		for (int i = 0; i < all.size(); i++) {
			if (all.get(i) == component) {
				return i;
			}
		}
		return component == null ? -1 : -2;
	}

	private static void requests(String name, LinkedList queue) {
		out.append(",\"").append(name).append("\":[");
		boolean first = true;
		for (HookRequest request = (HookRequest) queue.removeHead(); request != null; request = (HookRequest) queue.removeHead()) {
			if (!first) {
				out.append(',');
			}
			first = false;
			out.append('[').append(ordinal(request.source)).append(',').append(request.arguments[0]).append(',').append(request.mouseX)
				.append(',').append(request.mouseY).append(',').append(request.keyCode).append(',').append(request.keyChar).append(']');
		}
		out.append(']');
	}

	private static void ring(Random random, String name, int[] ring, int[] writer) {
		int count = random.nextInt(8) == 0 ? 33 + random.nextInt(4) : random.nextInt(3) == 0 ? 1 + random.nextInt(3) : 0;
		int[] ids = new int[count];
		for (int i = 0; i < count; i++) {
			ids[i] = random.nextInt(6);
			ring[writer[0]++ & 31] = ids[i];
		}
		list(name, ids);
	}

	private static void walk(Random random, int number) {
		// Fresh statics: the original keeps all of this in class fields.
		InterfaceList.components = new Component[200][];
		InterfaceList.interfaceLoaded = new boolean[200];
		InterfaceList.openInterfaces = new HashTable(8);
		InterfaceList.properties.clear();
		InterfaceList.highPriorityRequests.clear();
		InterfaceList.mediumPriorityRequests.clear();
		InterfaceList.lowPriorityRequests.clear();
		InterfaceList.scrollbarDragging = false;
		InterfaceList.scrollbarDragMargin = 0;
		InterfaceList.hoveredComponent = null;
		InterfaceList.clickedInventoryComponent = null;
		InterfaceList.worldMapDragState = 0;
		InterfaceList.topLevelInterface = 100;
		InterfaceList.transmitTimer = 1 + random.nextInt(5);
		InterfaceList.miscTransmitAt = random.nextInt(3);
		Chat.transmitAt = random.nextInt(3);
		FriendsList.transmitAt = random.nextInt(3);
		ClanChat.transmitAt = random.nextInt(3);
		StockMarketManager.transmitAt = random.nextInt(3);
		int[] varc = {random.nextInt(6)}, varcstr = {random.nextInt(6)}, varp = {random.nextInt(6)}, inv = {random.nextInt(6)},
			stat = {random.nextInt(6)};
		for (int i = 0; i < 32; i++) {
			VarcDomain.updatedVarcs[i] = random.nextInt(6);
			VarcDomain.updatedVarcstrs[i] = random.nextInt(6);
			VarpDomain.updatedVarps[i] = random.nextInt(6);
			Inv.updatedInventories[i] = random.nextInt(6);
			PlayerSkillXpTable.updatedStats[i] = random.nextInt(6);
		}
		Cs1ScriptRunner.isMenuOpen = false;
		Cs1ScriptRunner.draggedComponent = null;
		Cs1ScriptRunner.dragParentComponent = null;
		drags = random.nextInt(2) == 0;
		all.clear();
		int[][] canvases = {{765, 503}, {1024, 768}, {400, 300}, {640, 480}};
		int[] canvas = canvases[random.nextInt(canvases.length)];
		GameShell.canvasWidth = canvas[0];
		GameShell.canvasHeight = canvas[1];
		int groups = 1 + random.nextInt(3);
		out.append("{\"case\":\"walk\",\"number\":").append(number).append(",\"canvas\":[").append(canvas[0]).append(',').append(canvas[1])
			.append("],\"timer\":").append(InterfaceList.transmitTimer);
		list("stamps", new int[]{Chat.transmitAt, FriendsList.transmitAt, ClanChat.transmitAt, StockMarketManager.transmitAt,
			InterfaceList.miscTransmitAt});
		list("writers", new int[]{varc[0], varcstr[0], varp[0], inv[0], stat[0]});
		list("varcRing", VarcDomain.updatedVarcs);
		list("varcstrRing", VarcDomain.updatedVarcstrs);
		list("varpRing", VarpDomain.updatedVarps);
		list("invRing", Inv.updatedInventories);
		list("statRing", PlayerSkillXpTable.updatedStats);
		out.append(",\"components\":[");
		List<Component> layers = new ArrayList<>();
		for (int g = 0; g < groups; g++) {
			int group = 100 + g;
			int count = 3 + random.nextInt(8);
			Component[] statics = new Component[count];
			InterfaceList.components[group] = statics;
			InterfaceList.interfaceLoaded[group] = true;
			List<Component> created = new ArrayList<>();
			for (int i = 0; i < count; i++) {
				if (i > 0 && random.nextInt(14) == 0) {
					// The original's arrays can have holes.
					continue;
				}
				Component c = statics[i] = new Component();
				c.id = (group << 16) + i;
				c.if3 = random.nextInt(7) != 0;
				int[] kinds = c.if3 ? new int[]{0, 0, 0, 3, 4, 5, 5, 6, 9, 2} : drags ? new int[]{0, 0, 3, 4, 5, 2} : new int[]{0, 0, 3, 4, 5, 8, 2};
				c.type = i == 0 ? 0 : kinds[random.nextInt(kinds.length)];
				List<Integer> parents = new ArrayList<>();
				for (int p = 0; p < i; p++) {
					if (statics[p] != null && statics[p].type == 0) {
						parents.add(p);
					}
				}
				if (i > 0 && !parents.isEmpty() && random.nextInt(5) != 0) {
					c.overlayer = (group << 16) + parents.get(random.nextInt(parents.size()));
				}
				stage(random, c, canvas, count);
				all.add(c);
				if (c.type == 0) {
					layers.add(c);
				}
				if (c.type == 0 && c.if3 && random.nextInt(4) == 0) {
					c.createdComponents = new Component[1 + random.nextInt(4)];
					for (int slot = 0; slot < c.createdComponents.length; slot++) {
						if (random.nextInt(5) == 0) {
							continue;
						}
						Component child = c.createdComponents[slot] = new Component();
						child.id = c.id;
						child.createdComponentId = slot;
						child.overlayer = c.id;
						child.if3 = true;
						child.type = new int[]{3, 4, 5, 6, 9}[random.nextInt(5)];
						stage(random, child, canvas, 0);
						created.add(child);
					}
				}
			}
			all.addAll(created);
		}
		// Drag parents and server properties refer to staged components.
		for (Component c : all) {
			if (drags && c.if3 && random.nextInt(2) == 0 && !layers.isEmpty()) {
				c.dragParent = layers.get(random.nextInt(layers.size()));
			}
		}
		for (int i = 0; i < all.size(); i++) {
			Component c = all.get(i);
			if (i > 0) {
				out.append(',');
			}
			describe(c);
		}
		out.append("],\"open\":[");
		// Interfaces 101 and 102 open inside layers of earlier groups.
		boolean firstOpen = true;
		for (int g = 1; g < groups; g++) {
			List<Component> hosts = new ArrayList<>();
			for (Component c : layers) {
				if (c.id >> 16 < 100 + g) {
					hosts.add(c);
				}
			}
			if (hosts.isEmpty() || random.nextInt(6) == 0) {
				continue;
			}
			Component host = hosts.get(random.nextInt(hosts.size()));
			if (InterfaceList.openInterfaces.get(host.id) != null) {
				continue;
			}
			ComponentPointer pointer = new ComponentPointer();
			pointer.interfaceId = 100 + g;
			pointer.type = random.nextInt(2);
			InterfaceList.openInterfaces.put(pointer, host.id);
			if (!firstOpen) {
				out.append(',');
			}
			firstOpen = false;
			out.append('[').append(host.id).append(',').append(pointer.interfaceId).append(',').append(pointer.type).append(']');
		}
		out.append("],\"cycles\":[");
		// The interface is opened the way Protocol does it: an unforced layout.
		InterfaceList.layoutTopLevel(false);
		int cycles = 3 + random.nextInt(4);
		int[] point = {random.nextInt(canvas[0]), random.nextInt(canvas[1])};
		for (int cycle = 0; cycle < cycles; cycle++) {
			if (cycle > 0) {
				out.append(',');
			}
			out.append('{');
			// A change between cycles, as a script or the server would make.
			Component changed = all.get(random.nextInt(all.size()));
			int change = random.nextInt(8);
			if (change == 0) {
				changed.hidden = !changed.hidden;
			} else if (change == 1) {
				changed.scrollY = random.nextInt(60);
			} else if (change == 2) {
				changed.baseX += random.nextInt(21) - 10;
			} else {
				change = -1;
			}
			out.append("\"change\":[").append(change < 0 ? -1 : ordinal(changed)).append(',').append(change).append(',')
				.append(change == 0 ? (changed.hidden ? 1 : 0) : change == 1 ? changed.scrollY : changed.baseX).append(']');
			boolean resized = random.nextInt(6) == 0;
			if (resized) {
				int[] next = canvases[random.nextInt(canvases.length)];
				GameShell.canvasWidth = next[0] + random.nextInt(3) * 16;
				GameShell.canvasHeight = next[1] + random.nextInt(3) * 16;
			}
			out.append(",\"canvas\":[").append(GameShell.canvasWidth).append(',').append(GameShell.canvasHeight).append(']');
			// GameShell and DisplayMode force the layout when the canvas changes size.
			boolean forced = resized && (GameShell.canvasWidth != canvas[0] || GameShell.canvasHeight != canvas[1]);
			canvas = new int[]{GameShell.canvasWidth, GameShell.canvasHeight};
			InterfaceList.layoutTopLevel(forced);
			out.append(",\"forced\":").append(forced);
			// Mouse: stay, jump onto a component, or go anywhere (also off the canvas).
			int move = random.nextInt(10);
			if (move < 5) {
				int[] rectangle = absolute(all.get(random.nextInt(all.size())));
				if (rectangle != null) {
					point = new int[]{rectangle[0] + random.nextInt(Math.max(1, rectangle[2] + 2)) - 1,
						rectangle[1] + random.nextInt(Math.max(1, rectangle[3] + 2)) - 1};
				}
			} else if (move < 8) {
				point = new int[]{random.nextInt(canvas[0] + 20) - 10, random.nextInt(canvas[1] + 20) - 10};
			}
			Mouse.lastMouseX = point[0];
			Mouse.lastMouseY = point[1];
			Mouse.pressedButton = new int[]{0, 0, 1, 1, 1, 2}[random.nextInt(6)];
			Mouse.clickButton = new int[]{0, 0, 0, 1, 1, 2}[random.nextInt(6)];
			boolean inventoryHeld = random.nextInt(16) == 0;
			if (inventoryHeld) {
				// The mouse is still held on an inventory item: there is no new click.
				Mouse.clickButton = 0;
			}
			Mouse.clickX = point[0];
			Mouse.clickY = point[1];
			MouseWheel.wheelRotation = new int[]{0, 0, 0, 0, -1, 1, 3}[random.nextInt(7)];
			Cs1ScriptRunner.isMenuOpen = random.nextInt(10) == 0;
			InterfaceList.clickedInventoryComponent = inventoryHeld ? new Component() : null;
			Camera.splineJustFinished = random.nextInt(8) == 0;
			list("mouse", new int[]{point[0], point[1], Mouse.pressedButton, Mouse.clickButton, MouseWheel.wheelRotation,
				Cs1ScriptRunner.isMenuOpen ? 1 : 0, InterfaceList.clickedInventoryComponent != null ? 1 : 0,
				Camera.splineJustFinished ? 1 : 0});
			InterfaceList.keyQueueSize = random.nextInt(4) == 0 ? 1 + random.nextInt(2) : 0;
			out.append(",\"keys\":[");
			for (int k = 0; k < InterfaceList.keyQueueSize; k++) {
				InterfaceList.keyCodes[k] = random.nextInt(3) == 0 ? -1 : random.nextInt(110);
				InterfaceList.keyChars[k] = InterfaceList.keyCodes[k] == -1 ? 32 + random.nextInt(90) : -1;
				out.append(k > 0 ? "," : "").append('[').append(InterfaceList.keyCodes[k]).append(',').append(InterfaceList.keyChars[k])
					.append(']');
			}
			out.append(']');
			ring(random, "varc", VarcDomain.updatedVarcs, varc);
			ring(random, "varcstr", VarcDomain.updatedVarcstrs, varcstr);
			ring(random, "varp", VarpDomain.updatedVarps, varp);
			ring(random, "inv", Inv.updatedInventories, inv);
			ring(random, "stat", PlayerSkillXpTable.updatedStats, stat);
			VarcDomain.updatedVarcsWriterIndex = varc[0];
			VarcDomain.updatedVarcstrsWriterIndex = varcstr[0];
			VarpDomain.updatedVarpsWriterIndex = varp[0];
			Inv.updatedInventoriesWriterIndex = inv[0];
			PlayerSkillXpTable.updatedStatsWriterIndex = stat[0];
			int[] stamped = new int[5];
			for (int s = 0; s < 5; s++) {
				stamped[s] = random.nextInt(5) == 0 ? 1 : 0;
			}
			if (stamped[0] == 1) {
				Chat.transmitAt = InterfaceList.transmitTimer;
			}
			if (stamped[1] == 1) {
				FriendsList.transmitAt = InterfaceList.transmitTimer;
			}
			if (stamped[2] == 1) {
				ClanChat.transmitAt = InterfaceList.transmitTimer;
			}
			if (stamped[3] == 1) {
				StockMarketManager.transmitAt = InterfaceList.transmitTimer;
			}
			if (stamped[4] == 1) {
				InterfaceList.miscTransmitAt = InterfaceList.transmitTimer;
			}
			list("stamped", stamped);
			// Protocol.loop 2780-2796 around the walk; no drag carries over from the last cycle.
			InterfaceList.hoveredComponent = null;
			Protocol.tooltipComponent = null;
			Cs1ScriptRunner.draggedComponent = null;
			Cs1ScriptRunner.dragParentComponent = null;
			InterfaceList.processSubInterface(0, 0, 0, GameShell.canvasWidth, InterfaceList.topLevelInterface, 0, GameShell.canvasHeight);
			InterfaceList.transmitTimer++;
			requests("high", InterfaceList.highPriorityRequests);
			requests("medium", InterfaceList.mediumPriorityRequests);
			requests("low", InterfaceList.lowPriorityRequests);
			out.append(",\"hover\":").append(ordinal(InterfaceList.hoveredComponent)).append(",\"tooltip\":")
				.append(ordinal(Protocol.tooltipComponent)).append(",\"dragged\":").append(ordinal(Cs1ScriptRunner.draggedComponent));
			out.append(",\"state\":[");
			for (int i = 0; i < all.size(); i++) {
				Component c = all.get(i);
				out.append(i > 0 ? "," : "").append('[').append((c.mouseOver ? 1 : 0) | (c.dragging ? 2 : 0)).append(',').append(c.x)
					.append(',').append(c.y).append(',').append(c.width).append(',').append(c.height).append(',').append(c.scrollX)
					.append(',').append(c.scrollY).append(',').append(c.updatedVarcsReaderIndex).append(',')
					.append(c.updatedVarcstrsReaderIndex).append(',').append(c.updatedVarpsReaderIndex).append(',')
					.append(c.updatedInventoriesReaderIndex).append(',').append(c.updatedStatsReaderIndex).append(',')
					.append(c.lastTransmitTimer).append(']');
			}
			out.append("]}");
		}
		out.append("]}");
		line();
	}

	/** Seeded fields of one component. Everything written here is echoed by describe(). */
	private static void stage(Random random, Component c, int[] canvas, int groupSize) {
		boolean layer = c.type == 0;
		if (c.if3) {
			c.dynamicWidthValue = (byte) new int[]{0, 0, 0, 1, 2, 4}[random.nextInt(6)];
			c.dynamicHeightValue = (byte) new int[]{0, 0, 0, 1, 2, 4}[random.nextInt(6)];
			if (c.dynamicWidthValue == 4 && c.dynamicHeightValue == 4) {
				c.dynamicHeightValue = 0;
			}
			c.xMode = (byte) (random.nextInt(3) == 0 ? random.nextInt(6) : 0);
			c.yMode = (byte) (random.nextInt(3) == 0 ? random.nextInt(6) : 0);
		}
		if (c.type == 2) {
			c.dynamicWidthValue = 3;
			c.dynamicHeightValue = 3;
			c.baseWidth = 1 + random.nextInt(4);
			c.baseHeight = 1 + random.nextInt(4);
			c.invMarginX = random.nextInt(8);
			c.invMarginY = random.nextInt(8);
		} else {
			c.baseWidth = size(random, c.dynamicWidthValue, canvas[0], layer);
			c.baseHeight = size(random, c.dynamicHeightValue, canvas[1], layer);
		}
		c.aspectWidth = 1 + random.nextInt(4);
		c.aspectHeight = 1 + random.nextInt(4);
		c.baseX = c.yMode >= 3 ? random.nextInt(9000) : random.nextInt(layer ? 120 : 220) - 15;
		c.baseY = c.xMode >= 3 ? random.nextInt(9000) : random.nextInt(layer ? 100 : 180) - 15;
		c.hidden = random.nextInt(9) == 0;
		if (layer) {
			c.noClickThrough = c.if3 && random.nextInt(5) == 0;
			if (random.nextInt(4) == 0 && (c.if3 || !drags)) {
				c.scrollMaxV = (c.if3 ? 0 : c.baseHeight) + 40 + random.nextInt(300);
				c.scrollY = random.nextInt(120);
			}
			if (c.if3 && random.nextInt(5) == 0) {
				c.scrollMaxH = 40 + random.nextInt(500);
				c.scrollX = random.nextInt(120);
			}
		}
		if (c.if3) {
			if (random.nextInt(9) == 0) {
				c.clientCode = new int[]{1337, 1338, 1401, 1402, 1405, 328}[random.nextInt(6)];
			}
			if (random.nextInt(4) != 0) {
				for (int hook = 0; hook < HOOKS.length; hook++) {
					if (random.nextInt(3) == 0) {
						set(c, hook);
						c.hasEventHandlers = true;
					}
				}
			}
			if (random.nextInt(12) == 0) {
				c.hasEventHandlers = !c.hasEventHandlers && get(c, 8) == null;
			}
			c.varcTriggers = triggers(random);
			c.varcstrTriggers = triggers(random);
			c.varpTriggers = triggers(random);
			c.inventoryTriggers = triggers(random);
			c.statTriggers = triggers(random);
		} else {
			// Hooks on an IF1 component, which the original never looks at.
			if (random.nextInt(6) == 0) {
				set(c, 8);
				set(c, 5);
				c.hasEventHandlers = true;
			}
			if (groupSize > 0 && random.nextInt(3) == 0) {
				c.hoverOverlayer = random.nextInt(groupSize);
			}
			c.overColor = random.nextInt(3) == 0 ? 1 + random.nextInt(0xFFFFFF) : 0;
		}
		c.updatedVarcsReaderIndex = random.nextInt(4);
		c.updatedVarcstrsReaderIndex = random.nextInt(4);
		c.updatedVarpsReaderIndex = random.nextInt(4);
		c.updatedInventoriesReaderIndex = random.nextInt(4);
		c.updatedStatsReaderIndex = random.nextInt(4);
		c.lastTransmitTimer = random.nextInt(4) - 1;
		c.mouseOver = random.nextInt(12) == 0;
		c.dragging = random.nextInt(12) == 0;
		// Bits 18 to 20 are the drag depth, which also makes a component draggable.
		int events = random.nextInt(3) == 0 ? 1 << random.nextInt(drags ? 22 : 18) : 0;
		if (random.nextInt(3) == 0 && c.createdComponentId < 0) {
			// The server's own table takes precedence over the component's field.
			InterfaceList.properties.put(new ServerActiveProperties(events, -1), ((long) c.id << 32) + (long) c.createdComponentId);
			c.properties = new ServerActiveProperties(random.nextInt(2), -1);
		} else {
			c.properties = new ServerActiveProperties(events, -1);
		}
	}

	private static int size(Random random, int mode, int parent, boolean layer) {
		if (mode == 1) {
			return random.nextInt(120);
		}
		if (mode == 2) {
			return random.nextInt(16384);
		}
		// IF1 scroll layers stay tall enough for the scrollbar arithmetic to be defined.
		return layer ? 60 + random.nextInt(Math.max(1, parent / 2)) : random.nextInt(7) == 0 ? 0 : 1 + random.nextInt(140);
	}

	private static void describe(Component c) {
		ServerActiveProperties server = (ServerActiveProperties) InterfaceList.properties.get(((long) c.id << 32) + (long) c.createdComponentId);
		int hooks = 0;
		for (int hook = 0; hook < HOOKS.length; hook++) {
			if (get(c, hook) != null) {
				hooks |= 1 << hook;
			}
		}
		out.append("{\"id\":").append(c.id).append(",\"slot\":").append(c.createdComponentId).append(",\"layer\":").append(c.overlayer)
			.append(",\"type\":").append(c.type).append(",\"if3\":").append(c.if3).append(",\"hidden\":").append(c.hidden)
			.append(",\"noClickThrough\":").append(c.noClickThrough).append(",\"code\":").append(c.clientCode);
		list("base", new int[]{c.baseX, c.baseY, c.baseWidth, c.baseHeight});
		list("modes", new int[]{c.dynamicWidthValue, c.dynamicHeightValue, c.xMode, c.yMode});
		list("aspect", new int[]{c.aspectWidth, c.aspectHeight});
		list("margin", new int[]{c.invMarginX, c.invMarginY});
		list("scroll", new int[]{c.scrollX, c.scrollY, c.scrollMaxH, c.scrollMaxV});
		out.append(",\"hooks\":").append(hooks).append(",\"handlers\":").append(c.hasEventHandlers);
		list("varcTriggers", c.varcTriggers);
		list("varcstrTriggers", c.varcstrTriggers);
		list("varpTriggers", c.varpTriggers);
		list("invTriggers", c.inventoryTriggers);
		list("statTriggers", c.statTriggers);
		list("readers", new int[]{c.updatedVarcsReaderIndex, c.updatedVarcstrsReaderIndex, c.updatedVarpsReaderIndex,
			c.updatedInventoriesReaderIndex, c.updatedStatsReaderIndex, c.lastTransmitTimer});
		out.append(",\"flags\":").append((c.mouseOver ? 1 : 0) | (c.dragging ? 2 : 0)).append(",\"events\":").append(c.properties.events)
			.append(",\"serverEvents\":").append(server == null ? "null" : Integer.toString(server.events)).append(",\"hover\":")
			.append(c.hoverOverlayer).append(",\"overColor\":").append(c.overColor).append(",\"dragParent\":")
			.append(ordinal(c.dragParent)).append('}');
	}

	/** Where a component sits on the canvas, for aiming the mouse only; null when it is not in the open tree. */
	private static int[] absolute(Component target) {
		return find(InterfaceList.components[InterfaceList.topLevelInterface], -1, 0, 0, target, 0);
	}

	private static int[] find(Component[] children, int layer, int x, int y, Component target, int depth) {
		if (children == null || depth > 20) {
			return null;
		}
		for (Component c : children) {
			if (c == null || c.overlayer != layer) {
				continue;
			}
			int cx = x + c.x, cy = y + c.y;
			if (c == target) {
				return new int[]{cx, cy, c.width, c.height};
			}
			if (c.type == 0) {
				int[] found = find(children, c.id, cx - c.scrollX, cy - c.scrollY, target, depth + 1);
				if (found == null && c.createdComponents != null && c.createdComponentId < 0) {
					found = find(c.createdComponents, c.id, cx - c.scrollX, cy - c.scrollY, target, depth + 1);
				}
				ComponentPointer pointer = (ComponentPointer) InterfaceList.openInterfaces.get(c.id);
				if (found == null && pointer != null) {
					found = find(InterfaceList.components[pointer.interfaceId], -1, cx, cy, target, depth + 1);
				}
				if (found != null) {
					return found;
				}
			}
		}
		return null;
	}
}
