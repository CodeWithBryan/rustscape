package rt4;

import java.io.PrintStream;
import java.util.Random;

// Drives the unchanged RT4 picking code that `cargo xtask picking-oracle`
// extracts into ProbeModel, ProbeGround, ProbeScene and ProbeKey, over seeded
// inputs, and prints one observation per line.
public final class PickingOracle {
	private static final int[] SIDES = {1, 2, 4, 8};
	private static final int[] WALLS = {1, 2, 4, 8, 16, 32, 64, 128};
	private static PrintStream out;

	public static void main(String[] args) throws Exception {
		long seed = Long.parseLong(args[0]);
		out = new PrintStream(new java.io.FileOutputStream(args[5]), false, "US-ASCII");
		StringBuilder sin = new StringBuilder("sin");
		StringBuilder cos = new StringBuilder("cos");
		for (int i = 0; i < 2048; i++) {
			sin.append(' ').append(MathUtils.sin[i]);
			cos.append(' ').append(MathUtils.cos[i]);
		}
		out.println(sin);
		out.println(cos);
		keys();
		Random random = new Random(seed);
		for (int i = 0; i < Integer.parseInt(args[1]); i++) {
			hover(random);
		}
		for (int i = 0; i < Integer.parseInt(args[2]); i++) {
			tile(random);
		}
		for (int i = 0; i < Integer.parseInt(args[3]); i++) {
			frustum(random);
		}
		for (int i = 0; i < Integer.parseInt(args[4]); i++) {
			scene(random);
		}
		out.close();
	}

	private static void keys() {
		int[][] tiles = {{0, 0}, {31, 47}, {103, 103}, {127, 1}};
		int[] ids = {0, 1, 1530, 32767, 65535};
		for (int[] tile : tiles) {
			for (int shape = 0; shape < 23; shape += 3) {
				for (int id : ids) {
					for (int flags = 0; flags < 8; flags++) {
						int rotation = (shape + id + flags) & 3;
						int interactable = flags & 1;
						int support = (flags >> 1 & 1) * (1 + (id & 1));
						boolean animation = (flags & 4) != 0;
						long key = ProbeKey.key(tile[0], tile[1], shape, rotation, id, interactable, support, animation, false);
						out.println("key " + tile[0] + " " + tile[1] + " " + shape + " " + rotation + " " + id + " " + interactable + " " + support + " " + (animation ? 1 : 0) + " " + Long.toHexString(key));
					}
				}
			}
		}
	}

	// The camera axes in (east, down, north), from the same angles the code under test uses.
	private static double[][] axes(int pitch, int yaw) {
		double sp = MathUtils.sin[pitch] / 65536.0;
		double cp = MathUtils.cos[pitch] / 65536.0;
		double sy = MathUtils.sin[yaw] / 65536.0;
		double cy = MathUtils.cos[yaw] / 65536.0;
		return new double[][]{{cy, 0, sy}, {sy * sp, cp, -cy * sp}, {-sy * cp, sp, cy * cp}};
	}

	private static int[] place(double[][] axes, double across, double down, double depth) {
		int[] p = new int[3];
		for (int i = 0; i < 3; i++) {
			p[i] = (int) Math.round(axes[0][i] * across + axes[1][i] * down + axes[2][i] * depth);
		}
		return p;
	}

	private static int[] screen(double[][] axes, double x, double y, double z) {
		double across = axes[0][0] * x + axes[0][1] * y + axes[0][2] * z;
		double down = axes[1][0] * x + axes[1][1] * y + axes[1][2] * z;
		double depth = axes[2][0] * x + axes[2][1] * y + axes[2][2] * z;
		if (Math.abs(depth) < 1.0) {
			depth = 1.0;
		}
		return new int[]{(int) (across * 512.0 / depth), (int) (down * 512.0 / depth)};
	}

