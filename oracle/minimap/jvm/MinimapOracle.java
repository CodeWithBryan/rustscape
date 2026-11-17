import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import rt4.*;

/**
 * Observations for the minimap's drift, player dots and text labels.
 *
 * The statements under test are the excerpts of Protocol, client, SceneGraph,
 * MiniMap and Player that the generated rt4.MiniMap holds; Font, JagString,
 * Base37, MapElementList, MathUtils, SpriteLoader and the cache classes are
 * the unchanged originals. Prints one JSON object per line.
 *
 * Usage: MinimapOracle CACHE_DIRECTORY SEED
 */
public final class MinimapOracle {
	private static final List<BufferedFile> OPEN = new ArrayList<BufferedFile>();
	private static final long NAME_LIMIT = 6582952005840035281L;

	static Js5 archive(Path directory, int id) throws Exception {
		BufferedFile data = file(directory.resolve("main_file_cache.dat2"));
		BufferedFile idx255 = file(directory.resolve("main_file_cache.idx255"));
		BufferedFile idx = file(directory.resolve("main_file_cache.idx" + id));
		Cache references = new Cache(255, data, idx255, 16 * 1024 * 1024);
		final Cache storage = new Cache(id, data, idx, 16 * 1024 * 1024);
		byte[] reference = references.read(id);
		final Js5Index index = new Js5Index(reference, Buffer.crc32(reference, reference.length));
		Js5ResourceProvider provider = new Js5ResourceProvider() {
			public Js5Index fetchIndex() {return index;}
			public byte[] fetchGroup(int group) {return storage.read(group);}
			public void prefetchGroup(int group) {}
			public int getPercentageComplete(int group) {return 100;}
		};
		return new Js5(provider, false, false);
	}

	private static BufferedFile file(Path path) throws Exception {
		BufferedFile opened = new BufferedFile(new FileOnDisk(path.toFile(), "r", Long.MAX_VALUE), 4096, 0);
		OPEN.add(opened);
		return opened;
	}

	static String hex(byte[] bytes) {
		StringBuilder out = new StringBuilder();
		for (byte value : bytes) out.append(String.format("%02x", value & 255));
		return out.toString();
	}

	static String ints(int[] values) {
		StringBuilder out = new StringBuilder("[");
		for (int i = 0; i < values.length; i++) out.append(i == 0 ? "" : ",").append(values[i]);
		return out.append("]").toString();
	}

	static int[] drift() {
		return new int[] {MiniMap.Camera.cameraOffsetX, MiniMap.Camera.cameraOffsetY, MiniMap.Camera.yawDrift,
			MiniMap.compassAngleOffset, MiniMap.zoomOffset, MiniMap.Camera.offsetXDelta, MiniMap.cameraYOffsetDelta,
			MiniMap.yawDriftDelta, MiniMap.angleJitterStep, MiniMap.zoomJitterStep, MiniMap.cameraJitterTimer,
			MiniMap.jitterTimer};
	}

	static void setDrift(int[] v) {
		MiniMap.Camera.cameraOffsetX = v[0];
		MiniMap.Camera.cameraOffsetY = v[1];
		MiniMap.Camera.yawDrift = v[2];
		MiniMap.compassAngleOffset = v[3];
		MiniMap.zoomOffset = v[4];
		MiniMap.Camera.offsetXDelta = v[5];
		MiniMap.cameraYOffsetDelta = v[6];
		MiniMap.yawDriftDelta = v[7];
		MiniMap.angleJitterStep = v[8];
		MiniMap.zoomJitterStep = v[9];
		MiniMap.cameraJitterTimer = v[10];
		MiniMap.jitterTimer = v[11];
	}

