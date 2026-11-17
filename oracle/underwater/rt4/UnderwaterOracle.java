package rt4;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.lang.reflect.Field;

/**
 * Runs the original code that loads and builds the underwater scene over
 * scenes described by an input file, and writes what it produced as named
 * sections.
 *
 * <p>Inputs from the Rust side: the surface's ground-level tile arrays, render
 * flags and corner heights, the raw underwater map files (um, ul) of the
 * scene's regions, the floor definitions, each texture's brightness, speed
 * and material type, and for every location type its facts and its models as
 * LocType.getGlModel leaves them.
 *
 * <p>Original code: SceneGraph.load, setUnderwater, readTile, readLocs,
 * addLoc, setWall, setWallDecor, setGroundDecor, addEntity, addSceneryEntity,
 * setTile, clearTerrainRegion, mergeSceneNormals, buildGlPlainTiles and what
 * it calls, GlModel.alignToTerrain, the unchanged GlTile, FluType, FloType,
 * ColorUtils, Tile and entity classes, and four fragments of
 * SceneGraph.buildScene the generator wraps in methods without changing them:
 * the blended underlay colours, the tile loop (which fills tileWaterData on
 * the surface and makes the tile objects), the HD tile lightness and the
 * corner normals.
 *
 * <p>Adapter code here: what LoginManager.rebuildMap, SceneGraph.init,
 * LoginManager.readStaticTerrain/readStaticLocs and the 64 x 64 loop of
 * SceneGraph.readTerrain do around those calls, written out below in their
 * order. readTerrain itself is not compiled: its tail (occlusion, environment
 * and lights) needs most of the client.
 */
public final class UnderwaterOracle {
	private static DataOutputStream out;

