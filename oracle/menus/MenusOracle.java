import java.util.ArrayList;
import java.util.Calendar;
import java.util.GregorianCalendar;
import java.util.List;
import java.util.Random;
import rt4.Buffer;
import rt4.CursorType;
import rt4.CursorTypeList;
import rt4.Font;
import rt4.IndexedSprite;
import rt4.JagString;

/**
 * Runs the unchanged rt4.Font wavy-text renderers and rt4.CursorType.decode and
 * prints one JSON object per case. Glyph and icon drawing is recorded instead
 * of rasterised: Font's two abstract glyph methods and IndexedSprite's two
 * abstract render methods are implemented here to log their arguments.
 *
 * The "clock" cases are not original code: client.mainLoop cannot be compiled
 * alone, so its seed expression is repeated here over java.util.Calendar.
 * The "random" cases are java.util.Random itself.
 */
public final class MenusOracle {
	static final class RecordingFont extends Font {
		final List<int[]> calls = new ArrayList<int[]>();

		RecordingFont(byte[] metrics, int[] xOffsets, int[] yOffsets, int[] widths, int[] heights) {
			super(metrics, xOffsets, yOffsets, widths, heights);
		}

		@Override
		protected void renderGlyph(int glyph, int x, int y, int w, int h, int color) {
			this.calls.add(new int[]{0, glyph, x, y, color, 256});
		}

		@Override
		protected void renderGlyphTransparent(int glyph, int x, int y, int w, int h, int color, int alpha) {
			this.calls.add(new int[]{1, glyph, x, y, color, alpha});
		}
	}

	static final class RecordingIcon extends IndexedSprite {
		final RecordingFont font;
		final int index;

		RecordingIcon(RecordingFont font, int index, int width, int height) {
			this.font = font;
			this.index = index;
			this.innerWidth = width;
			this.innerHeight = height;
		}

		@Override
		public void renderAlpha(int x, int y, int alpha) {
			this.font.calls.add(new int[]{2, this.index, x, y, 0, alpha});
		}

		@Override
		public void renderTransparent(int x, int y) {
			this.font.calls.add(new int[]{2, this.index, x, y, 0, 256});
		}
	}

	static final String[] PIECES = {
		"Walk here", "Talk-to", "Attack", "Use", "Cast", " ", " ", " ", " -> ", " / ", "<col=ffff00>", "<col=ffffff>",
		"<col=00ffff>", "<col=ff9040>", "</col>", "Hans", "Bucket", "Man", "  (level-2)", " more options", "2", "17",
		"<img=0>", "<img=1>", "<img=2>", "<img=7>", "<img=x>", "<img=>", "<lt>", "<gt>", "<nbsp>", "<", ">", "<>",
		"<lt", "a", "W", "i", "é", "£", "<br>", "<shy>", "<times>", "<euro>", "<copy>", "<reg>"
	};

	static String hex(byte[] bytes) {
		StringBuilder out = new StringBuilder();
		for (byte b : bytes) {
			out.append(Character.forDigit((b >> 4) & 15, 16)).append(Character.forDigit(b & 15, 16));
		}
		return out.toString();
	}

	static String ints(int[] values) {
		StringBuilder out = new StringBuilder("[");
		for (int i = 0; i < values.length; i++) {
			if (i != 0) {
				out.append(',');
			}
			out.append(values[i]);
		}
		return out.append(']').toString();
	}

	static String calls(List<int[]> calls) {
		StringBuilder out = new StringBuilder("[");
		for (int i = 0; i < calls.size(); i++) {
			if (i != 0) {
				out.append(',');
			}
			out.append(ints(calls.get(i)));
		}
		return out.append(']').toString();
	}

	/** Latin-1 bytes as the client's string type, without JagString.parse's escapes. */
	static JagString text(byte[] bytes) {
		JagString out = JagString.allocate(bytes.length + 1);
		for (byte b : bytes) {
			out.append(b & 0xFF);
		}
		return out;
	}

	static byte[] randomText(Random gen) {
		int count = gen.nextInt(14);
		StringBuilder out = new StringBuilder();
		for (int i = 0; i < count; i++) {
			out.append(PIECES[gen.nextInt(PIECES.length)]);
		}
		byte[] bytes = new byte[out.length()];
		for (int i = 0; i < bytes.length; i++) {
			bytes[i] = (byte) out.charAt(i);
		}
		return bytes;
	}

