package rt4;

import java.io.PrintWriter;
import java.util.Random;

// Drives the title-screen scene code on a JVM and prints one line per case:
// the inputs, " -> ", what the original did. Built and run by
// `cargo xtask title-oracle`; crates/client/tests/title_scene.rs reproduces
// every line with rs09_client::title_scene and rs09_client::camera.
//
// - region: LoginManager.setupLoadingScreenRegion and the loadRegion it
//   ends in, three calls in a row with the camera moved between them.
// - spawn: LoginManager.decodeNpcFiles.
// - wander: the NPC pass of client.mainUpdate over a CollisionMap.
// - spline, locked: client.mainUpdate's camera part (Camera.
//   updateLoginScreenCamera / updateLockedCamera, then the scene set up
//   again when the camera leaves its middle).
public final class TitleSceneOracle {
	// The regions of the pinned cache that have an NPC file.
	static final int[][] TITLE = {{25, 83}, {26, 82}, {29, 66}, {39, 44}, {39, 51}, {40, 44}, {40, 51}, {41, 51}, {42, 50}, {42, 83}, {43, 66}, {45, 53}, {45, 58}, {46, 49}, {46, 50}, {47, 155}, {47, 160}, {48, 155}, {49, 71}, {50, 50}, {50, 51}, {50, 52}, {50, 53}, {51, 49}, {51, 50}, {53, 54}, {53, 154}, {54, 54}, {56, 152}, {59, 44}};
	static Random random;
	static PrintWriter out;

	static int range(int low, int high) {
		return low + random.nextInt(high - low + 1);
	}

	static void clearNpcs() {
		for (int i = 0; i < NpcList.size; i++) {
			NpcList.npcs[NpcList.ids[i]] = null;
		}
		for (int i = 0; i < 32768; i++) {
			NpcList.npcs[i] = null;
		}
		NpcList.size = 0;
	}

	// processLogout's zeroes and a fresh local player.
	static void loggedOut() {
		clearNpcs();
		Camera.originX = 0;
		Camera.originY = 0;
		SceneGraph.centralZoneX = 0;
		SceneGraph.centralZoneY = 0;
		SceneGraph.centralPlane = 0;
		Camera.resetCameraEffects();
		PlayerList.self = new Player();
		PlayerList.self.xFine = 3000;
		PlayerList.self.yFine = 3000;
		client.gameState = 10;
		LoginManager.step = 0;
		CreateManager.step = 0;
		LoginManager.mapFlagX = 0;
		FogManager.fades = 0;
		Math.script.clear();
		Math.next = 0;
	}

	static Npc npc(int slot, NpcType type, int tileX, int tileY) {
		Npc npc = new Npc();
		npc.setNpcType(type);
		npc.setSize(type.size);
		npc.teleport(npc.getSize(), tileX, tileY, true);
		NpcList.npcs[slot] = npc;
		NpcList.ids[NpcList.size++] = slot;
		return npc;
	}

	static String npcs() {
		StringBuilder text = new StringBuilder();
		for (int i = 0; i < NpcList.size; i++) {
			Npc npc = NpcList.npcs[NpcList.ids[i]];
			text.append(i == 0 ? "" : ";").append(NpcList.ids[i]).append(':').append(npc.xFine).append(':').append(npc.yFine).append(':').append(npc.movementQueueX[0]).append(':').append(npc.movementQueueY[0]);
		}
		return text.length() == 0 ? "-" : text.toString();
	}

	// An absolute tile in or near a region with an NPC file, or anywhere.
	static int[] tile() {
		if (random.nextInt(10) < 8) {
			int[] region = TITLE[random.nextInt(TITLE.length)];
			return new int[]{region[0] * 64 + range(-20, 83), region[1] * 64 + range(-20, 83)};
		}
		return new int[]{range(100, 16000), range(100, 16000)};
	}