	public static void main(String[] args) throws Exception {
		try (DataInputStream in = new DataInputStream(new BufferedInputStream(new FileInputStream(args[0]), 1 << 20));
			 DataOutputStream o = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[1]), 1 << 20))) {
			out = o;
			if (in.readInt() != 0x554E4431) {
				throw new IOException("not an underwater oracle input");
			}
			FluTypeList.types = new FluType[in.readUnsignedShort()];
			for (int id = 0; id < FluTypeList.types.length; id++) {
				FluType type = new FluType();
				byte[] data = bytes(in);
				if (data != null) {
					type.decode(id, new Buffer(data));
				}
				FluTypeList.types[id] = type;
			}
			FloTypeList.types = new FloType[in.readUnsignedShort()];
			for (int id = 0; id < FloTypeList.types.length; id++) {
				FloType type = new FloType();
				byte[] data = bytes(in);
				if (data != null) {
					type.decode(new Buffer(data), id);
				}
				FloTypeList.types[id] = type;
			}
			final int textures = in.readUnsignedShort();
			final int[] brightness = new int[textures];
			final int[] speed = new int[textures];
			final int[] material = new int[textures];
			for (int i = 0; i < textures; i++) {
				brightness[i] = in.readUnsignedByte();
				speed[i] = in.readUnsignedByte();
				material[i] = in.readUnsignedByte();
			}
			Rasteriser.textureProvider = new TextureProvider() {
				public int getTextureBrightness(int id) {
					return brightness[id];
				}

				public int getTextureSpeed(int id) {
					return speed[id];
				}

				public int getMaterialType(int id) {
					return material[id];
				}

				// Only the software tile colours setTile is handed read these two.
				public boolean isTextureRepeating(int id) {
					return true;
				}

				public int getAverageColor(int id) {
					return 0;
				}

				public boolean isOpaque(int id) {
					throw new UnsupportedOperationException();
				}

				public void bindTexture(int id) {
					throw new UnsupportedOperationException();
				}

				public int getAnimationType(int id) {
					throw new UnsupportedOperationException();
				}

				public boolean isTextureLoaded(int id) {
					throw new UnsupportedOperationException();
				}

				public boolean isTextureFlipped(int id) {
					throw new UnsupportedOperationException();
				}

				public int[] getAnimatedPixels(int id, float progress) {
					throw new UnsupportedOperationException();
				}

				public boolean isLowDetail(int id) {
					throw new UnsupportedOperationException();
				}

				public int[] getPixels(int id) {
					throw new UnsupportedOperationException();
				}
			};
			Rasteriser.oracleInit();
			int cases = in.readUnsignedByte();
			for (int c = 0; c < cases; c++) {
				scene(in);
			}
		}
	}

	private static void scene(DataInputStream in) throws Exception {
		String name = in.readUTF();
		// SceneGraph.centralZoneY, which decides whether regions without
		// terrain are cleared.
		int centralZoneY = in.readInt();

		// SceneGraph.init(.., true) and LoginManager.rebuildMap's reset.
		SceneGraph.width = 104;
		SceneGraph.length = 104;
		SceneGraph.surfaceGroundTiles = new Tile[4][104][104];
		SceneGraph.surfaceTileHeights = new int[4][105][105];
		SceneGraph.surfaceHdTiles = new GlTile[4][];
		SceneGraph.underWaterGroundTiles = new Tile[1][104][104];
		SceneGraph.tileWaterData = new int[104][104];
		SceneGraph.underwaterTileHeights = new int[1][105][105];
		SceneGraph.underWaterHdTiles = new GlTile[1][];
		SceneGraph.setUnderwater(false);
		SceneGraph.scenery = new Scenery[5000];
		SceneGraph.sceneryLen = 0;
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					SceneGraph.renderFlags[level][x][y] = 0;
				}
			}
		}

		// The surface as readStaticTerrain(false) would have left it, ground
		// level only: nothing below reads another level.
		SceneGraph.load(false);
		grid(in, SceneGraph.tileUnderlays[0]);
		grid(in, SceneGraph.tileOverlays[0]);
		grid(in, SceneGraph.tileShapes[0]);
		grid(in, SceneGraph.tileAngles[0]);
		grid(in, SceneGraph.renderFlags[0]);
		grid(in, SceneGraph.renderFlags[1]);
		for (int x = 0; x < 105; x++) {
			for (int y = 0; y < 105; y++) {
				SceneGraph.surfaceTileHeights[0][x][y] = in.readInt();
			}
		}
		// buildScene(.., false) for level 0, as far as the tile loop.
		int[][] underlayColors = new int[104][104];
		int[][] tileLightness = new int[104][104];
		SceneGraph.oracleUnderlayColors(0, underlayColors);
		SceneGraph.oracleTiles(false, 0, underlayColors, tileLightness, 0, 0);
		section(name, "water0", ints(SceneGraph.tileWaterData, 104));
		SceneGraph.unload();

		int regions = in.readInt();
		int[] baseX = new int[regions];
		int[] baseY = new int[regions];
		byte[][] terrain = new byte[regions][];
		byte[][] locations = new byte[regions][];
		for (int i = 0; i < regions; i++) {
			baseX[i] = in.readInt();
			baseY[i] = in.readInt();
			terrain[i] = bytes(in);
			locations[i] = bytes(in);
		}

		LocTypeList.types.clear();
		int types = in.readInt();
		for (int i = 0; i < types; i++) {
			LocType type = new LocType();
			type.id = in.readInt();
			type.width = in.readInt();
			type.length = in.readInt();
			int flags = in.readUnsignedByte();
			type.render = (flags & 1) != 0;
			type.interactable = (flags & 2) != 0 ? 1 : 0;
			type.forcedecor = (flags & 4) != 0;
			type.blockwalk = (flags & 8) != 0 ? 1 : 2;
			type.computeVertexColors = (flags & 16) != 0;
			// A type whose scene entity is a Loc: animated or multi-location.
			type.anim = (flags & 32) != 0 ? 0 : -1;
			type.hillskewType = in.readByte();
			type.hillskewAmount = in.readShort();
			int models = in.readUnsignedShort();
			for (int m = 0; m < models; m++) {
				int key = in.readUnsignedShort();
				ModelRecord record = new ModelRecord();
				int vertices = in.readInt();
				record.x = new int[vertices];
				record.y = new int[vertices];
				record.z = new int[vertices];
				for (int v = 0; v < vertices; v++) {
					record.x[v] = in.readInt();
					record.y[v] = in.readInt();
					record.z[v] = in.readInt();
				}
				// Nothing merges under water: no normals are needed.
				record.normalX = new short[0];
				record.normalY = new short[0];
				record.normalZ = new short[0];
				record.normalMagnitude = new short[0];
				record.vertexOffsets = new int[vertices + 1];
				record.vertexLookup = new short[0];
				record.triangleCount = 0;
				type.models.put(key, record);
			}
			LocTypeList.types.put(type.id, type);
		}

		// LoginManager.rebuildMap with hasUnderWaterMap.
		SceneGraph.setUnderwater(true);
		SceneGraph.load(true);
		// LoginManager.readStaticTerrain(true): every file, then the regions
		// without one.
		for (int i = 0; i < regions; i++) {
			if (terrain[i] != null) {
				// SceneGraph.readTerrain's tile loop for one level.
				Buffer buffer = new Buffer(terrain[i]);
				for (int y = 0; y < 64; y++) {
					for (int z = 0; z < 64; z++) {
						SceneGraph.readTile(0, 0, true, buffer, z + baseY[i], baseX[i] + y, 0, 0);
					}
				}
				if (buffer.offset != buffer.data.length) {
					throw new IOException("the underwater map of " + name + " has data after its tiles");
				}
			}
		}
		for (int i = 0; i < regions; i++) {
			if (terrain[i] == null && centralZoneY < 800) {
				SceneGraph.clearTerrainRegion(0, baseY[i], baseX[i], 64, 64);
			}
		}
		section(name, "heights", ints(SceneGraph.underwaterTileHeights[0], 105));
		ByteArrayOutputStream buffer = new ByteArrayOutputStream();
		for (byte[][] grid : new byte[][][]{SceneGraph.tileUnderlays[0], SceneGraph.tileOverlays[0], SceneGraph.tileShapes[0], SceneGraph.tileAngles[0]}) {
			for (byte[] column : grid) {
				buffer.write(column);
			}
		}
		section(name, "tiles", buffer);

		// LoginManager.readStaticLocs(true).
		CollisionMap[] maps = new CollisionMap[]{new CollisionMap(), new CollisionMap(), new CollisionMap(), new CollisionMap()};
		for (int i = 0; i < regions; i++) {
			if (locations[i] != null) {
				SceneGraph.readLocs(baseX[i], true, locations[i], baseY[i], maps);
			}
		}
		section(name, "water1", ints(SceneGraph.tileWaterData, 104));
		buffer = new ByteArrayOutputStream();
		for (int x = 0; x < 105; x++) {
			buffer.write(SceneGraph.shadowmap[0][x]);
		}
		section(name, "shadow", buffer);

		// SceneGraph.buildScene(.., true).
		underlayColors = new int[104][104];
		tileLightness = new int[104][104];
		SceneGraph.oracleTileLightness(SceneGraph.shadowmap[0], tileLightness);
		section(name, "light", ints(tileLightness, 104));
		SceneGraph.oracleUnderlayColors(0, underlayColors);
		section(name, "colors", ints(underlayColors, 104));
		SceneGraph.oracleTiles(true, 0, underlayColors, tileLightness, 0, 0);
		float[][] normalsX = new float[105][105];
		float[][] normalsY = new float[105][105];
		float[][] normalsZ = new float[105][105];
		SceneGraph.oracleNormals(SceneGraph.tileHeights[0], normalsX, normalsY, normalsZ);
		GlTile[] plain = SceneGraph.buildGlPlainTiles(SceneGraph.renderFlags, SceneGraph.tileShapes[0], SceneGraph.tileUnderlays[0], tileLightness, normalsX, SceneGraph.tileWaterData, SceneGraph.tileOverlays[0], SceneGraph.tileAngles[0], normalsY, 0, normalsZ, underlayColors, SceneGraph.tileHeights[0], SceneGraph.surfaceTileHeights[0]);
		buffer = new ByteArrayOutputStream();
		tiles(new DataOutputStream(buffer), plain);
		section(name, "plain", buffer);
		buffer = new ByteArrayOutputStream();
		for (int x = 0; x < 104; x++) {
			for (int y = 0; y < 104; y++) {
				// buildGlPlainTiles stores its diagonal into tileAngles.
				buffer.write(SceneGraph.tileUnderlays[0][x][y] != 0 && SceneGraph.tileOverlays[0][x][y] == 0 && x >= 1 && x <= 102 && y >= 1 && y <= 102 ? SceneGraph.tileAngles[0][x][y] : 255);
			}
		}
		section(name, "diag", buffer);
		SceneGraph.mergeSceneNormals();

		buffer = new ByteArrayOutputStream();
		ByteArrayOutputStream exists = new ByteArrayOutputStream();
		DataOutputStream o = new DataOutputStream(buffer);
		for (int x = 0; x < 104; x++) {
			for (int y = 0; y < 104; y++) {
				Tile tile = SceneGraph.underWaterGroundTiles[0][x][y];
				exists.write(tile == null ? 0 : 1);
				if (tile == null) {
					continue;
				}
				if (tile.wall != null) {
					slot(o, 1, x, y);
					o.writeInt(tile.wall.zFine);
					entity(o, tile.wall.primary);
					entity(o, tile.wall.secondary);
				}
				if (tile.wallDecor != null) {
					slot(o, 4, x, y);
					o.writeInt(tile.wallDecor.xOffset);
					o.writeInt(tile.wallDecor.yOffset);
					// The height: addLoc passes averageY in the parameter the
					// deobfuscation named orientation (renderTile draws at it).
					o.writeInt(tile.wallDecor.orientation);
					entity(o, tile.wallDecor.primary);
					entity(o, tile.wallDecor.secondary);
				}
				for (int i = 0; i < tile.sceneryLen; i++) {
					Scenery s = tile.scenery[i];
					if (s.xMin == x && s.yMin == y) {
						slot(o, 2, x, y);
						o.writeInt(s.xMax);
						o.writeInt(s.yMax);
						o.writeInt(s.xFine);
						o.writeInt(s.yFine);
						o.writeInt(s.zFine);
						entity(o, s.entity);
					}
				}
				if (tile.groundDecor != null) {
					slot(o, 3, x, y);
					o.writeInt(tile.groundDecor.zFine);
					entity(o, tile.groundDecor.entity);
				}
			}
		}
		section(name, "exists", exists);
		section(name, "locs", buffer);
		SceneGraph.unload();
		SceneGraph.setUnderwater(false);
	}

	private static void slot(DataOutputStream o, int kind, int x, int y) throws IOException {
		o.writeByte(kind);
		o.writeByte(x);
		o.writeByte(y);
	}

	private static void entity(DataOutputStream o, Entity entity) throws IOException {
		if (entity == null) {
			o.writeByte(0);
		} else if (entity instanceof GlModel) {
			GlModel model = (GlModel) entity;
			o.writeByte(model.mergeable ? 3 : 1);
			o.writeInt(model.vertexCount);
			for (int i = 0; i < model.vertexCount; i++) {
				o.writeInt(model.vertexX[i]);
				o.writeInt(model.vertexY[i]);
				o.writeInt(model.vertexZ[i]);
			}
		} else {
			o.writeByte(2);
		}
	}

	private static Object field(GlTile tile, String name) throws Exception {
		Field field = GlTile.class.getDeclaredField(name);
		field.setAccessible(true);
		return field.get(tile);
	}

	private static void tiles(DataOutputStream o, GlTile[] tiles) throws Exception {
		o.writeInt(tiles.length);
		for (GlTile tile : tiles) {
			o.writeLong(tile.key);
			o.writeInt(tile.texture);
			o.writeBoolean(tile.blend);
			o.writeBoolean((Boolean) field(tile, "underwater"));
			o.writeInt(tile.underwaterColor);
			o.writeInt(tile.vertexCount);
			// The bytes uploadVertexData packed: position, colour, normal,
			// texture coordinate and underwater coordinate of every vertex.
			o.write(((GlVertexBufferObject) field(tile, "vertexVbo")).data);
			int faces = (Integer) field(tile, "faceCount");
			int[] plane = (int[]) field(tile, "facePlane");
			int[] tileX = (int[]) field(tile, "faceTileX");
			int[] tileY = (int[]) field(tile, "faceTileY");
			int[][] vertices = (int[][]) field(tile, "faceVertexIndices");
			o.writeInt(faces);
			for (int f = 0; f < faces; f++) {
				o.writeInt(plane[f]);
				o.writeInt(tileX[f]);
				o.writeInt(tileY[f]);
				o.writeInt(vertices[f].length);
				for (int index : vertices[f]) {
					o.writeInt(index);
				}
			}
		}
	}

	private static ByteArrayOutputStream ints(int[][] grid, int size) throws IOException {
		ByteArrayOutputStream buffer = new ByteArrayOutputStream();
		DataOutputStream o = new DataOutputStream(buffer);
		for (int x = 0; x < size; x++) {
			for (int y = 0; y < size; y++) {
				o.writeInt(grid[x][y]);
			}
		}
		return buffer;
	}

	private static void grid(DataInputStream in, byte[][] grid) throws IOException {
		for (byte[] column : grid) {
			in.readFully(column);
		}
	}

	private static byte[] bytes(DataInputStream in) throws IOException {
		int length = in.readInt();
		if (length < 0) {
			return null;
		}
		byte[] data = new byte[length];
		in.readFully(data);
		return data;
	}

	private static void section(String scene, String name, ByteArrayOutputStream data) throws IOException {
		out.writeUTF(scene + "/" + name);
		out.writeInt(data.size());
		data.writeTo(out);
	}
}