	/** Runs cycles from the current state and reports the state after every cycle that changed a value. */
	static void driftCase(String name, long seed, boolean reset, int cycles) {
		MiniMap.Math.source = new Random(seed);
		StringBuilder out = new StringBuilder("{\"case\":\"drift\",\"name\":\"" + name + "\",\"seed\":" + seed
			+ ",\"reset\":" + reset + ",\"cycles\":" + cycles + ",\"start\":" + ints(drift()));
		if (reset) {
			MiniMap.reset();
			out.append(",\"yawTarget\":").append((int) MiniMap.Camera.yawTarget).append(",\"afterReset\":").append(ints(drift()));
		}
		out.append(",\"changes\":[");
		int[] previous = drift();
		boolean first = true;
		for (int cycle = 1; cycle <= cycles; cycle++) {
			MiniMap.cycle();
			int[] now = drift();
			boolean changed = false;
			for (int i = 0; i < 10; i++) changed |= now[i] != previous[i];
			if (changed || cycle == cycles) {
				out.append(first ? "" : ",").append("[").append(cycle).append(",").append(ints(now)).append("]");
				first = false;
			}
			previous = now;
		}
		System.out.println(out.append("]}"));
	}

	static void floorCase(long seed, int steps) {
		MiniMap.Math.source = new Random(seed);
		StringBuilder out = new StringBuilder("{\"case\":\"floorStep\",\"seed\":" + seed + ",\"start\":["
			+ MiniMap.hueJitter + "," + MiniMap.lightnessJitter + "],\"values\":[");
		for (int i = 0; i < steps; i++) {
			MiniMap.floorStep();
			out.append(i == 0 ? "" : ",").append(MiniMap.hueJitter).append(",").append(MiniMap.lightnessJitter);
		}
		System.out.println(out.append("]}"));
	}

	static long name(Random random) {
		switch (random.nextInt(8)) {
			case 0: return random.nextInt(60) - 10;
			case 1: return 37L * (1 + random.nextInt(1000000));
			case 2: return NAME_LIMIT - 3 + random.nextInt(6);
			case 3: return random.nextLong();
			case 4: return 1 + random.nextInt(37 * 37 * 37);
			default: return 1 + (random.nextLong() >>> 1) % (NAME_LIMIT - 1);
		}
	}

	static void usernameCases(Random random, int count) {
		StringBuilder out = new StringBuilder("{\"case\":\"username\",\"keys\":[");
		for (int i = 0; i < count; i++) {
			long key = name(random);
			MiniMap.Player player = new MiniMap.Player();
			String encoded;
			try {
				player.decodeName(key);
				encoded = "\"" + player.username.encode37() + "\"";
			} catch (NullPointerException e) {
				encoded = "null";
			}
			out.append(i == 0 ? "" : ",").append("[\"").append(key).append("\",").append(encoded).append("]");
		}
		System.out.println(out.append("]}"));
	}

