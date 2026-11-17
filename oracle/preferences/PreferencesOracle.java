import rt4.*;

import java.io.File;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Random;

/**
 * Observations of the unchanged rt4 Preferences and Cheat classes.
 *
 * usage: PreferencesOracle <seed> <decodeCases> <scratchDirectory>
 *
 * One JSON object per line on standard output:
 *   read   Preferences.read over a real file (or none), from a scrambled state
 *   decode Preferences.decode of seeded bytes over a given state
 *   write  Preferences.write over an existing file of another length
 *   cheat  Cheat.execute / Cheat.teleport, with every call it makes on the adapters in order
 *
 * A state is the 29 Preferences fields in STATE order, booleans as 0 and 1.
 */
public class PreferencesOracle {
	/** The order of every state array in the fixture. */
	static final String STATE = "brightness,allLevelsVisible,removeRoofsSelectively,showGroundDecorations,highDetailTextures,"
		+ "manyIdleAnimations,flickeringEffectsOn,manyGroundTextures,characterShadowsOn,sceneryShadowsType,highDetailLighting,"
		+ "highWaterDetail,fogEnabled,windowMode,antiAliasingMode,stereo,soundEffectVolume,musicVolume,ambientSoundsVolume,"
		+ "fullScreenWidth,fullScreenHeight,particles,lastWorldId,favoriteWorlds,safeMode,keyboardCameraEnabled,buildArea,hdr,"
		+ "cursorsEnabled";

	static final int[] LENGTHS = {19, 23, 22, 23, 24, 28, 28, 29, 30, 31, 32, 33};
	static final int[] EDGES = {0, 1, 2, 3, 4, 5, 95, 96, 127, 128, 254, 255};
	static final int[] WILD = {-1, 0, 1, 2, 3, 4, 5, 7, 8, 127, 128, 255, 256, 300, 511, 65535, 65536, 70000, -70000,
		Integer.MAX_VALUE, Integer.MIN_VALUE};
	static final int[] MEMORY = {64, 95, 96, 97, 128, 512};

	static final PrintStream out = System.out;
	static final SignLink signLink = new SignLink();
	static File directory;
	static File file;

	static int b(boolean value) {
		return value ? 1 : 0;
	}

	static int[] state() {
		return new int[]{Preferences.brightness, b(Preferences.allLevelsVisible), b(Preferences.removeRoofsSelectively),
			b(Preferences.showGroundDecorations), b(Preferences.highDetailTextures), b(Preferences.manyIdleAnimations),
			b(Preferences.flickeringEffectsOn), b(Preferences.manyGroundTextures), b(Preferences.characterShadowsOn),
			Preferences.sceneryShadowsType, b(Preferences.highDetailLighting), b(Preferences.highWaterDetail),
			b(Preferences.fogEnabled), Preferences.windowMode, Preferences.antiAliasingMode, b(Preferences.stereo),
			Preferences.soundEffectVolume, Preferences.musicVolume, Preferences.ambientSoundsVolume,
			Preferences.fullScreenWidth, Preferences.fullScreenHeight, Preferences.getParticleSetting(),
			Preferences.lastWorldId, Preferences.favoriteWorlds, b(Preferences.safeMode),
			b(Preferences.keyboardCameraEnabled), Preferences.buildArea, b(Preferences.hdr), b(Preferences.cursorsEnabled)};
	}

	static void set(int[] s) {
		Preferences.brightness = s[0];
		Preferences.allLevelsVisible = s[1] == 1;
		Preferences.removeRoofsSelectively = s[2] == 1;
		Preferences.showGroundDecorations = s[3] == 1;
		Preferences.highDetailTextures = s[4] == 1;
		Preferences.manyIdleAnimations = s[5] == 1;
		Preferences.flickeringEffectsOn = s[6] == 1;
		Preferences.manyGroundTextures = s[7] == 1;
		Preferences.characterShadowsOn = s[8] == 1;
		Preferences.sceneryShadowsType = s[9];
		Preferences.highDetailLighting = s[10] == 1;
		Preferences.highWaterDetail = s[11] == 1;
		Preferences.fogEnabled = s[12] == 1;
		Preferences.windowMode = s[13];
		Preferences.antiAliasingMode = s[14];
		Preferences.stereo = s[15] == 1;
		Preferences.soundEffectVolume = s[16];
		Preferences.musicVolume = s[17];
		Preferences.ambientSoundsVolume = s[18];
		Preferences.fullScreenWidth = s[19];
		Preferences.fullScreenHeight = s[20];
		Preferences.setParticles(s[21]);
		Preferences.lastWorldId = s[22];
		Preferences.favoriteWorlds = s[23];
		Preferences.safeMode = s[24] == 1;
		Preferences.keyboardCameraEnabled = s[25] == 1;
		Preferences.buildArea = s[26];
		Preferences.hdr = s[27] == 1;
		Preferences.cursorsEnabled = s[28] == 1;
	}

