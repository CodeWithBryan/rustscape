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
 * Runs the original HD floor builders, the location shade and the normal
 * merging over scenes described by an input file, and writes what they
 * produced as named sections.
 *
 * <p>The scene data (tile arrays, heights, blended underlay colours, location
 * types, landscape streams and location models) comes from the Rust decoders.
 * Everything computed from it here is original code: the unchanged FluType,
 * FloType, ColorUtils, GlTile, HashTable, ArrayUtils, Tile, Wall, Scenery and
 * GroundDecor classes and the method bodies the generator copies out of
 * SceneGraph, GlModel and Rasteriser.
 */
public final class TerrainOracle {
	private static DataOutputStream out;

	public static void main(String[] args) throws Exception {
		try (DataInputStream in = new DataInputStream(new BufferedInputStream(new FileInputStream(args[0]), 1 << 20));
			 DataOutputStream o = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(args[1]), 1 << 20))) {
			out = o;
			if (in.readInt() != 0x54455231) {
				throw new IOException("not a terrain oracle input");
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
			for (int i = 0; i < textures; i++) {
				brightness[i] = in.readUnsignedByte();
				speed[i] = in.readUnsignedByte();
			}
			Rasteriser.textureProvider = new TextureProvider() {
				public int getTextureBrightness(int id) {
					return brightness[id];
				}

				public int getTextureSpeed(int id) {
					return speed[id];
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

				public int getAverageColor(int id) {
					throw new UnsupportedOperationException();
				}

				public int[] getPixels(int id) {
					throw new UnsupportedOperationException();
				}

				public boolean isTextureRepeating(int id) {
					throw new UnsupportedOperationException();
				}

				public int getMaterialType(int id) {
					throw new UnsupportedOperationException();
				}
			};
			Rasteriser.oracleInit();

			int clears = in.readInt();
			ByteArrayOutputStream cleared = new ByteArrayOutputStream();
			for (int i = 0; i < clears; i++) {
				SceneGraph.shadowmap = new byte[4][105][105];
				SceneGraph.tileHeights = new int[4][105][105];
				SceneGraph.clearTerrainRegion(in.readInt(), in.readInt(), in.readInt(), in.readInt(), in.readInt());
				shadow(new DataOutputStream(cleared));
			}
			section("clear", "shadow", cleared);

			// Rasteriser.palette as it is without high detail lighting, where
			// it follows the brightness preference (0.7, preference 3, is the
			// palette of everything else here).
			int[] preferences = {1, 2, 4};
			float[] values = {0.9F, 0.8F, 0.6F};
			for (int i = 0; i < preferences.length; i++) {
				Rasteriser.oracleBrightness(values[i]);
				ByteArrayOutputStream palette = new ByteArrayOutputStream();
				DataOutputStream p = new DataOutputStream(palette);
				for (int color : Rasteriser.palette) {
					p.writeInt(color);
				}
				section("palette", "brightness" + preferences[i], palette);
			}
			Rasteriser.oracleBrightness(0.7F);

			int cases = in.readUnsignedByte();
			for (int c = 0; c < cases; c++) {
				scene(in);
			}
		}
	}

	private static void scene(DataInputStream in) throws Exception {
		String name = in.readUTF();
		byte[][][] underlays = new byte[4][104][104];
		byte[][][] overlays = new byte[4][104][104];
		byte[][][] shapes = new byte[4][104][104];
		byte[][][] angles = new byte[4][104][104];
		int[][][] heights = new int[4][105][105];
		int[][][] colors = new int[4][104][104];
		for (int level = 0; level < 4; level++) {
			grid(in, underlays[level]);
			grid(in, overlays[level]);
			grid(in, shapes[level]);
			grid(in, angles[level]);
			grid(in, SceneGraph.renderFlags[level]);
			for (int x = 0; x < 105; x++) {
				for (int y = 0; y < 105; y++) {
					heights[level][x][y] = in.readInt();
				}
			}
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					colors[level][x][y] = in.readInt();
				}
			}
		}
		byte[][][] anglesRead = new byte[4][104][];
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 104; x++) {
				anglesRead[level][x] = angles[level][x].clone();
			}
		}
		SceneGraph.tiles = new Tile[4][104][104];
		SceneGraph.tileHeights = heights;
		SceneGraph.surfaceTileHeights = heights;
		SceneGraph.underwaterTileHeights = null;
		SceneGraph.shadowmap = new byte[4][105][105];
		SceneGraph.occludeFlags = new int[4][105][105];
		SceneGraph.levels = 4;
		SceneGraph.width = 104;
		SceneGraph.length = 104;
		SceneGraph.firstVisibleLevel = 99;

		LocTypeList.types.clear();
		int types = in.readInt();
		for (int i = 0; i < types; i++) {
			LocType type = new LocType();
			type.id = in.readInt();
			type.width = in.readInt();
			type.length = in.readInt();
			int flags = in.readUnsignedByte();
			type.active = (flags & 1) != 0;
			type.computeVertexColors = (flags & 2) != 0;
			// A type whose scene entity is a Loc: animated or multi-location.
			type.anim = (flags & 4) != 0 ? 0 : -1;
			type.hillskewType = (byte) ((flags & 8) != 0 ? 1 : 0);
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
				int unique = in.readInt();
				record.normalX = new short[unique];
				record.normalY = new short[unique];
				record.normalZ = new short[unique];
				record.normalMagnitude = new short[unique];
				for (int u = 0; u < unique; u++) {
					record.normalX[u] = in.readShort();
					record.normalY[u] = in.readShort();
					record.normalZ[u] = in.readShort();
					record.normalMagnitude[u] = in.readShort();
				}
				// GlModel keeps one slot per face corner: the packed vertices
				// of a vertex first, then zeros.
				record.vertexOffsets = new int[vertices + 1];
				ByteArrayOutputStream lookup = new ByteArrayOutputStream();
				DataOutputStream slots = new DataOutputStream(lookup);
				int offset = 0;
				for (int v = 0; v < vertices; v++) {
					record.vertexOffsets[v] = offset;
					int used = in.readUnsignedShort();
					int capacity = in.readUnsignedShort();
					for (int s = 0; s < capacity; s++) {
						slots.writeShort(s < used ? in.readUnsignedShort() + 1 : 0);
					}
					offset += capacity;
				}
				record.vertexOffsets[vertices] = offset;
				byte[] packed = lookup.toByteArray();
				record.vertexLookup = new short[offset];
				for (int s = 0; s < offset; s++) {
					record.vertexLookup[s] = (short) ((packed[s * 2] & 0xFF) << 8 | packed[s * 2 + 1] & 0xFF);
				}
				record.triangleCount = in.readInt();
				type.models.put(key, record);
			}
			LocTypeList.types.put(type.id, type);
		}
		CollisionMap[] maps = new CollisionMap[]{new CollisionMap(), new CollisionMap(), new CollisionMap(), new CollisionMap()};
		int regions = in.readInt();
		for (int i = 0; i < regions; i++) {
			int baseX = in.readInt();
			int baseY = in.readInt();
			SceneGraph.readLocs(baseX, false, bytes(in), baseY, maps);
		}

		ByteArrayOutputStream buffer = new ByteArrayOutputStream();
		shadow(new DataOutputStream(buffer));
		section(name, "shadow", buffer);

		// buildScene allocates tileLightness once and reuses it for each level.
		int[][] tileLightness = new int[104][104];
		for (int level = 0; level < 4; level++) {
			SceneGraph.oracleTileLightness(SceneGraph.shadowmap[level], tileLightness);
			buffer = new ByteArrayOutputStream();
			DataOutputStream o = new DataOutputStream(buffer);
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					o.writeInt(tileLightness[x][y]);
				}
			}
			section(name, "light" + level, buffer);

			float[][] normalsX = new float[105][105];
			float[][] normalsY = new float[105][105];
			float[][] normalsZ = new float[105][105];
			SceneGraph.oracleNormals(heights[level], normalsX, normalsY, normalsZ);
			GlTile[] plain = SceneGraph.buildGlPlainTiles(SceneGraph.renderFlags, shapes[level], underlays[level], tileLightness, normalsX, null, overlays[level], angles[level], normalsY, level, normalsZ, colors[level], heights[level], null);
			GlTile[] shaped = SceneGraph.buildGlShapedTiles(normalsX, normalsY, heights[level], level, normalsZ, angles[level], tileLightness, shapes[level], underlays[level], overlays[level], SceneGraph.renderFlags);
			buffer = new ByteArrayOutputStream();
			tiles(new DataOutputStream(buffer), plain);
			section(name, "plain" + level, buffer);
			buffer = new ByteArrayOutputStream();
			tiles(new DataOutputStream(buffer), shaped);
			section(name, "shaped" + level, buffer);
			buffer = new ByteArrayOutputStream();
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					// buildGlPlainTiles stores its diagonal into tileAngles.
					buffer.write(underlays[level][x][y] != 0 && overlays[level][x][y] == 0 && x >= 1 && x <= 102 && y >= 1 && y <= 102 ? angles[level][x][y] : 255);
				}
			}
			section(name, "diag" + level, buffer);
		}

		SceneGraph.mergeSceneNormals();
		buffer = new ByteArrayOutputStream();
		DataOutputStream o = new DataOutputStream(buffer);
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 104; x++) {
				for (int y = 0; y < 104; y++) {
					Tile tile = SceneGraph.tiles[level][x][y];
					if (tile == null) {
						continue;
					}
					if (tile.wall != null) {
						slot(o, 1, level, x, y);
						entity(o, tile.wall.primary);
						entity(o, tile.wall.secondary);
					}
					for (int i = 0; i < tile.sceneryLen; i++) {
						Scenery s = tile.scenery[i];
						if (s.xMin == x && s.yMin == y) {
							slot(o, 2, level, x, y);
							o.writeInt(s.xMax);
							o.writeInt(s.yMax);
							entity(o, s.entity);
						}
					}
					if (tile.groundDecor != null) {
						slot(o, 3, level, x, y);
						entity(o, tile.groundDecor.entity);
					}
				}
			}
		}
		section(name, "merge", buffer);

		// High detail lighting off: buildScene's other tileLightness loop
		// under two light positions, and under the first the floor builders
		// again with the palette of brightness preference 4. The builders get
		// the tile angles as they were read (buildGlPlainTiles stores into them).
		int[][] suns = {{-50, -60, -50}, {-30, -50, 60}};
		for (int s = 0; s < suns.length; s++) {
			for (int axis = 0; axis < 3; axis++) {
				FogManager.light0Position[axis] = suns[s][axis];
			}
			for (int level = 0; level < 4; level++) {
				SceneGraph.oracleTileLightnessSd(level, SceneGraph.shadowmap[level], tileLightness);
				buffer = new ByteArrayOutputStream();
				o = new DataOutputStream(buffer);
				for (int x = 0; x < 104; x++) {
					for (int y = 0; y < 104; y++) {
						o.writeInt(tileLightness[x][y]);
					}
				}
				section(name, "sdlight" + s + "_" + level, buffer);
				if (s != 0) {
					continue;
				}
				Rasteriser.oracleBrightness(0.6F);
				byte[][] fresh = new byte[104][];
				for (int x = 0; x < 104; x++) {
					fresh[x] = anglesRead[level][x].clone();
				}
				float[][] normalsX = new float[105][105];
				float[][] normalsY = new float[105][105];
				float[][] normalsZ = new float[105][105];
				SceneGraph.oracleNormals(heights[level], normalsX, normalsY, normalsZ);
				GlTile[] plain = SceneGraph.buildGlPlainTiles(SceneGraph.renderFlags, shapes[level], underlays[level], tileLightness, normalsX, null, overlays[level], fresh, normalsY, level, normalsZ, colors[level], heights[level], null);
				GlTile[] shaped = SceneGraph.buildGlShapedTiles(normalsX, normalsY, heights[level], level, normalsZ, fresh, tileLightness, shapes[level], underlays[level], overlays[level], SceneGraph.renderFlags);
				Rasteriser.oracleBrightness(0.7F);
				buffer = new ByteArrayOutputStream();
				tiles(new DataOutputStream(buffer), plain);
				section(name, "sdplain" + level, buffer);
				buffer = new ByteArrayOutputStream();
				tiles(new DataOutputStream(buffer), shaped);
				section(name, "sdshaped" + level, buffer);
			}
		}
	}

	private static void slot(DataOutputStream o, int kind, int level, int x, int y) throws IOException {
		o.writeByte(kind);
		o.writeByte(level);
		o.writeByte(x);
		o.writeByte(y);
	}

	private static void entity(DataOutputStream o, Entity entity) throws IOException {
		if (entity == null) {
			o.writeByte(0);
		} else if (entity instanceof GlModel) {
			GlModel model = (GlModel) entity;
			o.writeByte(1);
			o.writeInt(model.vertexCount);
			for (int i = 0; i < model.vertexCount; i++) {
				o.writeInt(model.vertexX[i]);
				o.writeInt(model.vertexY[i]);
				o.writeInt(model.vertexZ[i]);
			}
			o.writeInt(model.uniqueVertexCount);
			for (int i = 0; i < model.uniqueVertexCount; i++) {
				o.writeShort(model.normalX[i]);
				o.writeShort(model.normalY[i]);
				o.writeShort(model.normalZ[i]);
				o.writeShort(model.normalMagnitude[i]);
			}
		} else {
			o.writeByte(2);
		}
	}

	private static void shadow(DataOutputStream o) throws IOException {
		for (int level = 0; level < 4; level++) {
			for (int x = 0; x < 105; x++) {
				o.write(SceneGraph.shadowmap[level][x]);
			}
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
			o.writeInt(tile.underwaterColor);
			o.writeInt(tile.vertexCount);
			// The bytes uploadVertexData packed: position, colour, normal and
			// texture coordinate of every vertex.
			o.write(((GlVertexBufferObject) field(tile, "vertexVbo")).data);
			int faces = (Integer) field(tile, "faceCount");
			int[] plane = (int[]) field(tile, "facePlane");
			int[] tileX = (int[]) field(tile, "faceTileX");
			int[] tileY = (int[]) field(tile, "faceTileY");
			int[][] vertices = (int[][]) field(tile, "faceVertexIndices");
			int[][] blendIndices = (int[][]) field(tile, "faceBlendIndices");
			boolean[] blend = (boolean[]) field(tile, "faceBlend");
			o.writeInt(faces);
			for (int f = 0; f < faces; f++) {
				o.writeInt(plane[f]);
				o.writeInt(tileX[f]);
				o.writeInt(tileY[f]);
				o.writeBoolean(blend != null && blend[f]);
				o.writeInt(vertices[f].length);
				for (int index : vertices[f]) {
					o.writeInt(index);
				}
				if (blendIndices == null || blendIndices[f] == null) {
					o.writeInt(-1);
				} else {
					o.writeInt(blendIndices[f].length);
					for (int index : blendIndices[f]) {
						o.writeInt(index);
					}
				}
			}
		}
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
