import java.security.MessageDigest;
import java.util.Random;
import rt4.Buffer;
import rt4.Camera;
import rt4.GameShell;
import rt4.GlRaster;
import rt4.InterfaceList;
import rt4.Keyboard;
import rt4.Player;
import rt4.PlayerList;
import rt4.Preferences;
import rt4.Protocol;
import rt4.SceneGraph;
import rt4.ScriptRunner;

/**
 * Runs the unchanged rt4.Camera (with rt4.MathUtils) and the copied pieces in
 * the adapters over seeded inputs and prints one JSON object per case. Every
 * input is in the output, so the Rust test replays them without this
 * generator's random numbers; only the scene arrays are derived from a seed,
 * by mix(), which the test repeats.
 */
public final class CameraOracle {
	private static final StringBuilder out = new StringBuilder();

	static int mix(int seed, int a, int b, int c) {
		int h = seed ^ (a * 0x9E3779B1);
		h = (h ^ (h >>> 15)) * 0x85EBCA6B + b * 0xC2B2AE35;
		h ^= h >>> 13;
		h = h * 0x27D4EB2F + c * 0x165667B1;
		h ^= h >>> 16;
		return h;
	}

	/** Heights fall 240 a level and vary by `mask`; one tile in eleven is a bridge. */
	static void scene(int seed, int mask, boolean occlusion) {
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 105; x++) {
				for (int y = 0; y < 105; y++) {
					SceneGraph.tileHeights[level][x][y] = -(mix(seed, level, x, y) & mask) - level * 240;
					if (x < 104 && y < 104) {
						SceneGraph.renderFlags[level][x][y] = (byte) (level == 1 && (mix(seed ^ 0x55, 1, x, y) & 0x7FFFFFFF) % 11 == 0 ? 2 : 0);
						SceneGraph.occlusionHeights[level][x][y] = occlusion ? (byte) mix(seed ^ 0xAA, level, x >> 2, y >> 2) : 0;
					}
				}
			}
		}
	}

	static String state() {
		return "[" + Double.doubleToRawLongBits(Camera.pitchTarget) + "," + Double.doubleToRawLongBits(Camera.yawTarget) + "," + Camera.cameraX + "," + Camera.cameraY + "," + Camera.maxPitchDistance + "," + Camera.cameraOffsetX + "," + Camera.cameraOffsetY + "," + Camera.offsetXDelta + "," + Protocol.cameraYOffsetDelta + "," + Camera.yawDrift + "," + Protocol.yawDriftDelta + "," + Protocol.cameraJitterTimer + "]";
	}

	static double angle(Random r) {
		switch (r.nextInt(6)) {
			case 0:
				return r.nextInt(4096) - 1024;
			case 1:
				return r.nextDouble() * 2048.0;
			case 2:
				return -r.nextDouble() * 5000.0;
			case 3:
				return 2047.0 - r.nextInt(3);
			case 4:
				return r.nextInt(600);
			default:
				return r.nextDouble() * 100000.0 - 30000.0;
		}
	}

	static void follow(Random r, int index) {
		int seed = r.nextInt();
		int mask = new int[]{0, 0x3F, 0xFF, 0x3FF, 0x7FF}[r.nextInt(5)];
		boolean occlusion = r.nextInt(3) == 0;
		scene(seed, mask, occlusion);
		Player.plane = r.nextInt(4);
		Camera.cameraType = 1;
		Camera.pitchTarget = r.nextBoolean() ? 128 : angle(r);
		Camera.yawTarget = r.nextBoolean() ? 0 : angle(r);
		int px = 600 + r.nextInt(12000);
		int py = 600 + r.nextInt(12000);
		PlayerList.self.xFine = px;
		PlayerList.self.yFine = py;
		Camera.cameraX = r.nextBoolean() ? px : r.nextInt(13312);
		Camera.cameraY = r.nextBoolean() ? py : r.nextInt(13312);
		Camera.cameraOffsetX = r.nextInt(120) - 60;
		Camera.cameraOffsetY = r.nextInt(130) - 65;
		Camera.offsetXDelta = r.nextBoolean() ? 2 : -2;
		Protocol.cameraYOffsetDelta = r.nextBoolean() ? 2 : -2;
		Camera.yawDrift = r.nextInt(90) - 45;
		Protocol.yawDriftDelta = r.nextBoolean() ? 1 : -1;
		Camera.maxPitchDistance = r.nextBoolean() ? 0 : r.nextInt(100000);
		Protocol.cameraJitterTimer = r.nextBoolean() ? 0 : 490 + r.nextInt(11);
		Camera.ZOOM = r.nextInt(4) == 0 ? 1 + r.nextInt(2000) : 600;
		for (int i = 0; i < 5; i++) {
			Camera.customCameraActive[i] = false;
		}
		GameShell shell = new GameShell();
		out.append("{\"case\":\"follow\",\"index\":").append(index).append(",\"seed\":").append(seed).append(",\"mask\":").append(mask).append(",\"occlusion\":").append(occlusion).append(",\"plane\":").append(Player.plane).append(",\"zoom\":").append(Camera.ZOOM).append(",\"player\":[").append(px).append(",").append(py).append("],\"start\":").append(state()).append(",\"steps\":[");
		int steps = 40 + r.nextInt(40);
		for (int step = 0; step < steps; step++) {
			if (step > 0) {
				out.append(',');
			}
			int kind = r.nextInt(20);
			if (kind < 8) {
				// One 20 ms cycle: the player walks, runs or teleports.
				int move = r.nextInt(12);
				if (move == 0) {
					px = r.nextInt(13312);
					py = r.nextInt(13312);
				} else if (move == 1) {
					px += r.nextInt(1100) - 550;
					py += r.nextInt(1100) - 550;
				} else {
					px += r.nextInt(33) - 16;
					py += r.nextInt(33) - 16;
				}
				PlayerList.self.xFine = px;
				PlayerList.self.yFine = py;
				Preferences.keyboardCameraEnabled = r.nextInt(3) == 0;
				InterfaceList.keyQueueSize = r.nextInt(4) == 0 ? r.nextInt(6) : 0;
				StringBuilder keys = new StringBuilder("[");
				for (int i = 0; i < InterfaceList.keyQueueSize; i++) {
					InterfaceList.keyCodes[i] = r.nextInt(5) == 0 ? r.nextInt(112) : 96 + r.nextInt(4);
					keys.append(i > 0 ? "," : "").append(InterfaceList.keyCodes[i]);
				}
				keys.append("]");
				Camera.updatePlayerCamera();
				out.append("{\"op\":\"cycle\",\"player\":[").append(px).append(',').append(py).append("],\"keyboard\":").append(Preferences.keyboardCameraEnabled).append(",\"keys\":").append(keys);
			} else if (kind < 12) {
				GameShell.renderDelta = r.nextInt(5) == 0 ? r.nextLong() >> r.nextInt(40) : 1_000_000L + r.nextInt(40_000_000);
				boolean[] held = new boolean[4];
				for (int i = 0; i < 4; i++) {
					held[i] = r.nextInt(3) == 0;
				}
				Keyboard.pressedKeys[Keyboard.KEY_UP] = held[0];
				Keyboard.pressedKeys[Keyboard.KEY_DOWN] = held[1];
				Keyboard.pressedKeys[Keyboard.KEY_LEFT] = held[2];
				Keyboard.pressedKeys[Keyboard.KEY_RIGHT] = held[3];
				shell.mainInputLoop();
				out.append("{\"op\":\"keys\",\"delta\":").append(GameShell.renderDelta).append(",\"held\":[").append(held[0]).append(',').append(held[1]).append(',').append(held[2]).append(',').append(held[3]).append("]");
			} else if (kind < 14) {
				// CAM_FORCEANGLE and CS2 5504 assign integers and clamp.
				int pitch = r.nextInt(3) == 0 ? r.nextInt(65536) : r.nextInt(600);
				int yaw = r.nextInt(3) == 0 ? r.nextInt(65536) : r.nextInt(2048);
				Camera.pitchTarget = pitch;
				Camera.yawTarget = yaw;
				Camera.clampCameraAngle();
				out.append("{\"op\":\"angles\",\"pitch\":").append(pitch).append(",\"yaw\":").append(yaw);
			} else if (kind < 16) {
				double random = r.nextDouble();
				Protocol.cameraDrift(random);
				out.append("{\"op\":\"drift\",\"random\":").append(Double.doubleToRawLongBits(random));
			} else if (kind < 17) {
				Player.plane = r.nextInt(4);
				out.append("{\"op\":\"plane\",\"plane\":").append(Player.plane);
			} else {
				int height = r.nextInt(4) == 0 ? r.nextInt(2000) - 200 : 300 + r.nextInt(200);
				Camera.customCameraActive[4] = r.nextInt(4) == 0;
				Camera.cameraAmplitude[4] = r.nextInt(300);
				if (r.nextInt(5) == 0) {
					ScriptRunner.minViewDistance = (short) (1 + r.nextInt(600));
					ScriptRunner.maxViewDistance = (short) (1 + r.nextInt(600));
				} else {
					ScriptRunner.minViewDistance = 256;
					ScriptRunner.maxViewDistance = 320;
				}
				ScriptRunner.followCamera(height);
				out.append("{\"op\":\"render\",\"height\":").append(height).append(",\"shake\":").append(Camera.customCameraActive[4]).append(",\"amplitude\":").append(Camera.cameraAmplitude[4]).append(",\"distance\":[").append(ScriptRunner.minViewDistance).append(',').append(ScriptRunner.maxViewDistance).append("],\"pose\":[").append(Camera.renderX).append(',').append(Camera.renderY).append(',').append(Camera.renderZ).append(',').append(Camera.cameraPitch).append(',').append(Camera.cameraYaw).append("]");
			}
			out.append(",\"state\":").append(state()).append('}');
		}
		out.append("]}\n");
	}

	static short limit(Random r, int usual) {
		switch (r.nextInt(8)) {
			case 0:
				return (short) (1 + r.nextInt(32767));
			case 1:
				return (short) (1 + r.nextInt(64));
			case 2:
				return (short) (100 + r.nextInt(600));
			default:
				return (short) usual;
		}
	}

	static void viewport(Random r, int index) {
		ScriptRunner.nearZoom = limit(r, 256);
		ScriptRunner.farZoom = limit(r, 205);
		ScriptRunner.minZoom = limit(r, 1);
		ScriptRunner.maxZoom = limit(r, 32767);
		if (ScriptRunner.maxZoom < ScriptRunner.minZoom) {
			ScriptRunner.maxZoom = ScriptRunner.minZoom;
		}
		ScriptRunner.minFov = limit(r, 1);
		ScriptRunner.maxFov = limit(r, 32767);
		if (ScriptRunner.maxFov < ScriptRunner.minFov) {
			ScriptRunner.maxFov = ScriptRunner.minFov;
		}
		int width = r.nextInt(6) == 0 ? r.nextInt(70000) - 10 : 1 + r.nextInt(2600);
		int height = r.nextInt(6) == 0 ? r.nextInt(70000) - 10 : 1 + r.nextInt(1500);
		if (index < 4) {
			int[][] usual = {{512, 334}, {765, 503}, {1280, 800}, {1920, 1080}};
			width = usual[index][0];
			height = usual[index][1];
			ScriptRunner.nearZoom = 256;
			ScriptRunner.farZoom = 205;
			ScriptRunner.minZoom = 1;
			ScriptRunner.maxZoom = 32767;
			ScriptRunner.minFov = 1;
			ScriptRunner.maxFov = 32767;
		}
		int left = r.nextInt(300) - 20;
		int top = r.nextInt(300) - 20;
		GlRaster.fills.clear();
		out.append("{\"case\":\"viewport\",\"index\":").append(index).append(",\"rect\":[").append(left).append(',').append(top).append(',').append(width).append(',').append(height).append("],\"limits\":[").append(ScriptRunner.nearZoom).append(',').append(ScriptRunner.farZoom).append(',').append(ScriptRunner.minZoom).append(',').append(ScriptRunner.maxZoom).append(',').append(ScriptRunner.minFov).append(',').append(ScriptRunner.maxFov).append("]");
		try {
			ScriptRunner.calculateViewportBounds(width, top, height, left, true);
		} catch (ArithmeticException ex) {
			out.append(",\"throws\":true}\n");
			return;
		}
		out.append(",\"bounds\":[").append(ScriptRunner.viewportLeft).append(',').append(ScriptRunner.viewportTop).append(',').append(ScriptRunner.viewportWidth).append(',').append(ScriptRunner.viewportHeight).append(',').append(ScriptRunner.viewportZoom).append("],\"bars\":[");
		for (int i = 0; i < GlRaster.fills.size(); i++) {
			int[] f = GlRaster.fills.get(i);
			out.append(i > 0 ? "," : "").append('[').append(f[0]).append(',').append(f[1]).append(',').append(f[2]).append(',').append(f[3]).append(']');
		}
		out.append("]");
		// renderGameScene's call of setupPerspectiveView (line 282).
		left = ScriptRunner.viewportLeft;
		top = ScriptRunner.viewportTop;
		width = ScriptRunner.viewportWidth;
		height = ScriptRunner.viewportHeight;
		try {
			ScriptRunner.perspectiveEdges(left, top, width, height, width / 2 + left, top - -(height / 2), ScriptRunner.viewportZoom, ScriptRunner.viewportZoom);
			out.append(",\"edges\":[").append(ScriptRunner.screenLowerX).append(',').append(ScriptRunner.screenUpperX).append(',').append(ScriptRunner.screenLowerY).append(',').append(ScriptRunner.screenUpperY).append("]");
		} catch (ArithmeticException ex) {
			out.append(",\"edges\":null");
		}
		out.append("}\n");
	}

	static void project(Random r, int index) {
		int seed = r.nextInt();
		int mask = new int[]{0, 0xFF, 0x3FF}[r.nextInt(3)];
		scene(seed, mask, false);
		Player.plane = r.nextInt(4);
		int tx = 1000 + r.nextInt(11000);
		int ty = 1000 + r.nextInt(11000);
		Camera.cameraType = 1;
		Camera.cameraX = tx;
		Camera.cameraY = ty;
		PlayerList.self.xFine = tx;
		PlayerList.self.yFine = ty;
		Camera.pitchTarget = 128 + r.nextInt(256);
		Camera.yawTarget = r.nextInt(2048);
		Camera.yawDrift = 0;
		Camera.maxPitchDistance = 0;
		Camera.ZOOM = 600;
		Camera.customCameraActive[4] = false;
		ScriptRunner.minViewDistance = 256;
		ScriptRunner.maxViewDistance = 320;
		int width = index % 3 == 0 ? 512 : index % 3 == 1 ? 765 : 300 + r.nextInt(1700);
		int height = index % 3 == 0 ? 334 : index % 3 == 1 ? 503 : 200 + r.nextInt(1000);
		ScriptRunner.nearZoom = 256;
		ScriptRunner.farZoom = 205;
		ScriptRunner.minZoom = 1;
		ScriptRunner.maxZoom = 32767;
		ScriptRunner.minFov = 1;
		ScriptRunner.maxFov = 32767;
		ScriptRunner.calculateViewportBounds(width, 0, height, 0, false);
		width = ScriptRunner.viewportWidth;
		height = ScriptRunner.viewportHeight;
		ScriptRunner.followCamera(height);
		if (r.nextInt(4) == 0) {
			// A scripted pose: any position and any angle of the table.
			Camera.renderX = r.nextInt(13312);
			Camera.renderY = r.nextInt(13312);
			Camera.renderZ = -r.nextInt(3000);
			Camera.cameraPitch = r.nextInt(2048);
			Camera.cameraYaw = r.nextInt(2048);
		}
		int zoom = ScriptRunner.viewportZoom;
		out.append("{\"case\":\"project\",\"index\":").append(index).append(",\"seed\":").append(seed).append(",\"mask\":").append(mask).append(",\"plane\":").append(Player.plane).append(",\"view\":[").append(width).append(',').append(height).append(',').append(zoom).append("],\"pose\":[").append(Camera.renderX).append(',').append(Camera.renderY).append(',').append(Camera.renderZ).append(',').append(Camera.cameraPitch).append(',').append(Camera.cameraYaw).append("],\"points\":[");
		for (int i = 0; i < 24; i++) {
			int x, y;
			if (i < 16) {
				x = tx + r.nextInt(2400) - 1200;
				y = ty + r.nextInt(2400) - 1200;
			} else if (i < 20) {
				x = r.nextInt(13400) - 40;
				y = r.nextInt(13400) - 40;
			} else {
				x = new int[]{127, 128, 13056, 13057}[r.nextInt(4)];
				y = new int[]{127, 128, 13056, 13057}[r.nextInt(4)];
			}
			int above = r.nextInt(5) == 0 ? r.nextInt(2000) - 500 : r.nextInt(300);
			// setOverheadScreenCoordinateOffsets (line 1260) as drawOverheads calls it.
			ScriptRunner.projectToScreen(width >> 1, zoom, y, above, height >> 1, x, zoom);
			out.append(i > 0 ? "," : "").append('[').append(x).append(',').append(y).append(',').append(above).append(',').append(ScriptRunner.screenX).append(',').append(ScriptRunner.screenY).append(']');
		}
		out.append("]}\n");
	}

	static void occlusion(Random r, int index) throws Exception {
		int seed = r.nextInt();
		SceneGraph.occlusionHeights = new byte[4][104][104];
		int originX = r.nextInt(64);
		int originY = r.nextInt(64);
		out.append("{\"case\":\"occlusion\",\"index\":").append(index).append(",\"seed\":").append(seed).append(",\"files\":[");
		int file = 0;
		// Six map files in the rebuild packet's order, some absent.
		for (int rx = 0; rx < 3; rx++) {
			for (int ry = 0; ry < 2; ry++) {
				if (r.nextInt(6) == 0) {
					continue;
				}
				int tileX = rx * 64 - originX - (r.nextInt(8) == 0 ? 64 : 0);
				int baseY = ry * 64 - originY - (r.nextInt(8) == 0 ? 64 : 0);
				int records = r.nextInt(3) == 0 ? 0 : 1 + r.nextInt(2);
				java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
				StringBuilder kinds = new StringBuilder("[");
				for (int record = 0; record < records; record++) {
					bytes.write(129);
					kinds.append(record > 0 ? "," : "").append('[');
					for (int level = 0; level < 4; level++) {
						int roll = r.nextInt(10);
						int kind = roll < 4 ? 1 : roll < 6 ? 0 : roll < 9 ? 2 : new int[]{3, -1, 127, -128}[r.nextInt(4)];
						bytes.write(kind);
						kinds.append(level > 0 ? "," : "").append(kind);
						if (kind == 1) {
							for (int k = 0; k < 256; k++) {
								bytes.write(mix(seed, file * 8 + record, level, k));
							}
						}
					}
					kinds.append(']');
				}
				kinds.append(']');
				SceneGraph.readTerrainOcclusion(baseY, tileX, new Buffer(bytes.toByteArray()));
				out.append(file > 0 ? "," : "").append("{\"at\":[").append(tileX).append(',').append(baseY).append("],\"records\":").append(kinds).append('}');
				file++;
			}
		}
		MessageDigest digest = MessageDigest.getInstance("SHA-256");
		int nonzero = 0;
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 104; x++) {
				digest.update(SceneGraph.occlusionHeights[level][x]);
				for (int y = 0; y < 104; y++) {
					if (SceneGraph.occlusionHeights[level][x][y] != 0) {
						nonzero++;
					}
				}
			}
		}
		StringBuilder hex = new StringBuilder();
		for (byte b : digest.digest()) {
			hex.append(String.format("%02x", b));
		}
		out.append("],\"nonzero\":").append(nonzero).append(",\"sha256\":\"").append(hex).append("\"}\n");
	}

	public static void main(String[] args) throws Exception {
		Random r = new Random(Long.parseLong(args[0]));
		int follow = Integer.parseInt(args[1]);
		int viewport = Integer.parseInt(args[2]);
		int project = Integer.parseInt(args[3]);
		int occlusion = Integer.parseInt(args[4]);
		for (int i = 0; i < follow; i++) {
			follow(r, i);
		}
		for (int i = 0; i < viewport; i++) {
			viewport(r, i);
		}
		for (int i = 0; i < project; i++) {
			project(r, i);
		}
		for (int i = 0; i < occlusion; i++) {
			occlusion(r, i);
		}
		java.io.PrintStream stream = new java.io.PrintStream(new java.io.FileOutputStream(java.io.FileDescriptor.out), false, "UTF-8");
		stream.print(out);
		stream.flush();
	}
}