	static final boolean[] FLAG = {false, true, true, true, true, true, true, true, true, false, true, true, true, false,
		false, true, false, false, false, false, false, false, false, false, true, true, false, true, true};

	static int[] scrambled(Random r) {
		int[] s = new int[29];
		for (int i = 0; i < s.length; i++) {
			if (FLAG[i]) {
				s[i] = r.nextInt(2);
			} else if (r.nextInt(3) == 0) {
				s[i] = r.nextInt();
			} else {
				s[i] = WILD[r.nextInt(WILD.length)];
			}
		}
		return s;
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

	static String hex(byte[] bytes, int length) {
		StringBuilder sb = new StringBuilder();
		for (int i = 0; i < length; i++) {
			sb.append(String.format("%02x", bytes[i] & 0xFF));
		}
		return sb.toString();
	}

	static String hex(byte[] bytes) {
		return hex(bytes, bytes.length);
	}

	static String quote(String text) {
		StringBuilder sb = new StringBuilder("\"");
		for (char c : text.toCharArray()) {
			if (c == '"' || c == '\\') {
				sb.append('\\').append(c);
			} else if (c < 32 || c > 126) {
				sb.append(String.format("\\u%04x", (int) c));
			} else {
				sb.append(c);
			}
		}
		return sb.append('"').toString();
	}

	/** The observations every preference case ends with. */
	static String observed() {
		Buffer encoded = Preferences.encode();
		return "\"after\":" + ints(state()) + ",\"encoded\":\"" + hex(encoded.data, encoded.offset) + "\",\"toInt\":"
			+ Preferences.toInt();
	}

	static byte[] data(Random r, int version, int length) {
		byte[] data = new byte[length];
		for (int i = 0; i < length; i++) {
			data[i] = (byte) (r.nextBoolean() ? EDGES[r.nextInt(EDGES.length)] : r.nextInt(256));
		}
		if (length > 0) {
			data[0] = (byte) version;
		}
		return data;
	}

	static int version(Random r) {
		int pick = r.nextInt(10);
		return pick == 0 ? 12 + r.nextInt(244) : r.nextInt(12);
	}

	static int length(Random r, int version) {
		int needed = 1 + (version < 12 ? LENGTHS[version] : 33);
		switch (r.nextInt(10)) {
			case 0:
				return 0;
			case 1:
				return 1;
			case 2:
				return needed - 1;
			case 3:
				return needed + 1 + r.nextInt(8);
			case 4:
				return 1 + r.nextInt(needed);
			default:
				return needed;
		}
	}

	static void reset() throws Exception {
		Files.deleteIfExists(file.toPath());
	}

	static byte[] fileBytes() throws Exception {
		return file.exists() ? Files.readAllBytes(file.toPath()) : null;
	}

	static String fileJson(byte[] bytes) {
		return bytes == null ? "null" : "\"" + hex(bytes) + "\"";
	}

	static void decodeCases(Random r, int count) {
		for (int i = 0; i < count; i++) {
			int memory = MEMORY[r.nextInt(MEMORY.length)];
			GameShell.maxMemory = memory;
			GlRenderer.enabled = r.nextBoolean();
			int[] before;
			if (r.nextBoolean()) {
				signLink.unavailable = true;
				Preferences.read(signLink);
				signLink.unavailable = false;
				before = state();
			} else {
				before = scrambled(r);
				set(before);
			}
			int version = i < 24 ? i / 2 : version(r);
			byte[] data = i < 24 ? data(r, version, 1 + LENGTHS[version] + (i & 1) * 3) : data(r, version, length(r, version));
			Preferences.decode(new Buffer(data));
			out.println("{\"case\":\"decode\",\"maxMemory\":" + memory + ",\"before\":" + ints(before) + ",\"data\":\""
				+ hex(data) + "\"," + observed() + "}");
		}
	}

	static void readCases(Random r, int count) throws Exception {
		for (int i = 0; i < count; i++) {
			int memory = MEMORY[r.nextInt(MEMORY.length)];
			GameShell.maxMemory = memory;
			GlRenderer.enabled = r.nextBoolean();
			set(scrambled(r));
			reset();
			byte[] stored = null;
			signLink.unavailable = false;
			if (i % 16 == 0) {
				// No file: the original creates an empty one by opening it "rw".
			} else if (i % 16 == 1) {
				signLink.unavailable = true;
			} else {
				int version = i % 16 < 14 ? i % 16 - 2 : version(r);
				stored = i % 16 < 14 ? data(r, version, 1 + LENGTHS[version]) : data(r, version, length(r, version));
				Files.write(file.toPath(), stored);
			}
			Preferences.read(signLink);
			out.println("{\"case\":\"read\",\"maxMemory\":" + memory + ",\"unavailable\":" + signLink.unavailable
				+ ",\"file\":" + fileJson(stored) + "," + observed() + ",\"fileAfter\":" + fileJson(fileBytes()) + "}");
			signLink.unavailable = false;
		}
	}

	static void writeCases(Random r, int count) throws Exception {
		for (int i = 0; i < count; i++) {
			GameShell.maxMemory = 128;
			int[] before = scrambled(r);
			set(before);
			reset();
			byte[] stored = null;
			if (i % 4 != 0) {
				stored = new byte[i % 4 == 1 ? r.nextInt(34) : 34 + r.nextInt(30)];
				r.nextBytes(stored);
				Files.write(file.toPath(), stored);
			}
			Preferences.write(signLink);
			out.println("{\"case\":\"write\",\"state\":" + ints(before) + ",\"file\":" + fileJson(stored) + ",\"fileAfter\":"
				+ fileJson(fileBytes()) + "}");
		}
	}

	static JagString raw(String text) {
		byte[] bytes = text.getBytes(StandardCharsets.ISO_8859_1);
		return JagString.decodeString(bytes, bytes.length, 0);
	}

	static String collision() {
		boolean cleared = true;
		boolean kept = true;
		for (CollisionMap map : PathFinder.collisionMaps) {
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					boolean inner = x >= 1 && x < 103 && y >= 1 && y < 103;
					int flag = map.flags[x][y];
					if (flag != 7) {
						kept = false;
					}
					if (flag != (inner ? 0 : 7)) {
						cleared = false;
					}
				}
			}
		}
		return kept ? "kept" : cleared ? "inner 1..102 cleared" : "other";
	}

	/** One command from the given switches; a null command is Cheat.teleport(x, y, plane). */
	static void cheat(String command, int modeWhere, boolean gl, boolean tween, boolean shift, int x, int y, int plane)
		throws Exception {
		GameShell.maxMemory = 128;
		GameShell.replaceCanvas = false;
		client.modeWhere = modeWhere;
		GlRenderer.enabled = gl;
		SeqType.applyTweening = tween;
		Cheat.shiftClick = shift;
		Cheat.displayFps = false;
		Cheat.rectDebug = 0;
		Cheat.qaOpTest = false;
		reset();
		Preferences.read(signLink);
		Preferences.sentToServer = true;
		for (int i = 0; i < 4; i++) {
			PathFinder.collisionMaps[i] = new CollisionMap();
			for (int[] row : PathFinder.collisionMaps[i].flags) {
				java.util.Arrays.fill(row, 7);
			}
		}
		OracleLog.events.clear();
		String error = null;
		System.setOut(new PrintStream(out) {
			@Override
			public void println(String line) {
				OracleLog.add("stdout " + line);
			}
		});
		try {
			if (command == null) {
				Cheat.teleport(x, y, plane);
			} else {
				Cheat.execute(raw(command));
			}
		} catch (Throwable t) {
			error = t.getClass().getName();
		} finally {
			System.setOut(out);
		}
		StringBuilder sb = new StringBuilder("{\"case\":\"cheat\",");
		if (command == null) {
			sb.append("\"teleport\":[").append(x).append(',').append(y).append(',').append(plane).append("],");
		} else {
			sb.append("\"command\":").append(quote(command)).append(',');
		}
		sb.append("\"modeWhere\":").append(modeWhere).append(",\"gl\":").append(gl).append(",\"tween\":").append(tween)
			.append(",\"shift\":").append(shift).append(",\"events\":[");
		for (int i = 0; i < OracleLog.events.size(); i++) {
			if (i > 0) {
				sb.append(',');
			}
			sb.append(quote(OracleLog.events.get(i)));
		}
		sb.append("],\"error\":").append(error == null ? "null" : quote(error));
		sb.append(",\"displayFps\":").append(Cheat.displayFps).append(",\"shiftClick\":").append(Cheat.shiftClick)
			.append(",\"rectDebug\":").append(Cheat.rectDebug).append(",\"qaOpTest\":").append(Cheat.qaOpTest)
			.append(",\"tweening\":").append(SeqType.applyTweening).append(",\"particles\":")
			.append(Preferences.getParticleSetting()).append(",\"sentToServer\":").append(Preferences.sentToServer)
			.append(",\"replaceCanvas\":").append(GameShell.replaceCanvas).append(",\"collision\":")
			.append(quote(collision())).append(",\"preferencesFile\":").append(fileJson(fileBytes())).append('}');
		out.println(sb);
	}

	static final String[] COMMANDS = {
		"::gc", "::GC", "::mm", "::pcachesize", "::cardmem", "::clientdrop", "::ClientDrop", "::clientjs5drop",
		"::serverjs5drop", "::breakcon", "::replacecanvas", "::rebuild", "::REBUILD", "::fpson", "::fpsoff", "::FPSON",
		"::wm0", "::wm1", "::wm2", "::wm3", "::WM3", "::wm4", "::noclip", "::NoClip", "::noclip ",
		"::setparticles 0", "::setparticles 1", "::setparticles 2", "::setparticles 300", "::setparticles -1",
		"::setparticles +2", "::setparticles", "::setparticlesX", "::setparticles=7", "::setparticles  2",
		"::setparticles 2 ", "::setparticles 2x", "::setparticles 99999999999", "::SETPARTICLES 1", "::setparticles 0002",
		"::setparticles -", "::setparticles a",
		"::fps 30", "::fps 0", "::fps -5", "::fps", "::fps ", "::fps x", "::fps 30 ", "::FPS 30", "::fpsx",
		"::errortest", "::ERRORTEST", "::errortest ",
		"::rect_debug 2", "::rect_debug2", "::rect_debug   3  ", "::rect_debug", "::rect_debugx", "::rect_debug -1",
		"::rect_debug 1 ", "::RECT_DEBUG 1", "::rect_debug 1 2",
		"::qa_op_test", "::QA_OP_TEST", "::tween", "::TWEEN", "::tweening", "::shiftclick", "::ShiftClick",
		"::reloadplugins", "::tele 0,50,50,10,10", "::unknown", "::", "::item 995 100", "::É", "gc", ":::gc",
	};

	public static void main(String[] args) throws Exception {
		Random r = new Random(Long.parseLong(args[0]));
		int decodeCases = Integer.parseInt(args[1]);
		directory = new File(args[2]);
		if (!directory.isDirectory() && !directory.mkdirs()) {
			throw new IllegalStateException("cannot create " + directory);
		}
		signLink.directory = directory;
		GameShell.signLink = signLink;
		file = new File(directory, "jagex_runescape_preferences.dat");
		out.println("{\"case\":\"layout\",\"state\":\"" + STATE + "\"}");
		// The initial values of the class itself, before any read.
		out.println("{\"case\":\"initial\"," + observed() + ",\"sentToServer\":" + Preferences.sentToServer + "}");
		readCases(r, 192);
		decodeCases(r, decodeCases);
		writeCases(r, 48);
		for (String command : COMMANDS) {
			for (int variant = 0; variant < 4; variant++) {
				cheat(command, variant < 2 ? 2 : 0, variant % 2 == 0, variant % 2 == 0, variant < 2, 0, 0, 0);
			}
		}
		int[][] tiles = {{3222, 3218, 0}, {0, 0, 0}, {3200, 3200, 3}, {16383, 16383, 1}, {63, 64, 2}, {-1, -65, 0},
			{2944, 3392, -1}, {12345, 54321, 7}};
		for (int[] tile : tiles) {
			cheat(null, 2, true, true, true, tile[0], tile[1], tile[2]);
		}
		reset();
	}
}