	private static void hover(Random r) {
		ProbeModel model = new ProbeModel();
		int vertices = 3 + r.nextInt(8);
		int scale = 20 << r.nextInt(5);
		model.vertexCount = model.uniqueVertexCount = vertices;
		model.vertexX = new int[vertices];
		model.vertexY = new int[vertices];
		model.vertexZ = new int[vertices];
		model.vertexOffsets = new int[vertices + 1];
		model.vertexLookup = new short[vertices];
		for (int i = 0; i < vertices; i++) {
			model.vertexX[i] = r.nextInt(scale * 2 + 1) - scale;
			model.vertexY[i] = r.nextInt(scale * 2 + 1) - scale * 7 / 4;
			model.vertexZ[i] = r.nextInt(scale * 2 + 1) - scale;
			model.vertexOffsets[i] = i;
			model.vertexLookup[i] = (short) (i + 1);
		}
		model.vertexOffsets[vertices] = vertices;
		int faces = 1 + r.nextInt(6);
		model.triangleCount = faces;
		model.triangleVertexA = new short[faces];
		model.triangleVertexB = new short[faces];
		model.triangleVertexC = new short[faces];
		for (int i = 0; i < faces; i++) {
			model.triangleVertexA[i] = (short) r.nextInt(vertices);
			model.triangleVertexB[i] = (short) r.nextInt(vertices);
			model.triangleVertexC[i] = (short) r.nextInt(vertices);
		}
		model.pickable = r.nextInt(3) == 0;
		int pitch = r.nextInt(8) == 0 ? r.nextInt(2048) : 128 + r.nextInt(256);
		int yaw = r.nextInt(2048);
		int modelYaw = r.nextBoolean() ? 0 : r.nextInt(2048);
		double[][] axes = axes(pitch, yaw);
		int kind = r.nextInt(20);
		double depth = kind == 0 ? 6000 + r.nextInt(3000) : kind < 4 ? 20 + r.nextInt(scale + 150) : 200 + r.nextInt(3800);
		double spread = kind == 1 ? 3.0 : 0.45;
		int[] at = place(axes, (r.nextDouble() * 2 - 1) * depth * spread, (r.nextDouble() * 2 - 1) * depth * spread * 0.7, depth);
		int half = r.nextInt(6) == 0 ? 100 + r.nextInt(400) : 256;
		Rasteriser.screenLowerX = -half;
		Rasteriser.screenUpperX = half;
		Rasteriser.screenLowerY = -half * 167 / 256;
		Rasteriser.screenUpperY = half * 167 / 256;
		// Aim at a vertex, turned as the model is, or at the model's origin.
		int target = r.nextInt(vertices + 1);
		double tx = 0;
		double ty = 0;
		double tz = 0;
		if (target < vertices) {
			double s = MathUtils.sin[modelYaw] / 65536.0;
			double c = MathUtils.cos[modelYaw] / 65536.0;
			tx = modelYaw == 0 ? model.vertexX[target] : model.vertexZ[target] * s + model.vertexX[target] * c;
			tz = modelYaw == 0 ? model.vertexZ[target] : model.vertexZ[target] * c - model.vertexX[target] * s;
			ty = model.vertexY[target];
		}
		int[] aim = screen(axes, at[0] + tx, at[1] + ty, at[2] + tz);
		int jitter = new int[]{0, 0, 3, 15, 60}[r.nextInt(5)];
		ProbeModel.pickScreenX = aim[0] + r.nextInt(jitter * 2 + 1) - jitter;
		RawModel.pickScreenY = aim[1] + r.nextInt(jitter * 2 + 1) - jitter;
		RawModel.allowInput = r.nextInt(12) != 0;
		long key = r.nextInt(15) == 0 ? -5L : 7L;
		MiniMenu.pickResultCount = 0;
		model.render(modelYaw, MathUtils.sin[pitch], MathUtils.cos[pitch], MathUtils.sin[yaw], MathUtils.cos[yaw], at[0], at[1], at[2], key, 0, null);
		StringBuilder line = new StringBuilder("hover ");
		line.append(pitch).append(' ').append(yaw).append(' ');
		line.append(Rasteriser.screenLowerX).append(' ').append(Rasteriser.screenLowerY).append(' ');
		line.append(Rasteriser.screenUpperX).append(' ').append(Rasteriser.screenUpperY).append(' ');
		line.append(ProbeModel.pickScreenX).append(' ').append(RawModel.pickScreenY).append(' ');
		line.append(RawModel.allowInput ? 1 : 0).append(' ').append(key > 0 ? 1 : 0).append(' ');
		line.append(modelYaw).append(' ').append(at[0]).append(' ').append(at[1]).append(' ').append(at[2]).append(' ');
		line.append(model.pickable ? 1 : 0).append(' ').append(vertices);
		for (int i = 0; i < vertices; i++) {
			line.append(' ').append(model.vertexX[i]).append(' ').append(model.vertexY[i]).append(' ').append(model.vertexZ[i]);
		}
		line.append(' ').append(faces);
		for (int i = 0; i < faces; i++) {
			line.append(' ').append(model.triangleVertexA[i]).append(' ').append(model.triangleVertexB[i]).append(' ').append(model.triangleVertexC[i]);
		}
		GlBoundingBox b = model.bounds;
		line.append(' ').append(MiniMenu.pickResultCount);
		line.append(' ').append(b.minX).append(' ').append(b.minY).append(' ').append(b.minZ);
		line.append(' ').append(b.maxX).append(' ').append(b.maxY).append(' ').append(b.maxZ).append(' ').append(b.cylinderRadius);
		out.println(line);
	}