	static void dotCases(Random random, int count) {
		long[] pool = new long[14];
		for (int i = 0; i < pool.length; i++) {
			do {
				pool[i] = 1 + (random.nextLong() >>> 1) % (NAME_LIMIT - 1);
			} while (pool[i] % 37 == 0);
		}
		MiniMap.Sprites.mapdots = new Sprite[6];
		for (int i = 0; i < 6; i++) MiniMap.Sprites.mapdots[i] = new Sprite();
		MiniMap.Component component = new MiniMap.Component();
		for (int n = 0; n < count; n++) {
			StringBuilder out = new StringBuilder("{\"case\":\"dots\",\"friends\":[");
			MiniMap.FriendsList.size = random.nextInt(7);
			for (int i = 0; i < MiniMap.FriendsList.size; i++) {
				MiniMap.FriendsList.encodedUsernames[i] = pool[random.nextInt(pool.length)];
				MiniMap.FriendsList.worlds[i] = random.nextInt(3) == 0 ? 0 : 1 + random.nextInt(200);
				out.append(i == 0 ? "" : ",").append("[\"").append(MiniMap.FriendsList.encodedUsernames[i]).append("\",")
					.append(MiniMap.FriendsList.worlds[i]).append("]");
			}
			// Entries past size are stale and must not match.
			MiniMap.FriendsList.encodedUsernames[MiniMap.FriendsList.size] = pool[0];
			MiniMap.FriendsList.worlds[MiniMap.FriendsList.size] = 1;
			out.append("],\"clan\":[");
			MiniMap.ClanChat.size = random.nextInt(6);
			MiniMap.ClanChat.members = new ClanMember[MiniMap.ClanChat.size + 1];
			for (int i = 0; i <= MiniMap.ClanChat.size; i++) {
				MiniMap.ClanChat.members[i] = new ClanMember();
				MiniMap.ClanChat.members[i].key = pool[random.nextInt(pool.length)];
				if (i < MiniMap.ClanChat.size) {
					out.append(i == 0 ? "" : ",").append("\"").append(MiniMap.ClanChat.members[i].key).append("\"");
				}
			}
			MiniMap.PlayerList.self = new MiniMap.Player();
			MiniMap.PlayerList.self.team = random.nextInt(3) == 0 ? 0 : 1 + random.nextInt(3);
			out.append("],\"team\":").append(MiniMap.PlayerList.self.team).append(",\"players\":[");
			MiniMap.PlayerList.size = 1 + random.nextInt(10);
			for (int i = 0; i < MiniMap.PlayerList.size; i++) {
				MiniMap.Player player = new MiniMap.Player();
				long key = pool[random.nextInt(pool.length)];
				player.decodeName(key);
				player.team = random.nextInt(3) == 0 ? 0 : 1 + random.nextInt(3);
				MiniMap.PlayerList.ids[i] = 100 + i;
				MiniMap.PlayerList.players[100 + i] = player;
				out.append(i == 0 ? "" : ",").append("[\"").append(key).append("\",").append(player.team).append("]");
			}
			MinimapRecorder.events.clear();
			MiniMap.dots(0, 0, component);
			out.append("],\"frames\":[");
			for (int i = 0; i < MinimapRecorder.events.size(); i++) {
				out.append(i == 0 ? "" : ",").append(MinimapRecorder.events.get(i)[0]);
			}
			System.out.println(out.append("]}"));
		}
	}

	static byte[] labelFile(String name, int flags, int x, int y, int color) {
		byte[] text = name.getBytes(java.nio.charset.StandardCharsets.ISO_8859_1);
		byte[] out = new byte[text.length + 10];
		System.arraycopy(text, 0, out, 0, text.length);
		int at = text.length;
		out[at++] = 0;
		out[at++] = (byte) flags;
		out[at++] = (byte) (x >> 8);
		out[at++] = (byte) x;
		out[at++] = (byte) (y >> 8);
		out[at++] = (byte) y;
		out[at++] = (byte) (color >> 24);
		out[at++] = (byte) (color >> 16);
		out[at++] = (byte) (color >> 8);
		out[at] = (byte) color;
		return out;
	}

	static void listCase(String name, MapElementList list, byte[][] files, boolean forced) {
		int shown = 0;
		for (int i = 0; i < list.count; i++) {
			if (list.isMinimapLabelVisible(i)) shown++;
		}
		StringBuilder out = new StringBuilder("{\"case\":\"list\",\"name\":\"" + name + "\",\"count\":" + list.count
			+ ",\"minimap\":" + shown + ",\"forced\":" + forced + ",\"files\":[");
		for (int i = 0; i < files.length; i++) out.append(i == 0 ? "" : ",").append("\"").append(hex(files[i])).append("\"");
		System.out.println(out.append("]}"));
	}

