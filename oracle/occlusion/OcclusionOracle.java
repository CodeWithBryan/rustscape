package rt4;

import java.io.PrintStream;
import java.util.Random;

// Drives the unchanged RT4 occluder code that `cargo xtask occlusion-oracle`
// extracts into the holder class SceneGraph, over seeded inputs, and prints
// one observation per line.
public final class OcclusionOracle {
	private static final int[] SIDES = {1, 2, 4, 8};
	private static final int[] WALLS = {1, 2, 4, 8, 16, 32, 64, 128};
	private static final int[] HEIGHTS = {0, -40, -120, -240, -600, -32768};
	// FloType.occludeUnderlay of overlays 1 to 6.
	private static final boolean[] OVERLAYS = {true, true, false, true, false, true};
	private static PrintStream out;
	private static StringBuilder ops;
	private static int parts;
	private static boolean failed;

	public static void main(String[] args) throws Exception {
		long seed = Long.parseLong(args[0]);
		out = new PrintStream(new java.io.FileOutputStream(args[3]), false, "US-ASCII");
		FloTypeList.types = new FloType[OVERLAYS.length];
		for (int i = 0; i < OVERLAYS.length; i++) {
			FloTypeList.types[i] = new FloType();
			FloTypeList.types[i].occludeUnderlay = OVERLAYS[i];
		}
		Random random = new Random(seed);
		for (int i = 0; i < Integer.parseInt(args[1]); i++) {
			build(random);
		}
		for (int i = 0; i < Integer.parseInt(args[2]); i++) {
			scene(random);
		}
		for (int i = 0; i < 24; i++) {
			staged(random, i % 3);
		}
		out.close();
	}

	static int mix(int seed, int level, int x, int y) {
		int h = (x * 73856093) ^ (y * 19349663) ^ (level * 83492791) ^ seed;
		h ^= h >>> 15;
		h *= 0x2c1b3c6d;
		h ^= h >>> 12;
		return h >>> 8;
	}

	// Level floors 240 apart, with bumps on `rough` percent of the corners.
	static int height(int seed, int rough, int level, int x, int y) {
		int h = -240 * level;
		if (mix(seed, level, x, y) % 100 < rough) {
			h -= mix(seed ^ 0x5bd1e995, level, x, y) % 64;
		}
		return h;
	}

	private static void op(String text) {
		if (ops.length() > 0) {
			ops.append(';');
		}
		ops.append(text);
	}