	private static void tile(Random r) {
		int pitch = r.nextInt(10) == 0 ? r.nextInt(2048) : 128 + r.nextInt(256);
		int yaw = r.nextInt(2048);
		double[][] axes = axes(pitch, yaw);
		int tileX = 1 + r.nextInt(102);
		int tileY = 1 + r.nextInt(102);
		int[] heights = new int[4];
		boolean level = r.nextInt(3) == 0;
		for (int i = 0; i < 4; i++) {
			heights[i] = level ? -64 : -r.nextInt(240);
		}
		double depth = r.nextInt(12) == 0 ? 30 + r.nextInt(120) : 300 + r.nextInt(2700);
		int[] back = place(axes, (r.nextDouble() * 2 - 1) * depth * 0.4, (r.nextDouble() * 2 - 1) * depth * 0.3, depth);
		int cameraX = tileX * 128 + 64 - back[0];
		int cameraZ = -64 - back[1];
		int cameraY = tileY * 128 + 64 - back[2];
		// A point on or just around the tile.
		int px = r.nextInt(208) - 40;
		int py = r.nextInt(208) - 40;
		double fx = Math.max(0, Math.min(128, px)) / 128.0;
		double fy = Math.max(0, Math.min(128, py)) / 128.0;
		double height = (heights[0] * (1 - fx) + heights[1] * fx) * (1 - fy) + (heights[3] * (1 - fx) + heights[2] * fx) * fy;
		int[] aim = screen(axes, tileX * 128 + px - cameraX, height - cameraZ, tileY * 128 + py - cameraY);
		int jitter = r.nextInt(3) == 0 ? 2 : 0;
		aim[0] += r.nextInt(jitter * 2 + 1) - jitter;
		aim[1] += r.nextInt(jitter * 2 + 1) - jitter;
		ProbeGround.cameraX = cameraX;
		ProbeGround.cameraY = cameraY;
		ProbeGround.cameraZ = cameraZ;
		ProbeGround.tileHeights = new int[1][105][105];
		ProbeGround.tileHeights[0][tileX][tileY] = heights[0];
		ProbeGround.tileHeights[0][tileX + 1][tileY] = heights[1];
		ProbeGround.tileHeights[0][tileX + 1][tileY + 1] = heights[2];
		ProbeGround.tileHeights[0][tileX][tileY + 1] = heights[3];
		Rasteriser.centerX = r.nextInt(3) == 0 ? 256 : 0;
		Rasteriser.centerY = r.nextInt(3) == 0 ? 167 : 0;
		MiniMenu.walkPending = true;
		MiniMenu.walkDestTileX = aim[0];
		MiniMenu.walkDestTileY = aim[1];
		MiniMenu.clickTileX = -1;
		MiniMenu.clickTileY = -1;
		// renderTile's argument order: sinPitch, cosPitch, sinYaw, cosYaw.
		ProbeGround.drawPlainTile(new PlainTile(), 0, MathUtils.sin[pitch], MathUtils.cos[pitch], MathUtils.sin[yaw], MathUtils.cos[yaw], tileX, tileY, false);
		boolean hit = MiniMenu.clickTileX != -1;
		if (hit && (MiniMenu.clickTileX != tileX || MiniMenu.clickTileY != tileY)) {
			throw new IllegalStateException("click tile");
		}
		out.println("tile " + pitch + " " + yaw + " " + cameraX + " " + cameraZ + " " + cameraY + " " + tileX + " " + tileY + " " + heights[0] + " " + heights[1] + " " + heights[2] + " " + heights[3] + " " + aim[0] + " " + aim[1] + " " + (hit ? 1 : 0));
		Rasteriser.centerX = 0;
		Rasteriser.centerY = 0;
	}