	static void region() {
		loggedOut();
		int[] at = tile();
		for (int call = 0; call < 3; call++) {
			// The camera, scene-local as the original keeps it.
			if (call > 0) {
				if (random.nextInt(4) == 0) {
					at = tile();
				} else {
					at = new int[]{at[0] + range(-70, 70), at[1] + range(-70, 70)};
				}
			}
			Camera.renderX = (at[0] - Camera.originX) * 128 + range(0, 127);
			Camera.renderY = (at[1] - Camera.originY) * 128 + range(0, 127);
			Camera.cameraType = range(2, 3);
			Camera.lockedTargetX = range(-200, 300);
			Camera.lockedTargetY = range(-200, 300);
			Camera.lockedLookAtX = range(-200, 300);
			Camera.lockedLookAtY = range(-200, 300);
			if (call == 0 || random.nextInt(3) == 0) {
				int count = range(0, 5);
				for (int i = 0; i < count; i++) {
					int slot = range(0, 500);
					if (NpcList.npcs[slot] == null) {
						Npc npc = npc(slot, new NpcType(), range(-130, 230), range(-130, 230));
						npc.xFine += range(-64, 63);
						npc.yFine += range(-64, 63);
						if (random.nextInt(3) == 0) {
							// On or just outside an edge of the scene this
							// call is expected to set up.
							int[] edges = {-1, 0, 13184, 13185};
							int nextX = ((Camera.renderX >> 10) + (Camera.originX >> 3) - 6) * 8;
							int nextY = ((Camera.renderY >> 10) + (Camera.originY >> 3)) * 8 - 48;
							if (random.nextBoolean()) {
								npc.xFine = (nextX - Camera.originX) * 128 + edges[random.nextInt(4)];
								npc.yFine = (nextY - Camera.originY) * 128 + range(0, 13184);
							} else {
								npc.xFine = (nextX - Camera.originX) * 128 + range(0, 13184);
								npc.yFine = (nextY - Camera.originY) * 128 + edges[random.nextInt(4)];
							}
						}
					}
				}
			}
			FogManager.fades = 0;
			client.gameState = 10;
			out.print("region origin=" + Camera.originX + "," + Camera.originY + " central=" + SceneGraph.centralZoneX + "," + SceneGraph.centralZoneY + " render=" + Camera.renderX + "," + Camera.renderY + " type=" + Camera.cameraType + " target=" + Camera.lockedTargetX + "," + Camera.lockedTargetY + " look=" + Camera.lockedLookAtX + "," + Camera.lockedLookAtY + " npcs=" + npcs());
			LoginManager.setupLoadingScreenRegion();
			StringBuilder packed = new StringBuilder();
			StringBuilder files = new StringBuilder();
			for (int i = 0; i < 18; i++) {
				packed.append(i == 0 ? "" : ",").append(LoginManager.regionBitPacked[i]);
				// Bit 0: the NPC file; 1: the terrain; 2: the scenery; 3 and 4:
				// the two underwater files.
				files.append((LoginManager.npcSpawnsFileIds[i] != -1 ? 1 : 0) + (LoginManager.mapFileIds[i] != -1 ? 2 : 0) + (LoginManager.locationsMapFileIds[i] != -1 ? 4 : 0) + (LoginManager.underWaterMapFileIds[i] != -1 ? 8 : 0) + (LoginManager.underWaterLocationsMapFileIds[i] != -1 ? 16 : 0)).append(i == 17 ? "" : ",");
			}
			out.print(" -> state=" + client.gameState + " packed=" + packed + " files=" + files + " origin=" + Camera.originX + "," + Camera.originY + " central=" + SceneGraph.centralZoneX + "," + SceneGraph.centralZoneY + " render=" + Camera.renderX + "," + Camera.renderY + " type=" + Camera.cameraType + " target=" + Camera.lockedTargetX + "," + Camera.lockedTargetY + " look=" + Camera.lockedLookAtX + "," + Camera.lockedLookAtY + " npcs=" + npcs() + " fades=" + FogManager.fades + " self=" + PlayerList.self.movementQueueX[0] + "," + PlayerList.self.movementQueueY[0] + " plane=" + Player.plane + "\n");
		}
	}

	static String hex(byte[] data) {
		StringBuilder text = new StringBuilder();
		for (byte b : data) {
			text.append(String.format("%02x", b & 0xFF));
		}
		return text.length() == 0 ? "-" : text.toString();
	}