	private static void reset(int seed, int rough) {
		SceneGraph.tiles = new Tile[4][104][104];
		SceneGraph.scenery = new Scenery[5000];
		SceneGraph.sceneryLen = 0;
		SceneGraph.tileQueue = new LinkedList();
		int[][][] heights = new int[4][105][105];
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 105; x++) {
				for (int y = 0; y < 105; y++) {
					heights[level][x][y] = height(seed, rough, level, x, y);
				}
			}
		}
		SceneGraph.tileHeights = SceneGraph.surfaceTileHeights = heights;
		SceneGraph.underwaterTileHeights = null;
		SceneGraph.tileWaterData = new int[104][104];
		SceneGraph.occludeFlags = new int[4][105][105];
		SceneGraph.shadowmap = new byte[4][105][105];
		SceneGraph.tileShapes = new byte[4][104][104];
		SceneGraph.tileUnderlays = new byte[4][104][104];
		SceneGraph.tileOverlays = new byte[4][104][104];
		SceneGraph.occluders = new Occluder[500];
		SceneGraph.occluderCount = 0;
		SceneGraph.activeOccluders = new Occluder[500];
		SceneGraph.activeOccluderCount = 0;
		SceneGraph.occlusionCache = new int[4][105][105];
		SceneGraph.startLevel = 0;
	}

	// SceneGraph.readLocs places locations on tiles 1 to 102 only.
	private static void loc(int level, int x, int y, int shape, int rotation, boolean occlude, boolean hidden) {
		if (x < 1 || y < 1 || x > 102 || y > 102) {
			return;
		}
		LocInfo info = new LocInfo();
		info.occlude = occlude;
		info.active = true;
		info.render = hidden;
		SceneGraph.locMarks(true, level, x, y, shape, rotation, info);
		op("L " + level + " " + x + " " + y + " " + shape + " " + rotation + " " + (occlude ? 1 : 0) + " " + (hidden ? 1 : 0));
	}

	// SceneGraph.buildScene visits the same tiles.
	private static void floor(int level, int x, int y, int underlay, int overlay, int shape) {
		if (x < 1 || y < 1 || x > 102 || y > 102) {
			return;
		}
		SceneGraph.tileUnderlays[level][x][y] = (byte) underlay;
		SceneGraph.tileOverlays[level][x][y] = (byte) overlay;
		SceneGraph.tileShapes[level][x][y] = (byte) shape;
		SceneGraph.floorMark(level, x, y);
		op("P " + level + " " + x + " " + y + " " + underlay + " " + overlay + " " + shape);
	}

	// Wall runs, roof patches, floors and stray locations inside a rectangle.
	private static void marks(Random r, int x0, int x1, int y0, int y1, int count) {
		for (int c = 0; c < count; c++) {
			int kind = r.nextInt(10);
			int level = r.nextInt(3) == 0 ? 1 + r.nextInt(3) : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			if (kind < 5) {
				boolean alongY = r.nextBoolean();
				int length = 1 + r.nextInt(13);
				int above = r.nextInt(3) == 0 ? 1 + r.nextInt(3) : 0;
				int rotation = alongY ? (r.nextBoolean() ? 0 : 2) : (r.nextBoolean() ? 1 : 3);
				int shape = r.nextInt(6) == 0 ? 2 : 0;
				for (int l = level; l <= Math.min(3, level + above); l++) {
					for (int t = 0; t < length; t++) {
						loc(l, alongY ? x : x + t, alongY ? y + t : y, shape, rotation, r.nextInt(14) != 0, r.nextInt(40) == 0);
					}
				}
			} else if (kind < 7) {
				int w = 1 + r.nextInt(4);
				int h = 1 + r.nextInt(4);
				int shape = new int[]{12, 12, 13, 14, 15, 16, 17, 17, 18, 10, 22, 9}[r.nextInt(12)];
				int roofLevel = r.nextInt(5) == 0 ? 0 : 1 + r.nextInt(3);
				for (int dx = 0; dx < w; dx++) {
					for (int dy = 0; dy < h; dy++) {
						loc(roofLevel, x + dx, y + dy, shape, r.nextInt(4), r.nextBoolean(), r.nextInt(40) == 0);
					}
				}
			} else if (kind < 9) {
				int w = 1 + r.nextInt(5);
				int h = 1 + r.nextInt(5);
				int floorLevel = r.nextInt(6) == 0 ? 0 : 1 + r.nextInt(3);
				int underlay = r.nextInt(4) == 0 ? 0 : r.nextInt(6) == 0 ? 200 : 1 + r.nextInt(5);
				int overlay = r.nextInt(3) == 0 ? 1 + r.nextInt(OVERLAYS.length) : 0;
				int shape = overlay > 0 && r.nextInt(3) == 0 ? 1 + r.nextInt(11) : 0;
				for (int dx = 0; dx < w; dx++) {
					for (int dy = 0; dy < h; dy++) {
						floor(floorLevel, x + dx, y + dy, underlay, overlay, shape);
					}
				}
			} else {
				loc(level, x, y, r.nextInt(23), r.nextInt(4), r.nextBoolean(), false);
			}
		}
	}

	private static String occluders() {
		StringBuilder text = new StringBuilder();
		for (int i = 0; i < SceneGraph.occluderCount; i++) {
			Occluder o = SceneGraph.occluders[i];
			if (o.minTileX != o.minX / 128 || o.maxTileX != o.maxX / 128 || o.minTileY != o.minY / 128 || o.maxTileY != o.maxY / 128) {
				throw new IllegalStateException("tile bounds");
			}
			text.append(i == 0 ? "" : " ").append(o.type).append(' ').append(o.minX).append(' ').append(o.maxX).append(' ').append(o.minY).append(' ').append(o.maxY).append(' ').append(o.minZ).append(' ').append(o.maxZ);
		}
		return text.toString();
	}

	private static void build(Random r) {
		ops = new StringBuilder();
		int seed = r.nextInt();
		int rough = new int[]{0, 3, 30, 30}[r.nextInt(4)];
		reset(seed, rough);
		boolean whole = r.nextInt(3) == 0;
		int x0 = whole ? 0 : 20 + r.nextInt(40);
		int y0 = whole ? 0 : 20 + r.nextInt(40);
		marks(r, x0, whole ? 104 : x0 + 24, y0, whole ? 104 : y0 + 24, 6 + r.nextInt(34));
		SceneGraph.buildOccluders();
		out.println("build " + seed + " " + rough + ";" + ops + "|" + occluders());
		for (int i = 0; i < 3; i++) {
			view(r, x0 + 12, y0 + 12, i);
		}
	}

	private static int[] groups(Random r, int count, StringBuilder text) {
		SceneGraph.roofGroupMaxHeight = new int[count];
		SceneGraph.roofGroupMinX = new int[count];
		SceneGraph.roofGroupMaxX = new int[count];
		SceneGraph.roofGroupMinZ = new int[count];
		SceneGraph.roofGroupMaxZ = new int[count];
		text.append(count);
		for (int i = 0; i < count; i++) {
			int height = r.nextInt(4) == 0 ? -1000000 : -r.nextInt(900);
			int minX = r.nextInt(13312);
			int maxX = minX + r.nextInt(4000);
			int minZ = r.nextInt(13312);
			int maxZ = minZ + r.nextInt(4000);
			SceneGraph.roofGroupMaxHeight[i] = height;
			SceneGraph.roofGroupMinX[i] = minX;
			SceneGraph.roofGroupMaxX[i] = maxX;
			SceneGraph.roofGroupMinZ[i] = minZ;
			SceneGraph.roofGroupMaxZ[i] = maxZ;
			text.append(' ').append(height).append(' ').append(minX).append(' ').append(maxX).append(' ').append(minZ).append(' ').append(maxZ);
		}
		return SceneGraph.roofGroupMaxHeight;
	}

	private static void visible(int visibility, int seed, int percent) {
		SceneGraph.visibleTiles = new boolean[visibility * 2 + 1][visibility * 2 + 1];
		for (int column = 0; column < visibility * 2 + 1; column++) {
			for (int row = 0; row < visibility * 2 + 1; row++) {
				SceneGraph.visibleTiles[column][row] = mix(seed, 0, column, row) % 100 < percent;
			}
		}
	}

	// A point in the shadow of an active occluder (or just beside it), or anywhere.
	private static int[] behind(Random r) {
		if (SceneGraph.activeOccluderCount > 0 && r.nextInt(5) != 0) {
			Occluder o = SceneGraph.activeOccluders[r.nextInt(SceneGraph.activeOccluderCount)];
			int tx = o.minX + r.nextInt(o.maxX - o.minX + 61) - 30;
			int ty = o.minY + r.nextInt(o.maxY - o.minY + 61) - 30;
			int tz = o.minZ + r.nextInt(o.maxZ - o.minZ + 61) - 30;
			double t = 1.02 + r.nextDouble() * 2.0;
			return new int[]{(int) (SceneGraph.cameraX + (tx - SceneGraph.cameraX) * t), (int) (SceneGraph.cameraZ + (tz - SceneGraph.cameraZ) * t), (int) (SceneGraph.cameraY + (ty - SceneGraph.cameraY) * t)};
		}
		return new int[]{r.nextInt(13312), -r.nextInt(1200), r.nextInt(13312)};
	}

	private static int clamp(int n) {
		return Math.max(0, Math.min(103, n));
	}

	// The third view of a scene (`staged` 2), when the scene has a wall
	// occluder along the y axis: the camera stands 128 units east of it and
	// one unit south of its south end, so that its shadow's south edge runs
	// two units inside the tile row for the next two tiles; the first question
	// is about scenery on those tiles, whose south corners isAreaVisible takes
	// two units inside the row.
	// The second view (`staged` 1), when the scene has a floor occluder two
	// levels up or more: the camera is 512 units above it and four units west
	// of its west edge, so that the edge of its shadow moves one unit east 128
	// units below it; the first question is about a south wall on the level
	// below, at the shadow's corner, whose west end is hidden at its top and
	// at its middle, 119 units down, and no longer 129 units down.
	private static void view(Random r, int cx, int cy, int staged) {
		int visibility = new int[]{6, 12, 25, 56}[r.nextInt(4)];
		int cameraX = cx * 128 + r.nextInt(8000) - 4000;
		int cameraY = cy * 128 + r.nextInt(8000) - 4000;
		int cameraZ = -50 - r.nextInt(2500);
		if (SceneGraph.occluderCount > 0 && r.nextInt(4) == 0) {
			// Around the 32 units either side of a wall inside which it is not used.
			Occluder o = SceneGraph.occluders[r.nextInt(SceneGraph.occluderCount)];
			int near = r.nextBoolean() ? new int[]{-33, -32, -31, 31, 32, 33}[r.nextInt(6)] : r.nextInt(81) - 40;
			if (o.type == 1) {
				cameraX = o.minX + near;
				cameraY = (o.minY + o.maxY) / 2 + r.nextInt(601) - 300;
			} else if (o.type == 2) {
				cameraY = o.minY + near;
				cameraX = (o.minX + o.maxX) / 2 + r.nextInt(601) - 300;
			} else {
				cameraZ = o.minZ - 128 + r.nextInt(5) - 2;
				cameraX = (o.minX + o.maxX) / 2 + r.nextInt(2001) - 1000;
				cameraY = (o.minY + o.maxY) / 2 + r.nextInt(2001) - 1000;
			}
		}
		int seed = r.nextInt();
		int percent = new int[]{100, 100, 70, 20}[r.nextInt(4)];
		int removed = new int[]{0, 0, 1, 3, 8}[r.nextInt(5)];
		Occluder wall = null;
		Occluder roof = null;
		for (int i = 0; i < SceneGraph.occluderCount; i++) {
			Occluder o = SceneGraph.occluders[i];
			if (staged == 2 && wall == null && o.type == 1 && o.minX >= 3 * 128 && o.minX <= 102 * 128) {
				wall = o;
			}
			if (staged == 1 && roof == null && o.type == 4 && o.minZ <= -480 && o.minX >= 128 && o.maxY - o.minY >= 384) {
				roof = o;
			}
		}
		if (roof != null) {
			cameraX = roof.minX - 4;
			cameraY = roof.minY + 200;
			cameraZ = roof.minZ - 512;
			visibility = 25;
			percent = 100;
			removed = 0;
		}
		if (wall != null) {
			cameraX = wall.minX + 128;
			cameraY = wall.minY - 1;
			cameraZ = wall.maxZ - 120;
			visibility = 25;
			percent = 100;
			removed = 0;
		}
		StringBuilder line = new StringBuilder("view ");
		line.append(cameraX).append(' ').append(cameraZ).append(' ').append(cameraY).append(' ').append(visibility).append(' ').append(seed).append(' ').append(percent).append(' ');
		groups(r, removed, line);
		SceneGraph.visibility = visibility;
		SceneGraph.camera(cameraX, cameraZ, cameraY, 0, 0);
		visible(visibility, seed, percent);
		SceneGraph.buildActiveOccluders();
		line.append('|');
		for (int i = 0; i < SceneGraph.activeOccluderCount; i++) {
			Occluder o = SceneGraph.activeOccluders[i];
			line.append(i == 0 ? "" : " ").append(o.type).append(' ').append(o.minX).append(' ').append(o.maxX).append(' ').append(o.minY).append(' ').append(o.maxY).append(' ').append(o.minZ).append(' ').append(o.maxZ).append(' ').append(o.direction);
			if (o.direction <= 2) {
				line.append(' ').append(o.nearYSlope).append(' ').append(o.farYSlope).append(' ').append(o.nearZSlope).append(' ').append(o.farZSlope);
			} else if (o.direction <= 4) {
				line.append(' ').append(o.farXSlope).append(' ').append(o.nearXSlope).append(' ').append(o.nearZSlope).append(' ').append(o.farZSlope);
			} else {
				line.append(' ').append(o.farXSlope).append(' ').append(o.nearXSlope).append(' ').append(o.nearYSlope).append(' ').append(o.farYSlope);
			}
		}
		line.append('|');
		for (int i = 0; i < 40; i++) {
			int[] p = behind(r);
			line.append(i == 0 ? "" : " ").append(p[0]).append(' ').append(p[1]).append(' ').append(p[2]).append(' ').append(SceneGraph.isOccluded(p[0], p[1], p[2]) ? 1 : 0);
		}
		line.append('|');
		// One frame of questions: the answers about tiles are remembered.
		SceneGraph.drawCycle++;
		int x = 0;
		int y = 0;
		for (int i = 0; i < 70; i++) {
			if (i == 0 || r.nextInt(3) != 0) {
				int[] p = behind(r);
				x = clamp(p[0] >> 7);
				y = clamp(p[2] >> 7);
			}
			line.append(i == 0 ? "" : " ");
			if (i == 0 && wall != null) {
				int level = Math.min(3, -wall.maxZ / 240);
				int xMin = wall.minX / 128 - 3;
				int yMin = wall.minY / 128;
				int yMax = Math.min(103, yMin + 1);
				line.append("A ").append(level).append(' ').append(xMin).append(' ').append(xMin + 1).append(' ').append(yMin).append(' ').append(yMax).append(" 0 ").append(SceneGraph.isAreaVisible(level, xMin, xMin + 1, yMin, yMax, 0) ? 1 : 0);
				continue;
			}
			if (i == 0 && roof != null) {
				int level = Math.min(3, -roof.minZ / 240) - 1;
				int tileX = roof.minX / 128;
				int tileY = roof.minY / 128;
				line.append("W ").append(level).append(' ').append(tileX).append(' ').append(tileY).append(" 8 ").append(SceneGraph.isWallVisible(level, tileX, tileY, 8) ? 1 : 0);
				continue;
			}
			ask(r, line, r.nextInt(3) == 0 ? r.nextInt(4) : 0, x, y, r.nextInt(8));
		}
		// Sixty more where an answer is about to change: at the edges of a shadow.
		for (int i = 0; i < 60; i++) {
			int[] at = SceneGraph.activeOccluderCount > 0 ? edge(r) : new int[]{r.nextInt(4), r.nextInt(104), r.nextInt(104)};
			line.append(' ');
			ask(r, line, at[0], at[1], at[2], new int[]{0, 2, 3, 3, 3, 4, 5, 6}[r.nextInt(8)]);
		}
		out.println(line);
	}

	private static void ask(Random r, StringBuilder line, int level, int x, int y, int kind) {
		int height = HEIGHTS[r.nextInt(HEIGHTS.length)];
		if (kind < 2) {
			line.append("T ").append(level).append(' ').append(x).append(' ').append(y).append(' ').append(SceneGraph.isTileOccluded(level, x, y) ? 1 : 0);
		} else if (kind == 2) {
			line.append("O ").append(level).append(' ').append(x).append(' ').append(y).append(' ').append(height).append(' ').append(SceneGraph.isTileVisible(level, x, y, height) ? 1 : 0);
		} else if (kind < 5) {
			int flags = new int[]{1, 2, 4, 8, 16, 32, 64, 128, 0, 3, 256}[r.nextInt(11)];
			line.append("W ").append(level).append(' ').append(x).append(' ').append(y).append(' ').append(flags).append(' ').append(SceneGraph.isWallVisible(level, x, y, flags) ? 1 : 0);
		} else {
			int xMin = clamp(x - r.nextInt(3));
			int xMax = clamp(x + r.nextInt(3));
			int yMin = clamp(y - r.nextInt(3));
			int yMax = clamp(y + r.nextInt(3));
			line.append("A ").append(level).append(' ').append(xMin).append(' ').append(xMax).append(' ').append(yMin).append(' ').append(yMax).append(' ').append(height).append(' ').append(SceneGraph.isAreaVisible(level, xMin, xMax, yMin, yMax, height) ? 1 : 0);
		}
	}

	// One end of a span, a little either side of it, or somewhere inside.
	private static int across(Random r, int low, int high) {
		int pick = r.nextInt(3);
		if (pick == 0) {
			return low + r.nextInt(257) - 128;
		} else if (pick == 1) {
			return high + r.nextInt(257) - 128;
		}
		return low + r.nextInt(Math.max(1, high - low + 1));
	}

	// A level and tile where the foot, middle, top or peak of a wall there
	// lies about on the boundary of an active occluder's shadow.
	private static int[] edge(Random r) {
		Occluder o = SceneGraph.activeOccluders[r.nextInt(SceneGraph.activeOccluderCount)];
		int part = new int[]{0, 1, 121, 231, 239}[r.nextInt(5)];
		int level;
		int x;
		int y;
		if (o.direction == 5) {
			level = Math.max(0, Math.min(3, -o.minZ / 240 - (r.nextInt(4) == 0 ? 2 : 1)));
			int delta = -240 * level - part - o.minZ;
			x = across(r, o.minX + (o.farXSlope * delta >> 8), o.maxX + (o.nearXSlope * delta >> 8));
			y = across(r, o.minY + (o.nearYSlope * delta >> 8), o.maxY + (o.farYSlope * delta >> 8));
		} else {
			level = r.nextInt(3);
			int target = -240 * level - part;
			boolean upper = r.nextBoolean();
			int slope = upper ? o.nearZSlope : o.farZSlope;
			int bound = upper ? o.minZ : o.maxZ;
			int delta = slope == 0 ? 0 : (int) ((long) (target - bound) * 256 / slope);
			if (delta <= 0 || delta > 9000) {
				delta = 128 + r.nextInt(2000);
			}
			delta += r.nextInt(257) - 128;
			if (o.direction <= 2) {
				x = o.direction == 1 ? o.minX - delta : o.minX + delta;
				y = across(r, o.minY + (o.nearYSlope * delta >> 8), o.maxY + (o.farYSlope * delta >> 8));
			} else {
				y = o.direction == 3 ? o.minY - delta : o.minY + delta;
				x = across(r, o.minX + (o.farXSlope * delta >> 8), o.maxX + (o.nearXSlope * delta >> 8));
			}
		}
		return new int[]{level, clamp(x >> 7), clamp(y >> 7)};
	}

	private static Tile ensure(int level, int x, int y) {
		for (int l = level; l >= 0; l--) {
			if (SceneGraph.tiles[l][x][y] == null) {
				SceneGraph.tiles[l][x][y] = new Tile(l, x, y);
			}
		}
		return SceneGraph.tiles[level][x][y];
	}

	// SceneGraph.setTile: a plain tile (12345678 is "no colour"), a shaped one, or (3) none.
	private static void setGround(int level, int x, int y, int kind) {
		Tile tile = ensure(level, x, y);
		tile.shapedTile = kind == 2 ? new ShapedTile() : null;
		tile.plainTile = kind < 2 ? new PlainTile() : null;
		if (kind < 2) {
			tile.plainTile.colorNE = kind == 1 ? 12345678 : 0;
		}
	}

	private static void ground(int seed, int x0, int x1, int y0, int y1, int[] chance) {
		op("G " + seed + " " + x0 + " " + x1 + " " + y0 + " " + y1 + " " + chance[0] + " " + chance[1] + " " + chance[2]);
		for (int x = x0; x <= x1; x++) {
			for (int y = y0; y <= y1; y++) {
				for (int level = 0; level < 3; level++) {
					int roll = mix(seed, level, x, y);
					if (roll % 100 >= chance[level]) {
						continue;
					}
					int pick = roll / 100 % 10;
					setGround(level, x, y, pick < 7 ? 0 : pick == 7 ? 1 : 2);
				}
			}
		}
	}

	// Three arrangements behind one wall of eight tiles, each moved about
	// the scene, where an answer depends on what was asked before or on
	// which level's heights are read:
	// 0: scenery over a tile without ground whose only question so far is
	//    its wall decoration's, asked while the decoration is not yet drawn;
	// 1: the same over a bridge column, asked for the ground linked below;
	// 2: scenery on a bridge deck, whose tile keeps its own level.
	private static void staged(Random r, int variant) {
		ops = new StringBuilder();
		parts = 0;
		int seed = r.nextInt();
		reset(seed, 0);
		int ox = r.nextInt(21);
		int oy = r.nextInt(31);
		SceneGraph.setBaseLevel(0);
		op("B");
		ground(r.nextInt(), 40 + ox, 62 + ox, 36 + oy, 52 + oy, new int[]{100, 0, 0});
		int tx = 46 + ox;
		int ty = 45 + oy;
		if (variant < 2) {
			// A spike at the tile's far corner keeps that tile in view.
			SceneGraph.tileHeights[0][tx + 1][ty + 1] = -5000;
			op("H 0 " + (tx + 1) + " " + (ty + 1) + " -5000");
		}
		for (int y = 40; y < 48; y++) {
			loc(0, 50 + ox, y + oy, 0, 0, true, false);
		}
		SceneGraph.buildOccluders();
		op("K");
		int sx = 44 + ox;
		int sy = 42 + oy;
		if (variant == 0) {
			setGround(0, tx, ty, 3);
			op("g 0 " + tx + " " + ty + " 3");
			int decor = parts++;
			SceneGraph.setWallDecor(0, tx, ty, 0, new Entity(decor, 0), null, 4, 0, 0, 0, 0x40000000L);
			op("D 0 " + tx + " " + ty + " " + decor + " -1 4 0 0");
		} else if (variant == 1) {
			SceneGraph.bridgeTile(tx, ty);
			op("R " + tx + " " + ty);
		}
		if (variant < 2) {
			int part = parts++;
			boolean added = SceneGraph.addSceneryEntity(0, sx, sy, 3, 4, sx * 128 + 192, sy * 128 + 256, 0, new Entity(part, -100), 0, false, 0x40000000L);
			op("S 0 " + sx + " " + sy + " 3 4 " + (sx * 128 + 192) + " " + (sy * 128 + 256) + " " + part + " -100 " + (added ? 1 : 0));
		} else {
			int ux = 45 + ox;
			int uy = 44 + oy;
			setGround(1, ux, uy, 0);
			op("g 1 " + ux + " " + uy + " 0");
			int part = parts++;
			boolean added = SceneGraph.addSceneryEntity(1, ux, uy, 1, 1, ux * 128 + 64, uy * 128 + 64, 0, new Entity(part, 0), 0, false, 0x40000000L);
			op("S 1 " + ux + " " + uy + " 1 1 " + (ux * 128 + 64) + " " + (uy * 128 + 64) + " " + part + " 0 " + (added ? 1 : 0));
			SceneGraph.bridgeTile(ux, uy);
			op("R " + ux + " " + uy);
		}
		int cameraX = (58 + ox) * 128 + 64 + r.nextInt(41) - 20;
		int cameraY = (44 + oy) * 128 + 64 + r.nextInt(41) - 20;
		drawAt(r, 58 + ox, 44 + oy, 16, cameraX, variant == 2 ? -400 : -120, cameraY, true);
		if (failed) {
			throw new IllegalStateException("staged scene lost");
		}
		out.println("scene " + seed + " 0;" + ops);
	}

	private static int minY(Random r) {
		return HEIGHTS[r.nextInt(HEIGHTS.length)];
	}

	// A scene around the camera as the picking oracle makes them, with
	// occluders among it, drawn twice.
	private static void scene(Random r) {
		ops = new StringBuilder();
		parts = 0;
		int seed = r.nextInt();
		int rough = new int[]{0, 0, 30, 30}[r.nextInt(4)];
		reset(seed, rough);
		int cx = 14 + r.nextInt(76);
		int cy = 14 + r.nextInt(76);
		int visibility = 4 + r.nextInt(6);
		int span = visibility + 2;
		int x0 = cx - span;
		int x1 = cx + span;
		int y0 = cy - span;
		int y1 = cy + span;
		if (r.nextInt(10) < 7) {
			SceneGraph.setBaseLevel(0);
			op("B");
		}
		// Which tiles have ground, and of what kind, follows from a seed.
		int[] chance = r.nextInt(3) == 0 ? new int[]{65, 45, 15} : new int[]{88, 30, 12};
		int ground = r.nextInt();
		ground(ground, x0, x1, y0, y1, chance);
		marks(r, x0, x1, y0, y1, 3 + r.nextInt(9));
		SceneGraph.buildOccluders();
		op("K");
		for (int i = r.nextInt(16); i > 0; i--) {
			int level = r.nextInt(3) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			int primary = parts++;
			int secondary = r.nextInt(10) < 3 ? parts++ : -1;
			int pf = WALLS[r.nextInt(8)];
			int sf = secondary < 0 ? 0 : SIDES[r.nextInt(4)];
			SceneGraph.setWall(level, x, y, 0, new Entity(primary, 0), secondary < 0 ? null : new Entity(secondary, 0), pf, sf, 0x40000000L);
			op("W " + level + " " + x + " " + y + " " + primary + " " + secondary + " " + pf + " " + sf);
		}
		for (int i = r.nextInt(12); i > 0; i--) {
			int level = r.nextInt(3) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			int flags = r.nextInt(3) == 0 ? 256 : SIDES[r.nextInt(4)];
			int primary = parts++;
			int secondary = flags == 256 && r.nextInt(10) < 4 ? parts++ : -1;
			int orientation = r.nextInt(4);
			int height = minY(r);
			// addLoc's argument order: the height, then models, flags and the orientation.
			SceneGraph.setWallDecor(level, x, y, 0, new Entity(primary, height), secondary < 0 ? null : new Entity(secondary, 0), flags, orientation, r.nextInt(17) - 8, r.nextInt(17) - 8, 0x40000000L);
			op("D " + level + " " + x + " " + y + " " + primary + " " + secondary + " " + flags + " " + orientation + " " + height);
		}
		for (int i = r.nextInt(12); i > 0; i--) {
			int level = r.nextInt(3) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			boolean early = r.nextInt(4) == 0;
			int part = parts++;
			SceneGraph.setGroundDecor(level, x, y, 0, new Entity(part, 0), early ? 0xC0000000L : 0x40000000L, false);
			op("F " + level + " " + x + " " + y + " " + part + " " + (early ? 1 : 0));
		}
		for (int i = r.nextInt(6); i > 0; i--) {
			int level = r.nextInt(4) == 0 ? 1 : 0;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			int primary = parts++;
			int secondary = r.nextInt(10) < 4 ? parts++ : -1;
			int tertiary = r.nextInt(10) < 2 ? parts++ : -1;
			boolean raised = r.nextBoolean();
			SceneGraph.setObjStack(level, x, y, 0, new Entity(primary, 0), 0x60000000L, secondary < 0 ? null : new Entity(secondary, 0), tertiary < 0 ? null : new Entity(tertiary, 0));
			// The offset comes from supporting scenery; only whether it is zero matters to the order.
			SceneGraph.tiles[level][x][y].objStack.heightOffset = raised ? 40 : 0;
			op("O " + level + " " + x + " " + y + " " + primary + " " + secondary + " " + tertiary + " " + (raised ? 1 : 0));
		}
		java.util.ArrayList<int[]> upper = new java.util.ArrayList<int[]>();
		for (int i = r.nextInt(24); i > 0; i--) {
			int level = r.nextInt(4) == 0 ? 1 : 0;
			int w = r.nextInt(4) == 0 ? 2 + r.nextInt(2) : 1;
			int h = r.nextInt(4) == 0 ? 2 + r.nextInt(2) : 1;
			int x = x0 + r.nextInt(x1 - x0 + 1);
			int y = y0 + r.nextInt(y1 - y0 + 1);
			if (level == 1 && w == 1 && h == 1) {
				upper.add(new int[]{x, y});
			}
			int part = parts++;
			int height = r.nextBoolean() ? 0 : minY(r);
			// SceneGraph.addEntity's centre.
			int fx = x * 128 + w * 64;
			int fy = y * 128 + h * 64;
			boolean added = SceneGraph.addSceneryEntity(level, x, y, w, h, fx, fy, 0, new Entity(part, height), 0, false, 0x40000000L);
			op("S " + level + " " + x + " " + y + " " + w + " " + h + " " + fx + " " + fy + " " + part + " " + height + " " + (added ? 1 : 0));
		}
		if (r.nextInt(3) == 0) {
			for (int i = 2 + r.nextInt(5); i > 0; i--) {
				int x = x0 + r.nextInt(x1 - x0 + 1);
				int y = y0 + r.nextInt(y1 - y0 + 1);
				if (!upper.isEmpty() && r.nextInt(3) != 0) {
					// Under something standing on the level above.
					int[] at = upper.remove(r.nextInt(upper.size()));
					x = at[0];
					y = at[1];
				}
				SceneGraph.bridgeTile(x, y);
				op("R " + x + " " + y);
			}
		}
		actors(r, x0, x1, y0, y1, r.nextInt(8));
		draw(r, cx, cy, visibility);
		// A second frame over the same tiles: the actors are replaced and the camera moves.
		for (int i = 0; i < SceneGraph.sceneryLen; i++) {
			SceneGraph.removeScenery(SceneGraph.scenery[i]);
			SceneGraph.scenery[i] = null;
		}
		SceneGraph.sceneryLen = 0;
		op("C");
		actors(r, x0, x1, y0, y1, r.nextInt(6));
		draw(r, cx + r.nextInt(5) - 2, cy + r.nextInt(5) - 2, visibility);
		if (failed) {
			failed = false;
			out.println("lost");
		} else {
			out.println("scene " + seed + " " + rough + ";" + ops);
		}
	}

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
			int height = minY(r);
			boolean added = SceneGraph.add(level, fx, fy, 0, radius, new Entity(part, height), orientation, 0x20000000L, extend);
			op("A " + level + " " + fx + " " + fy + " " + radius + " " + part + " " + orientation + " " + (extend ? 1 : 0) + " " + height + " " + (added ? 1 : 0));
		}
	}

	private static void draw(Random r, int cx, int cy, int visibility) {
		int cameraX = cx * 128 + r.nextInt(128);
		int cameraY = cy * 128 + r.nextInt(128);
		int cameraZ = r.nextInt(4) == 0 ? -100 - r.nextInt(1800) : -250 - r.nextInt(900);
		drawAt(r, cx, cy, visibility, cameraX, cameraZ, cameraY, false);
	}

	// `whole`: nothing is taken out of view by roof removal or the frustum.
	private static void drawAt(Random r, int cx, int cy, int visibility, int cameraX, int cameraZ, int cameraY, boolean whole) {
		int walk = r.nextInt(4) - 1;
		int seed = r.nextInt();
		int percent = whole ? 0 : new int[]{0, 0, 0, 10, 30}[r.nextInt(5)];
		int visibleSeed = r.nextInt();
		int visiblePercent = whole || r.nextInt(5) != 0 ? 100 : 85;
		StringBuilder fields = new StringBuilder();
		fields.append(cameraX).append(' ').append(cameraZ).append(' ').append(cameraY).append(' ').append(visibility).append(' ').append(walk).append(' ').append(seed).append(' ').append(percent).append(' ').append(visibleSeed).append(' ').append(visiblePercent).append(' ');
		groups(r, !whole && r.nextInt(6) == 0 ? 1 + r.nextInt(2) : 0, fields);
		SceneGraph.visibility = visibility;
		SceneGraph.camera(cameraX, cameraZ, cameraY, 0, 0);
		visible(visibility, visibleSeed, visiblePercent);
		byte[][][] hidden = new byte[4][104][104];
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					hidden[level][x][y] = (byte) (mix(seed, level, x, y) % 100 < percent ? 1 : 0);
				}
			}
		}
		SceneGraph.buildActiveOccluders();
		fields.append(' ').append(SceneGraph.activeOccluderCount);
		MiniMenu.walkPending = walk >= 0;
		MiniMenu.walkDestPlane = walk;
		SceneGraph.out.clear();
		String drawn;
		try {
			SceneGraph.setShapedTile(cameraX, cameraZ, cameraY, hidden, 0, (byte) 1, cx, cy);
			drawn = String.join(" ", SceneGraph.out);
		} catch (RuntimeException e) {
			// The original would lose the frame here; nothing to compare.
			drawn = "!";
			failed = true;
		}
		op("X " + fields + ":" + drawn);
	}
}