	static int mix(int seed, int level, int x, int y) {
		int h = (x * 73856093) ^ (y * 19349663) ^ (level * 83492791) ^ seed;
		h ^= h >>> 15;
		h *= 0x2c1b3c6d;
		h ^= h >>> 12;
		return h >>> 8;
	}

	private static void frustum(Random r) {
		int pitch = r.nextInt(10) == 0 ? r.nextInt(2048) : 128 + r.nextInt(256);
		int yaw = r.nextInt(2048);
		int seed = r.nextInt();
		int visibility = 3 + r.nextInt(23);
		boolean edge = r.nextInt(5) == 0;
		int cameraX = edge ? r.nextInt(14000) - 300 : 2000 + r.nextInt(9000);
		int cameraY = edge ? r.nextInt(14000) - 300 : 2000 + r.nextInt(9000);
		int cameraZ = -200 - r.nextInt(2500);
		int half = 100 + r.nextInt(300);
		Rasteriser.screenLowerX = -half;
		Rasteriser.screenUpperX = half;
		Rasteriser.screenLowerY = -half * 167 / 256;
		Rasteriser.screenUpperY = half * 167 / 256;
		ProbeScene.visibility = visibility;
		ProbeScene.underwaterTileHeights = null;
		ProbeScene.surfaceTileHeights = new int[4][105][105];
		for (int x = 0; x < 105; x++) {
			for (int y = 0; y < 105; y++) {
				ProbeScene.surfaceTileHeights[0][x][y] = -(mix(seed, 0, x, y) % 600);
				ProbeScene.surfaceTileHeights[3][x][y] = -720 - mix(seed, 3, x, y) % 200;
			}
		}
		ProbeScene.visibleTiles = new boolean[visibility * 2 + 1][visibility * 2 + 1];
		ProbeScene.visibilityPoints = new boolean[visibility * 2 + 2][visibility * 2 + 2];
		ProbeScene.camera(cameraX, cameraZ, cameraY, pitch, yaw);
		ProbeScene.frustum((short) GlobalConfig.VIEW_DISTANCE);
		StringBuilder line = new StringBuilder("frustum ");
		line.append(pitch).append(' ').append(yaw).append(' ').append(cameraX).append(' ').append(cameraZ).append(' ').append(cameraY).append(' ');
		line.append(Rasteriser.screenLowerX).append(' ').append(Rasteriser.screenLowerY).append(' ');
		line.append(Rasteriser.screenUpperX).append(' ').append(Rasteriser.screenUpperY).append(' ');
		line.append(visibility).append(' ').append(seed).append(' ');
		for (int x = 0; x < visibility * 2 + 1; x++) {
			for (int y = 0; y < visibility * 2 + 1; y++) {
				line.append(ProbeScene.visibleTiles[x][y] ? '1' : '0');
			}
		}
		out.println(line);
	}

	private static StringBuilder ops;
	private static int parts;

	private static void op(String text) {
		if (ops.length() > 0) {
			ops.append(';');
		}
		ops.append(text);
	}

	private static Tile ensure(int level, int x, int y) {
		for (int l = level; l >= 0; l--) {
			if (ProbeScene.tiles[l][x][y] == null) {
				ProbeScene.tiles[l][x][y] = new Tile(l, x, y);
			}
		}
		return ProbeScene.tiles[level][x][y];
	}