	static void spawn() {
		loggedOut();
		int[] region = TITLE[random.nextInt(TITLE.length)];
		// A scene origin from which part of the region is inside the scene.
		Camera.originX = region[0] * 64 + range(-70, 40);
		Camera.originY = region[1] * 64 + range(-70, 40);
		NpcTypeList.types.clear();
		StringBuilder types = new StringBuilder();
		int typeCount = range(1, 6);
		for (int i = 0; i < typeCount; i++) {
			NpcType type = new NpcType();
			type.loginscreenproperties = (byte) (random.nextInt(8) == 0 ? -1 : range(0, 3));
			type.size = range(1, 3);
			type.spawndirection = (byte) range(0, 7);
			type.rotationspeed = random.nextInt(4) == 0 ? 0 : range(1, 64);
			type.bastypeid = range(-1, 40);
			int id = range(0, 9000);
			NpcTypeList.types.put(id, type);
			types.append(i == 0 ? "" : ";").append(id).append(':').append(type.loginscreenproperties).append(':').append(type.size).append(':').append(type.spawndirection).append(':').append(type.rotationspeed).append(':').append(type.bastypeid);
		}
		Integer[] ids = NpcTypeList.types.keySet().toArray(new Integer[0]);
		// The slot table as earlier scenes left it.
		LoginManager.decodedNpcRegionCount = range(0, 5);
		StringBuilder slots = new StringBuilder();
		for (int i = 0; i < LoginManager.decodedNpcRegionCount; i++) {
			int[] other = TITLE[random.nextInt(TITLE.length)];
			LoginManager.decodedNpcRegions[i] = random.nextInt(3) == 0 && i == 0 ? (region[0] << 8) + region[1] : (other[0] << 8) + other[1] + 100000 * (i + 1);
			slots.append(i == 0 ? "" : ",").append(LoginManager.decodedNpcRegions[i]);
		}
		LoginManager.npcSpawnsFilesBuffer = new byte[18][];
		LoginManager.regionBitPacked = new int[18];
		StringBuilder files = new StringBuilder();
		int fileCount = range(1, 2);
		for (int file = 0; file < fileCount; file++) {
			int index = file * 3 + range(0, 2);
			int packedRegion = ((region[0] + file) << 8) + region[1];
			int records = range(0, 24);
			byte[] data = new byte[records * 4];
			for (int i = 0; i < records; i++) {
				int position = (random.nextInt(5) == 0 ? range(0, 3) : 0) << 14 | range(0, 63) << 7 | range(0, 63);
				int id = random.nextInt(6) == 0 ? range(0, 9000) : ids[random.nextInt(ids.length)];
				data[i * 4] = (byte) (position >> 8);
				data[i * 4 + 1] = (byte) position;
				data[i * 4 + 2] = (byte) (id >> 8);
				data[i * 4 + 3] = (byte) id;
			}
			LoginManager.npcSpawnsFilesBuffer[index] = data;
			LoginManager.regionBitPacked[index] = packedRegion;
			files.append(file == 0 ? "" : ";").append(packedRegion).append(':').append(hex(data));
		}
		// NPCs earlier scenes left in place keep their indices.
		StringBuilder occupied = new StringBuilder();
		int kept = range(0, 4);
		for (int i = 0; i < kept; i++) {
			int slot = range(0, 6) << 6 | range(0, 5);
			if (NpcList.npcs[slot] == null) {
				npc(slot, new NpcType(), 50, 50);
				occupied.append(occupied.length() == 0 ? "" : ",").append(slot);
			}
		}
		int before = NpcList.size;
		client.loop = range(0, 100000);
		out.print("spawn origin=" + Camera.originX + "," + Camera.originY + " slots=" + (slots.length() == 0 ? "-" : slots) + " occupied=" + (occupied.length() == 0 ? "-" : occupied) + " types=" + types + " files=" + files);
		LoginManager.decodeNpcFiles();
		StringBuilder placed = new StringBuilder();
		for (int i = before; i < NpcList.size; i++) {
			int slot = NpcList.ids[i];
			Npc npc = NpcList.npcs[slot];
			int type = -1;
			for (Integer id : ids) {
				if (NpcTypeList.types.get(id) == npc.type) {
					type = id;
				}
			}
			placed.append(i == before ? "" : ";").append(slot).append(':').append(type).append(':').append(npc.xFine).append(':').append(npc.yFine).append(':').append(npc.movementQueueX[0]).append(':').append(npc.movementQueueY[0]).append(':').append(npc.targetAngle).append(':').append(npc.currentAngle).append(':').append(npc.turnSpeed).append(':').append(npc.basTypeId).append(':').append(npc.getSize()).append(':').append(npc.lastSeenLoop == client.loop ? 1 : 0);
		}
		out.print(" -> count=" + LoginManager.decodedNpcRegionCount + " placed=" + (placed.length() == 0 ? "-" : placed) + "\n");
	}