	static int randomSeed(Random gen) {
		// Mostly the range client.mainLoop produces, sometimes any int.
		return gen.nextInt(8) == 0 ? gen.nextInt() : gen.nextInt(14400);
	}

	public static void main(String[] args) {
		Random gen = new Random(530L);

		int[] seeds = new int[72];
		int[] fixed = {0, 1, -1, 2, 599, 600, 14399, 14400, Integer.MAX_VALUE, Integer.MIN_VALUE, 0x5DEECE66, -0x5DEECE67};
		for (int i = 0; i < seeds.length; i++) {
			seeds[i] = i < fixed.length ? fixed[i] : randomSeed(gen);
		}
		Random shared = new Random();
		for (int seed : seeds) {
			// The client keeps one generator and reseeds it for every draw.
			shared.setSeed(seed);
			int[] values = new int[12];
			for (int i = 0; i < values.length; i++) {
				values[i] = shared.nextInt();
			}
			System.out.println("{\"case\":\"random\",\"seed\":" + seed + ",\"ints\":" + ints(values) + "}");
		}

		int[][] times = {{0, 0, 0}, {0, 0, 5}, {0, 0, 6}, {0, 0, 59}, {0, 1, 0}, {0, 59, 59}, {1, 0, 0}, {11, 59, 59},
			{12, 0, 0}, {13, 7, 5}, {23, 59, 59}};
		for (int i = 0; i < 32; i++) {
			int[] time = i < times.length ? times[i] : new int[]{gen.nextInt(24), gen.nextInt(60), gen.nextInt(60)};
			GregorianCalendar calendar = new GregorianCalendar(2026, Calendar.JUNE, 15, time[0], time[1], time[2]);
			int seed = calendar.get(Calendar.HOUR_OF_DAY) * 600 + calendar.get(Calendar.MINUTE) * 10 + calendar.get(Calendar.SECOND) / 6;
			System.out.println("{\"case\":\"clock\",\"time\":" + ints(time) + ",\"seed\":" + seed + "}");
		}

		RecordingFont[] fonts = new RecordingFont[4];
		for (int f = 0; f < fonts.length; f++) {
			byte[] metrics = new byte[257];
			int[] yOffsets = new int[256];
			int[] heights = new int[256];
			int[] widths = new int[256];
			for (int i = 0; i < 256; i++) {
				metrics[i] = (byte) (f == 3 ? gen.nextInt(256) : gen.nextInt(15));
				// Some glyphs have no pixels, as in the cached fonts.
				heights[i] = gen.nextInt(6) == 0 ? 0 : 1 + gen.nextInt(14);
				yOffsets[i] = gen.nextInt(8);
				widths[i] = 1 + gen.nextInt(12);
			}
			metrics[256] = (byte) (8 + gen.nextInt(12));
			// Glyph x offsets are zero so a recorded x is the pen plus the shift.
			fonts[f] = new RecordingFont(metrics, new int[256], yOffsets, widths, heights);
			int[][] icons = f == 0 ? new int[0][] : new int[][]{{13, 13}, {11, 9 + f}, {8 + f, 14}};
			StringBuilder iconText = new StringBuilder("[");
			if (f != 0) {
				IndexedSprite[] sprites = new IndexedSprite[icons.length];
				for (int i = 0; i < icons.length; i++) {
					sprites[i] = new RecordingIcon(fonts[f], i, icons[i][0], icons[i][1]);
					iconText.append(i == 0 ? "" : ",").append(ints(icons[i]));
				}
				// Font 2 gives explicit icon heights, the others use the sprites' own.
				fonts[f].setNameIcons(sprites, f == 2 ? new int[]{12, 12, 12} : null);
			}
			iconText.append(']');
			System.out.println("{\"case\":\"font\",\"id\":" + f + ",\"metrics\":\"" + hex(metrics) + "\",\"y\":" + ints(yOffsets)
				+ ",\"heights\":" + ints(heights) + ",\"icons\":" + iconText + ",\"icon_heights\":"
				+ (f == 2 ? "[12,12,12]" : "null") + "}");
		}

		String[] menu = {
			"Walk here", "Talk-to <col=ffff00>Hans<col=ffffff> / 2 more options",
			"Attack <col=ffff00>Man<col=40ff00>  (level-2)<col=ffffff> / 3 more options", "Use <col=ff9040>Bucket ->",
			"Cast <col=00ff00>Wind Strike ->", "Follow <col=ffffff><img=1>Zezima<col=ff0000>  (level-126)<col=ffffff> / 4 more options", ""
		};
		for (int i = 0; i < 400; i++) {
			RecordingFont font = fonts[gen.nextInt(fonts.length)];
			byte[] bytes = i < menu.length ? menu[i].getBytes(java.nio.charset.StandardCharsets.ISO_8859_1) : randomText(gen);
			int x = gen.nextInt(1200) - 100;
			int y = gen.nextInt(900) - 100;
			int seed = randomSeed(gen);
			font.calls.clear();
			int extra = font.renderWavyText(text(bytes), x, y, shared, seed);
			System.out.println("{\"case\":\"wavy\",\"font\":" + java.util.Arrays.asList(fonts).indexOf(font) + ",\"text\":\"" + hex(bytes)
				+ "\",\"at\":" + ints(new int[]{x, y}) + ",\"seed\":" + seed + ",\"extra\":" + extra + ",\"calls\":" + calls(font.calls) + "}");
		}
		for (int i = 0; i < 400; i++) {
			RecordingFont font = fonts[gen.nextInt(fonts.length)];
			byte[] bytes = i < menu.length ? menu[i].getBytes(java.nio.charset.StandardCharsets.ISO_8859_1) : randomText(gen);
			int[] rect = {gen.nextInt(1200) - 100, gen.nextInt(900) - 100, gen.nextInt(600) - 20, gen.nextInt(80) - 5};
			int color = gen.nextInt(0x1000000);
			int shadow = gen.nextInt(3) == 0 ? -1 : gen.nextInt(2) == 0 ? 0 : gen.nextInt(0x1000000);
			int halign = gen.nextInt(4);
			int valign = gen.nextInt(4);
			int seed = randomSeed(gen);
			int[] bounds = new int[4];
			font.calls.clear();
			int extra = font.renderWavyTextAligned(text(bytes), rect[0], rect[1], rect[2], rect[3], color, shadow, halign, valign, shared, seed, bounds);
			System.out.println("{\"case\":\"aligned\",\"font\":" + java.util.Arrays.asList(fonts).indexOf(font) + ",\"text\":\"" + hex(bytes)
				+ "\",\"rect\":" + ints(rect) + ",\"color\":" + color + ",\"shadow\":" + shadow + ",\"halign\":" + halign + ",\"valign\":" + valign
				+ ",\"seed\":" + seed + ",\"extra\":" + extra + ",\"bounds\":" + ints(bounds) + ",\"calls\":" + calls(font.calls) + "}");
		}

		byte[][] cursors = {{0}, {1, 1, 44, 2, 7, (byte) 200, 0}, {9, 1, 0, 5, 1, (byte) 255, (byte) 255, 0, 1}, {}, {1, 1}, {2, 3, 4}, {2, 3}};
		for (int i = 0; i < 300; i++) {
			byte[] data;
			if (i < cursors.length) {
				data = cursors[i];
			} else {
				java.io.ByteArrayOutputStream out = new java.io.ByteArrayOutputStream();
				int parts = gen.nextInt(6);
				for (int p = 0; p < parts; p++) {
					int kind = gen.nextInt(5);
					if (kind == 0) {
						out.write(1);
						out.write(gen.nextInt(256));
						out.write(gen.nextInt(256));
					} else if (kind == 1) {
						out.write(2);
						out.write(gen.nextInt(256));
						out.write(gen.nextInt(256));
					} else if (kind == 2) {
						out.write(3 + gen.nextInt(253));
					} else {
						out.write(1 + gen.nextInt(2));
						out.write(1 + gen.nextInt(255));
						out.write(1 + gen.nextInt(255));
					}
				}
				// One case in five is cut short or lacks the terminator.
				if (gen.nextInt(5) != 0) {
					out.write(0);
				}
				data = out.toByteArray();
			}
			CursorType type = new CursorType();
			String result;
			try {
				type.decode(new Buffer(data), i);
				CursorTypeList.sprites.lastKey = -1L;
				type.getSprite();
				result = "\"sprite\":" + CursorTypeList.sprites.lastKey + ",\"hot\":" + ints(new int[]{type.hotSpotX, type.hotSpotY});
			} catch (ArrayIndexOutOfBoundsException e) {
				result = "\"error\":true";
			}
			System.out.println("{\"case\":\"cursor\",\"data\":\"" + hex(data) + "\"," + result + "}");
		}
	}
}