	private static void scene(Random r) {
		ops = new StringBuilder();
		parts = 0;
		ProbeScene.tiles = new Tile[4][104][104];
		ProbeScene.scenery = new Scenery[5000];
		ProbeScene.sceneryLen = 0;
		ProbeScene.tileQueue = new LinkedList();
		ProbeScene.tileHeights = ProbeScene.surfaceTileHeights = new int[4][105][105];
		ProbeScene.tileWaterData = new int[104][104];
		ProbeScene.underwaterTileHeights = null;
		boolean edge = r.nextInt(6) == 0;
		int cx = edge ? new int[]{0, 1, 2, 101, 102, 103}[r.nextInt(6)] : 8 + r.nextInt(88);
		int cy = edge && r.nextBoolean() ? new int[]{0, 1, 2, 101, 102, 103}[r.nextInt(6)] : 8 + r.nextInt(88);
		int visibility = 2 + r.nextInt(4);
		int span = visibility + 2;
		int x0 = Math.max(0, cx - span);
		int x1 = Math.min(103, cx + span);
		int y0 = Math.max(0, cy - span);
		int y1 = Math.min(103, cy + span);
		if (r.nextInt(10) < 7) {
			ProbeScene.setBaseLevel(0);
			op("B");
		}
		int[] chance = {85, 25, 10};
		for (int x = x0; x <= x1; x++) {
			for (int y = y0; y <= y1; y++) {
				for (int level = 0; level < 3; level++) {
					if (r.nextInt(100) >= chance[level]) {
						continue;
					}
					int pick = r.nextInt(10);
					int kind = pick < 7 ? 0 : pick == 7 ? 1 : 2;
					Tile tile = ensure(level, x, y);
					// SceneGraph.setTile: a plain tile (12345678 is "no colour") or a shaped one.
					if (kind == 2) {
						tile.shapedTile = new ShapedTile();
						tile.plainTile = null;
					} else {
						tile.plainTile = new PlainTile();
						tile.plainTile.colorNE = kind == 1 ? 12345678 : 0;
						tile.shapedTile = null;
					}
					op("G " + level + " " + x + " " + y + " " + kind);
				}
			}
		}
		for (int i = r.nextInt(12); i > 0; i--) {
			int level = r.nextInt(3) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			int primary = parts++;
			int secondary = r.nextInt(10) < 3 ? parts++ : -1;
			int pf = WALLS[r.nextInt(8)];
			int sf = secondary < 0 ? 0 : SIDES[r.nextInt(4)];
			ProbeScene.setWall(level, x, y, 0, new Entity(primary), secondary < 0 ? null : new Entity(secondary), pf, sf, 0x40000000L);
			op("W " + level + " " + x + " " + y + " " + primary + " " + secondary + " " + pf + " " + sf);
		}
		for (int i = r.nextInt(8); i > 0; i--) {
			int level = r.nextInt(3) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			int flags = r.nextInt(3) == 0 ? 256 : SIDES[r.nextInt(4)];
			int primary = parts++;
			int secondary = flags == 256 && r.nextInt(10) < 4 ? parts++ : -1;
			int orientation = r.nextInt(4);
			// addLoc's argument order: the height, then models, flags and the orientation.
			ProbeScene.setWallDecor(level, x, y, 0, new Entity(primary), secondary < 0 ? null : new Entity(secondary), flags, orientation, r.nextInt(17) - 8, r.nextInt(17) - 8, 0x40000000L);
			op("D " + level + " " + x + " " + y + " " + primary + " " + secondary + " " + flags + " " + orientation);
		}
		for (int i = r.nextInt(8); i > 0; i--) {
			int level = r.nextInt(3) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			boolean early = r.nextInt(4) == 0;
			int part = parts++;
			ProbeScene.setGroundDecor(level, x, y, 0, new Entity(part), early ? 0xC0000000L : 0x40000000L, false);
			op("F " + level + " " + x + " " + y + " " + part + " " + (early ? 1 : 0));
		}
		for (int i = r.nextInt(5); i > 0; i--) {
			int level = r.nextInt(4) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			int primary = parts++;
			int secondary = r.nextInt(10) < 4 ? parts++ : -1;
			int tertiary = r.nextInt(10) < 2 ? parts++ : -1;
			boolean raised = r.nextBoolean();
			ProbeScene.setObjStack(level, x, y, 0, new Entity(primary), 0x60000000L, secondary < 0 ? null : new Entity(secondary), tertiary < 0 ? null : new Entity(tertiary));
			// The offset comes from supporting scenery; only whether it is zero matters to the order.
			ProbeScene.tiles[level][x][y].objStack.heightOffset = raised ? 40 : 0;
			op("O " + level + " " + x + " " + y + " " + primary + " " + secondary + " " + tertiary + " " + (raised ? 1 : 0));
		}
		for (int i = r.nextInt(14); i > 0; i--) {
			int level = r.nextInt(4) == 0 ? 1 : 0;
			int w = r.nextInt(4) == 0 ? 2 + r.nextInt(2) : 1;
			int h = r.nextInt(4) == 0 ? 2 + r.nextInt(2) : 1;
			int x = x0 + r.nextInt(x1 - x0 + 1) - (r.nextInt(20) == 0 ? 2 : 0);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			int part = parts++;
			// SceneGraph.addEntity's centre.
			int fx = x * 128 + w * 64;
			int fy = y * 128 + h * 64;
			boolean added = ProbeScene.addSceneryEntity(level, x, y, w, h, fx, fy, 0, new Entity(part), 0, false, 0x40000000L);
			op("S " + level + " " + x + " " + y + " " + w + " " + h + " " + fx + " " + fy + " " + part + " 1 0 " + (added ? 1 : 0));
		}
		if (r.nextInt(3) == 0) {
			for (int i = 1 + r.nextInt(3); i > 0; i--) {
				int x = x0 + r.nextInt(x1 - x0 + 1);
				int y = y0 + r.nextInt(y1 - y0 + 1);
				ProbeScene.bridgeTile(x, y);
				op("R " + x + " " + y);
			}
		}
		actors(r, x0, x1, y0, y1, r.nextInt(8));
		draw(r, cx, cy, visibility);
		// A second frame over the same tiles: the actors are replaced and the camera moves.
		for (int i = 0; i < ProbeScene.sceneryLen; i++) {
			ProbeScene.removeScenery(ProbeScene.scenery[i]);
			ProbeScene.scenery[i] = null;
		}
		ProbeScene.sceneryLen = 0;
		op("C");
		actors(r, x0, x1, y0, y1, r.nextInt(6));
		draw(r, Math.max(0, Math.min(103, cx + r.nextInt(3) - 1)), Math.max(0, Math.min(103, cy + r.nextInt(3) - 1)), visibility);
		if (failed) {
			failed = false;
			out.println("lost");
		} else {
			out.println("scene " + ops);
		}
	}