	static void labelCases(String name, MapElementList list, Random random, int count) {
		MiniMap.LoginManager.mapElementList = list;
		int[][] sizes = {{152, 152}, {146, 151}, {200, 180}, {96, 60}};
		for (int n = 0; n < count; n++) {
			MiniMap.Component component = new MiniMap.Component();
			int[] size = sizes[random.nextInt(sizes.length)];
			component.width = size[0];
			component.height = size[1];
			int target = random.nextInt(list.count);
			// A scene whose player stands within a few tiles of one label.
			int localX = 16 + random.nextInt(72), localY = 16 + random.nextInt(72);
			MiniMap.Camera.originX = list.coordX[target] - localX + random.nextInt(25) - 12;
			MiniMap.Camera.originY = list.coordY[target] - localY + random.nextInt(25) - 12;
			MiniMap.PlayerList.self = new MiniMap.Player();
			MiniMap.PlayerList.self.xFine = localX * 128 + random.nextInt(128);
			MiniMap.PlayerList.self.yFine = localY * 128 + random.nextInt(128);
			MiniMap.Camera.yawTarget = random.nextInt(2048);
			MiniMap.compassAngleOffset = random.nextInt(125) - 62;
			MiniMap.zoomOffset = random.nextInt(33) - 21;
			int x = random.nextInt(700), y = random.nextInt(400);
			MinimapRecorder.events.clear();
			MiniMap.labels(x, y, component);
			if (SoftwareRaster.masks != 0) throw new IllegalStateException("unbalanced line masks");
			StringBuilder out = new StringBuilder("{\"case\":\"labels\",\"list\":\"" + name + "\",\"args\":" + ints(new int[] {
				size[0], size[1], MiniMap.Camera.originX, MiniMap.Camera.originY, MiniMap.PlayerList.self.xFine,
				MiniMap.PlayerList.self.yFine, (int) MiniMap.Camera.yawTarget, MiniMap.compassAngleOffset,
				MiniMap.zoomOffset}) + ",\"draws\":[");
			for (int i = 0; i < MinimapRecorder.events.size(); i++) {
				int[] event = MinimapRecorder.events.get(i).clone();
				// Positions relative to the component.
				if (event[0] == -1) {
					event[1] -= x;
					event[2] -= y;
				} else {
					event[2] -= x;
					event[3] -= y;
				}
				out.append(i == 0 ? "" : ",").append(ints(event));
			}
			System.out.println(out.append("]}"));
		}
	}