	static int hash(int hash, int value) {
		return hash * 31 + value;
	}

	static void wander() {
		loggedOut();
		// The camera stands still: only the NPC pass runs.
		LoginManager.step = 1;
		Player.plane = 0;
		CollisionMap map = new CollisionMap(104, 104);
		PathFinder.collisionMaps[0] = map;
		int[] bits = {0x100, 0x200000, 0x40000, 0x20000, 1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x1000000};
		StringBuilder flags = new StringBuilder();
		// A small arena, so that the NPCs meet the obstacles and each other.
		int low = range(0, 60);
		int high = low + range(8, 40);
		// Reading a region's terrain clears the "not loaded" flag of its
		// tiles; here a rectangle of them, usually the whole arena.
		int[] loaded = random.nextInt(5) == 0 ? new int[]{range(1, 50), range(1, 50), range(51, 98), range(51, 98)} : new int[]{1, 1, 98, 98};
		for (int x = loaded[0]; x <= loaded[2]; x++) {
			for (int y = loaded[1]; y <= loaded[3]; y++) {
				map.flags[x][y] &= ~0x1000000;
			}
		}
		int marks = range(0, 120);
		for (int i = 0; i < marks; i++) {
			int x = range(low, high);
			int y = range(low, high);
			int bit = bits[random.nextInt(bits.length)];
			map.flags[x][y] |= bit;
			flags.append(i == 0 ? "" : ";").append(x).append(':').append(y).append(':').append(bit);
		}
		StringBuilder npcs = new StringBuilder();
		int count = range(1, 7);
		for (int i = 0; i < count; i++) {
			int slot = range(0, 3000);
			if (NpcList.npcs[slot] != null) {
				continue;
			}
			NpcType type = new NpcType();
			type.size = random.nextBoolean() ? range(2, 3) : 1;
			type.loginscreenproperties = (byte) (random.nextInt(6) == 0 ? range(0, 1) : range(2, 3));
			boolean edge = random.nextInt(8) == 0;
			int x = edge ? (random.nextBoolean() ? range(0, 2) : range(100, 104 - type.size)) : range(low, high);
			int y = edge ? (random.nextBoolean() ? range(0, 2) : range(100, 104 - type.size)) : range(low, high);
			npc(slot, type, x, y);
			npcs.append(npcs.length() == 0 ? "" : ";").append(slot).append(':').append(x).append(':').append(y).append(':').append(type.size).append(':').append(type.loginscreenproperties);
		}
		for (int i = 0; i < 6000; i++) {
			double value = random.nextDouble();
			Math.script.add(random.nextBoolean() ? value : value * 0.02);
		}
		int cycles = range(20, 80);
		StringBuilder hashes = new StringBuilder();
		for (int cycle = 0; cycle < cycles; cycle++) {
			ProbeClient.mainUpdate();
			int hash = 17;
			for (int n = 0; n < 32768; n++) {
				Npc npc = NpcList.npcs[n];
				if (npc != null) {
					hash = hash(hash(hash(hash(hash(hash, npc.movementQueueX[0]), npc.movementQueueY[0]), npc.movementQueueSize), npc.xFine), npc.yFine);
				}
			}
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					hash = hash(hash, map.flags[x][y]);
				}
			}
			hashes.append(cycle == 0 ? "" : ",").append(hash);
		}
		StringBuilder randoms = new StringBuilder();
		for (int i = 0; i < Math.next; i++) {
			randoms.append(i == 0 ? "" : ",").append(Long.toHexString(Double.doubleToLongBits(Math.script.get(i))));
		}
		out.print("wander loaded=" + loaded[0] + "," + loaded[1] + "," + loaded[2] + "," + loaded[3] + " flags=" + (flags.length() == 0 ? "-" : flags) + " npcs=" + (npcs.length() == 0 ? "-" : npcs) + " cycles=" + cycles + " randoms=" + (randoms.length() == 0 ? "-" : randoms) + " -> hashes=" + hashes + " npcs=" + npcs() + "\n");
	}

	static String pose() {
		return Camera.renderX + ":" + Camera.renderY + ":" + Camera.renderZ + ":" + Camera.cameraPitch + ":" + Camera.cameraYaw + ":" + client.gameState + ":" + Camera.originX + ":" + Camera.originY;
	}

	// client.mainUpdate's cycles; a scene set up again is built at once
	// (LoginManager.rebuildMap ends game state 28 with 10).
	static String cycles(int steps, boolean spline, StringBuilder busy) {
		StringBuilder text = new StringBuilder();
		for (int step = 0; step < steps; step++) {
			LoginManager.step = random.nextInt(9) == 0 ? 1 : 0;
			CreateManager.step = random.nextInt(40) == 0 ? 1 : 0;
			busy.append(LoginManager.step != 0 || CreateManager.step != 0 ? '1' : '0');
			ProbeClient.mainUpdate();
			text.append(step == 0 ? "" : ";").append(pose());
			if (spline) {
				text.append(':').append(Float.floatToIntBits(Camera.pitchRadians)).append(':').append(Float.floatToIntBits(Camera.yawRadians)).append(':').append(Camera.splineProgress).append(':').append(Camera.splineFinished ? 1 : 0).append(':').append(Camera.splineJustFinished ? 1 : 0);
			}
			client.gameState = 10;
		}
		return text.toString();
	}

	static void spline() {
		loggedOut();
		int[] start = tile();
		StringBuilder paths = new StringBuilder();
		for (int path = 0; path < 2; path++) {
			int points = range(2, 6);
			// CS2 5405 and 5406 (ScriptRunner.java:4743-4766), by hand.
			Camera.cameraPathData[path] = new int[points << 1][4];
			int[] at = {start[0], start[1]};
			paths.append(path == 0 ? " path0=" : " path1=");
			for (int point = 0; point < points; point++) {
				at[0] += range(-75, 75);
				at[1] += range(-75, 75);
				int[] control = {at[0] + range(-12, 12), at[1] + range(-12, 12)};
				int height = range(-200, 2500);
				int controlHeight = range(-200, 2500);
				Camera.cameraPathData[path][point * 2] = new int[]{(at[0] & 0x3FFF) * 128, height, (at[1] & 0x3FFF) * 128, range(0, 9)};
				Camera.cameraPathData[path][point * 2 + 1] = new int[]{(control[0] & 0x3FFF) * 128, controlHeight, (control[1] & 0x3FFF) * 128};
				paths.append(point == 0 ? "" : ";").append((at[0] & 0x3FFF) * 128).append(':').append(height).append(':').append((at[1] & 0x3FFF) * 128).append(';').append((control[0] & 0x3FFF) * 128).append(':').append(controlHeight).append(':').append((control[1] & 0x3FFF) * 128);
			}
		}
		// CS2 5502 (ScriptRunner.java:4899-4926), by hand.
		Camera.movePathId = range(0, 1);
		Camera.moveSplineIndex = range(0, (Camera.cameraPathData[Camera.movePathId].length >> 1) - 2);
		Camera.splineProgress = 0;
		Camera.moveSplineSpeedStart = range(1, 1200);
		Camera.moveSplineSpeedEnd = range(1, 1200);
		Camera.lookAtPathId = range(0, 1);
		Camera.lookAtSplineIndex = range(0, (Camera.cameraPathData[Camera.lookAtPathId].length >> 1) - 2);
		Camera.cameraType = 3;
		out.print("spline" + paths + " move=" + Camera.movePathId + "," + Camera.moveSplineIndex + "," + Camera.moveSplineSpeedStart + "," + Camera.moveSplineSpeedEnd + " look=" + Camera.lookAtPathId + "," + Camera.lookAtSplineIndex);
		// InterfaceList.resetToLoginScreen (1113-1121), by hand.
		Camera.updateLoginScreenCamera();
		LoginManager.setupLoadingScreenRegion();
		String first = pose();
		client.gameState = 10;
		StringBuilder busy = new StringBuilder();
		String steps = cycles(range(30, 160), true, busy);
		out.print(" busy=" + busy + " -> first=" + first + " steps=" + steps + "\n");
	}

	static void locked() {
		loggedOut();
		// A scene already set up around some zone.
		int[] middle = tile();
		SceneGraph.centralZoneX = middle[0] >> 3;
		SceneGraph.centralZoneY = middle[1] >> 3;
		Camera.originX = (SceneGraph.centralZoneX - 6) * 8;
		Camera.originY = SceneGraph.centralZoneY * 8 - 48;
		Camera.renderX = range(14 * 128, 90 * 128 - 1);
		Camera.renderY = range(14 * 128, 90 * 128 - 1);
		Camera.renderZ = range(-3000, -100);
		Camera.cameraPitch = range(128, 383);
		Camera.cameraYaw = range(0, 2047);
		int[] move = {range(0, 103), range(0, 103), range(0, 1500), range(0, 20), random.nextInt(4) == 0 ? range(100, 120) : range(0, 99)};
		int[] look = {range(0, 103), range(0, 103), range(0, 600), range(0, 20), random.nextInt(4) == 0 ? range(100, 120) : range(0, 99)};
		out.print("locked origin=" + Camera.originX + "," + Camera.originY + " central=" + SceneGraph.centralZoneX + "," + SceneGraph.centralZoneY + " pose=" + Camera.renderX + "," + Camera.renderY + "," + Camera.renderZ + "," + Camera.cameraPitch + "," + Camera.cameraYaw + " move=" + move[0] + "," + move[1] + "," + move[2] + "," + move[3] + "," + move[4] + " look=" + look[0] + "," + look[1] + "," + look[2] + "," + look[3] + "," + look[4]);
		// CS2 5500 and 5501 (ScriptRunner.java:4881-4898): the unchanged
		// Camera.setLockedPosition and setLockedLookAt.
		Camera.setLockedPosition(false, move[3], move[2], move[4], move[1], move[0]);
		Camera.setLockedLookAt(look[2], look[1], look[3], look[0], look[4]);
		StringBuilder busy = new StringBuilder();
		String steps = cycles(range(20, 120), false, busy);
		out.print(" busy=" + busy + " -> steps=" + steps + "\n");
	}

	public static void main(String[] args) throws Exception {
		random = new Random(Long.parseLong(args[0]));
		out = new PrintWriter(args[5], "UTF-8");
		for (int[] region : TITLE) {
			client.js5Archive5.names.add("n" + region[0] + "_" + region[1]);
		}
		// Every region has terrain and scenery. A third of them have an
		// underwater map and a quarter underwater locations, by fixed rules
		// the test repeats, so that a listed region is asked for whichever
		// of the two it has and an unlisted one for neither.
		for (int x = 0; x < 256; x++) {
			for (int y = 0; y < 256; y++) {
				client.js5Archive5.names.add("m" + x + "_" + y);
				client.js5Archive5.names.add("l" + x + "_" + y);
				if ((x + y) % 3 == 0) {
					client.js5Archive5.names.add("um" + x + "_" + y);
				}
				if ((x * 7 + y) % 4 == 0) {
					client.js5Archive5.names.add("ul" + x + "_" + y);
				}
			}
		}
		for (int i = 0; i < 4; i++) {
			PathFinder.collisionMaps[i] = new CollisionMap(104, 104);
		}
		for (int i = Integer.parseInt(args[1]); i > 0; i--) {
			region();
		}
		for (int i = Integer.parseInt(args[2]); i > 0; i--) {
			spawn();
		}
		for (int i = Integer.parseInt(args[3]); i > 0; i--) {
			wander();
		}
		for (int i = Integer.parseInt(args[4]); i > 0; i--) {
			spline();
			locked();
		}
		out.close();
	}
}