	private static boolean failed;

	private static void actors(Random r, int x0, int x1, int y0, int y1, int count) {
		for (int i = count; i > 0; i--) {
			int level = r.nextInt(5) == 0 ? 1 : 0;
			int fx = x0 * 128 + r.nextInt((x1 - x0 + 1) * 128);
			int fy = y0 * 128 + r.nextInt((y1 - y0 + 1) * 128);
			if (r.nextInt(3) == 0) {
				fx = (fx & ~127) + 64;
				fy = (fy & ~127) + 64;
			}
			int radius = new int[]{60, 60, 60, 124, 188}[r.nextInt(5)];
			int orientation = r.nextInt(2048);
			boolean extend = r.nextInt(4) == 0;
			int part = parts++;
			boolean added = ProbeScene.add(level, fx, fy, 0, radius, new Entity(part), orientation, 0x20000000L, extend);
			op("A " + level + " " + fx + " " + fy + " " + radius + " " + part + " " + orientation + " " + (extend ? 1 : 0) + " " + (added ? 1 : 0));
		}
	}

	private static void draw(Random r, int cx, int cy, int visibility) {
		int cameraX = cx * 128 + r.nextInt(128);
		int cameraY = cy * 128 + r.nextInt(128);
		if (r.nextInt(25) == 0) {
			cameraX = cx < 52 ? -r.nextInt(300) : 13312 + r.nextInt(300);
		}
		int walk = r.nextInt(4) - 1;
		int seed = r.nextInt();
		int percent = new int[]{0, 0, 10, 30}[r.nextInt(4)];
		ProbeScene.visibility = visibility;
		ProbeScene.camera(cameraX, 0, cameraY, 0, 0);
		ProbeScene.visibleTiles = new boolean[visibility * 2 + 1][visibility * 2 + 1];
		for (boolean[] row : ProbeScene.visibleTiles) {
			java.util.Arrays.fill(row, true);
		}
		byte[][][] hidden = new byte[4][104][104];
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					hidden[level][x][y] = (byte) (mix(seed, level, x, y) % 100 < percent ? 1 : 0);
				}
			}
		}
		MiniMenu.walkPending = walk >= 0;
		MiniMenu.walkDestPlane = walk;
		ProbeScene.out.clear();
		String drawn;
		try {
			ProbeScene.setShapedTile(cameraX, 0, cameraY, hidden, 0, (byte) 1, cx, cy);
			drawn = String.join(" ", ProbeScene.out);
		} catch (RuntimeException e) {
			// The original would lose the frame here; nothing to compare.
			drawn = "!";
			failed = true;
		}
		op("X " + cameraX + " " + cameraY + " " + visibility + " " + walk + " " + seed + " " + percent + ":" + drawn);
	}
}