	public static void main(String[] args) throws Exception {
		Path directory = Paths.get(args[0]);
		long seed = Long.parseLong(args[1]);
		// SceneGraph's static initialisers ran when rt4.MiniMap loaded, from -Dminimap.seed.
		System.out.println("{\"case\":\"floorStart\",\"seed\":" + Long.getLong("minimap.seed", 0L)
			+ ",\"values\":[" + MiniMap.hueJitter + "," + MiniMap.lightnessJitter + "]}");
		Random random = new Random(seed);
		for (int i = 0; i < 6; i++) floorCase(random.nextLong(), 400);
		int[] initial = drift();
		for (int i = 0; i < 8; i++) {
			setDrift(initial);
			driftCase("session", random.nextLong(), true, 60000);
		}
		int[][] edges = {
			{49, 53, 39, 59, 9, 2, 2, 1, 2, 1, 495, 495},
			{-49, -53, -39, -59, -19, -2, -2, -1, -2, -1, 0, 250},
			{51, 57, 41, 61, 11, 2, 2, 1, 2, 1, 500, 500},
			{-51, -57, -41, -61, -21, -2, -2, -1, -2, -1, 499, 501},
		};
		for (int[] edge : edges) {
			setDrift(edge);
			driftCase("edge", random.nextLong(), false, 40000);
		}
		usernameCases(random, 600);
		dotCases(random, 250);

		Js5 sprites = archive(directory, 8), fonts = archive(directory, 13), maps = archive(directory, 23);
		String[] names = {"p11_full", "p12_full", "b12_full"};
		Font[] loaded = new Font[3];
		for (int i = 0; i < 3; i++) {
			int id = sprites.getGroupId(JagString.parse(names[i]));
			byte[] data = fonts.fetchFile(id, 0);
			SoftwareFont font = (SoftwareFont) Font.load(id, sprites, fonts);
			font.index = i;
			loaded[i] = font;
			java.lang.reflect.Field top = Font.class.getDeclaredField("paragraphTopPadding");
			java.lang.reflect.Field bottom = Font.class.getDeclaredField("paragraphBottomPadding");
			top.setAccessible(true);
			bottom.setAccessible(true);
			int[] boxes = new int[256 * 4];
			for (int g = 0; g < 256; g++) {
				boxes[g * 4] = font.xOffsets[g];
				boxes[g * 4 + 1] = font.yOffsets[g];
				boxes[g * 4 + 2] = font.innerWidths[g];
				boxes[g * 4 + 3] = font.innerHeights[g];
			}
			System.out.println("{\"case\":\"font\",\"index\":" + i + ",\"name\":\"" + names[i] + "\",\"id\":" + id
				+ ",\"lineHeight\":" + font.lineHeight + ",\"padding\":[" + top.getInt(font) + "," + bottom.getInt(font)
				+ "],\"data\":\"" + hex(data) + "\",\"boxes\":" + ints(boxes) + "}");
		}
		MiniMap.Fonts.p11Full = loaded[0];
		MiniMap.Fonts.p12Full = loaded[1];
		MiniMap.Fonts.b12Full = loaded[2];

		// The pinned cache's only world map, as LoginManager reads its labels.
		JagString group = JagString.parse("main_labels");
		int groupId = maps.getGroupId(group);
		int[] fileIds = maps.getFileIds(groupId);
		byte[][] files = new byte[fileIds.length][];
		for (int i = 0; i < fileIds.length; i++) files[i] = maps.fetchFile(groupId, fileIds[i]);
		MapElementList main = MapElementList.create(group, maps);
		listCase("main", main, files, false);
		labelCases("main", main, random, 20);
		// No label of the cache carries the minimap flag; set it to exercise the layout.
		for (int i = 0; i < main.count; i++) {
			main.flags[i] |= 4;
			files[i][main.names[i].length() + 1] |= 4;
		}
		listCase("main-forced", main, files, true);
		labelCases("main-forced", main, random, 110);

		String[] texts = {
			"Plain", "Two words", "A much longer label that has to wrap over several lines of text",
			"Hyphen-ated-words-break-after-the-hyphen-without-a-space", "Line<br>break", "<col=ff0000>Red</col> then white",
			"<shad=00ff00>Green shadow</shad> and <shad>black", "<trans=128>Half</trans> clear", "<str>Struck</str> <u>under</u> <u=ff>blue",
			"<str=123456>x", "Tom <lt>3 <gt> Jerry <times> <euro> <copy> <reg> <nbsp> <shy>", "<img=0>icon", "<col=zz>bad colour",
			"<col=7fffffff>big <col=80000000>overflow", "unterminated <col=ff", "> stray", "", " ", "Two  spaces", "Castle<br><br>Keep",
			"<unknown>tag", "Wrap <col=ffff00>across a colour change that carries over the line break</col> end", "AVAVAV WAWAWA To To",
			"été über £", "<br>leading break", "trailing break<br>", "<trans=x>bad", "<u></u>",
			"Supercalifragilisticexpialidocious-and-then-some more words here", "a-b-c-d-e-f-g-h-i-j-k-l-m-n-o-p-q-r-s-t-u-v-w-x-y-z-0-1-2-3-4-5-6-7-8-9",
		};
		MapElementList synthetic = new MapElementList(texts.length);
		byte[][] made = new byte[texts.length][];
		for (int i = 0; i < texts.length; i++) {
			int flags = (i % 5 == 4 ? 0 : 4) | (i * 7 % 4) | 8;
			int x = 3200 + (i % 6) * 14, y = 3200 + (i / 6) * 16;
			int color = i % 3 == 0 ? -1 : (i * 0x3a5c71) & 0xffffff;
			made[i] = labelFile(texts[i], flags, x, y, color);
			Buffer buffer = new Buffer(made[i]);
			synthetic.names[i] = buffer.gjstr();
			synthetic.flags[i] = buffer.g1b();
			synthetic.coordX[i] = (short) buffer.g2();
			synthetic.coordY[i] = (short) buffer.g2();
			synthetic.colors[i] = buffer.g4();
		}
		listCase("synthetic", synthetic, made, false);
		labelCases("synthetic", synthetic, random, 80);
		for (BufferedFile opened : OPEN) opened.close();
	}
}
