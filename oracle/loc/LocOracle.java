import java.io.*;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.*;
import java.security.MessageDigest;
import java.util.*;
import rt4.*;

/**
 * Runs the unchanged RT4 LocType, LocTypeList, GlModel, RawModel and Model
 * against the pinned cache with the GL renderer flagged on, and prints one
 * JSON object per observation:
 *
 *   definition  every location type's placement fields (non-default ones
 *               listed, all of them hashed)
 *   glmodel     LocType.getGlModel(rotation 0..7, shape) through reflection
 *               (the baked model) and through getAnimatedEntity without a
 *               sequence (the animated model, including the shape 10 turn)
 *   entity      getStaticEntity / getAnimatedEntity of types that have a
 *               hill skew, over uneven synthetic heights
 *   align       GlModel.alignToTerrain types 1..5 called directly
 *   multiloc    LocType.getMultiLoc over variable values
 *   sdcolour    GlModel.packInterleavedVertexData's colours of the glmodel
 *               cases with Preferences.highDetailLighting off, over three
 *               values of FogManager.light0Position and a palette that maps
 *               every index to itself
 *
 * The cache reader, texture flags, variables and the GL/software classes
 * named in oracle/loc/adapters are adapters; no GL context exists and no
 * shadow is projected.
 */
public final class LocOracle {
	private static final List<BufferedFile> OPEN = new ArrayList<BufferedFile>();
	private static final PrintStream OUT = new PrintStream(new FileOutputStream(FileDescriptor.out), false);

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

	/** The texture flags Js5GlTextureProvider's constructor reads from archive 26, in its order. */
	static final class Materials implements TextureProvider {
		final boolean[] defined, repeating, opaque, lowDetail, flipped;
		final byte[] speed, brightness, material, animation;
		Materials(byte[] data) {
			Buffer b = new Buffer(data);
			int n = b.g2();
			defined = new boolean[n]; repeating = new boolean[n]; opaque = new boolean[n]; lowDetail = new boolean[n]; flipped = new boolean[n];
			speed = new byte[n]; brightness = new byte[n]; material = new byte[n]; animation = new byte[n];
			for (int i = 0; i < n; i++) defined[i] = b.g1() == 1;
			for (int i = 0; i < n; i++) if (defined[i]) repeating[i] = b.g1() == 1;
			for (int i = 0; i < n; i++) if (defined[i]) opaque[i] = b.g1() == 1;
			for (int i = 0; i < n; i++) if (defined[i]) lowDetail[i] = b.g1() == 1;
			for (int i = 0; i < n; i++) if (defined[i]) flipped[i] = b.g1() == 1;
			for (int i = 0; i < n; i++) if (defined[i]) speed[i] = b.g1b();
			for (int i = 0; i < n; i++) if (defined[i]) brightness[i] = b.g1b();
			for (int i = 0; i < n; i++) if (defined[i]) material[i] = b.g1b();
			for (int i = 0; i < n; i++) if (defined[i]) animation[i] = b.g1b();
		}
		public boolean isOpaque(int id) {return opaque[id];}
		public void bindTexture(int id) {throw new UnsupportedOperationException();}
		public int getAnimationType(int id) {return animation[id] & 255;}
		public int getTextureSpeed(int id) {return speed[id] & 255;}
		public boolean isTextureLoaded(int id) {return true;}
		public boolean isTextureFlipped(int id) {return flipped[id];}
		public int[] getAnimatedPixels(int id, float progress) {throw new UnsupportedOperationException();}
		public boolean isLowDetail(int id) {return lowDetail[id];}
		public int getAverageColor(int id) {throw new UnsupportedOperationException();}
		public int[] getPixels(int id) {throw new UnsupportedOperationException();}
		public boolean isTextureRepeating(int id) {return repeating[id];}
		public int getMaterialType(int id) {return material[id] & 255;}
		public int getTextureBrightness(int id) {return brightness[id] & 255;}
	}

	static Object get(Object o, String name) throws Exception {
		Field f = o.getClass().getDeclaredField(name);
		f.setAccessible(true);
		return f.get(o);
	}

	static String hex(byte[] bytes) {
		StringBuilder out = new StringBuilder();
		for (byte value : bytes) out.append(String.format("%02x", value & 255));
		return out.toString();
	}

	static String sha(byte[] bytes) throws Exception {
		return hex(MessageDigest.getInstance("SHA-256").digest(bytes));
	}

	static Method GL_MODEL;

	static GlModel glModel(LocType type, int rotation, boolean animated, int shape) throws Exception {
		return (GlModel) GL_MODEL.invoke(type, rotation, animated, shape);
	}

	/** Vertices in the raw model's order. */
	static byte[] vertices(GlModel g) throws Exception {
		ByteArrayOutputStream bytes = new ByteArrayOutputStream();
		DataOutputStream out = new DataOutputStream(bytes);
		out.writeInt(g.vertexCount);
		for (int i = 0; i < g.vertexCount; i++) {
			out.writeInt(g.vertexX[i]);
			out.writeInt(g.vertexY[i]);
			out.writeInt(g.vertexZ[i]);
		}
		return bytes.toByteArray();
	}

	/**
	 * Vertices; then every GL triangle in GL order as three corners (raw vertex
	 * index and the corner's normal), colour, texture and alpha; then the two
	 * light intensities.
	 */
	static byte[] canonical(GlModel g) throws Exception {
		ByteArrayOutputStream bytes = new ByteArrayOutputStream();
		DataOutputStream out = new DataOutputStream(bytes);
		out.write(vertices(g));
		int triangles = (Integer) get(g, "triangleCount");
		int unique = (Integer) get(g, "uniqueVertexCount");
		short[] a = (short[]) get(g, "triangleVertexA"), b = (short[]) get(g, "triangleVertexB"), c = (short[]) get(g, "triangleVertexC");
		short[] nx = (short[]) get(g, "normalX"), ny = (short[]) get(g, "normalY"), nz = (short[]) get(g, "normalZ"), nm = (short[]) get(g, "normalMagnitude");
		short[] colors = (short[]) get(g, "triangleColors"), textures = (short[]) get(g, "triangleTextures");
		byte[] alpha = (byte[]) get(g, "triangleAlpha");
		short[] lookup = (short[]) get(g, "vertexLookup");
		int[] offsets = (int[]) get(g, "vertexOffsets");
		int[] source = new int[unique];
		Arrays.fill(source, -1);
		for (int i = 0; i < g.vertexCount; i++) {
			for (int at = offsets[i]; at < offsets[i + 1]; at++) {
				int index = (lookup[at] & 65535) - 1;
				if (index == -1) break;
				source[index] = i;
			}
		}
		out.writeInt(triangles);
		for (int i = 0; i < triangles; i++) {
			for (int v : new int[] {a[i] & 65535, b[i] & 65535, c[i] & 65535}) {
				out.writeInt(source[v]);
				out.writeInt(nx[v]);
				out.writeInt(ny[v]);
				out.writeInt(nz[v]);
				out.writeInt(nm[v]);
			}
			out.writeShort(colors[i]);
			out.writeShort(textures == null ? -1 : textures[i]);
			out.writeByte(alpha == null ? 0 : alpha[i]);
		}
		out.writeInt(g.getAmbientIntensity());
		out.writeInt(g.getDiffuseIntensity());
		return bytes.toByteArray();
	}

	/** GlModel.packInterleavedVertexData, which is private. */
	static Method PACK;

	/**
	 * Packs the colours alone, as the first draw does, and returns the packed
	 * colour (red, green, blue, alpha) of every GL triangle's three corners in
	 * GL order. Meant for high detail lighting off, when a colour holds the
	 * vertex's own lightness; Rasteriser.palette maps each index to itself.
	 */
	static byte[] colours(GlModel g) throws Exception {
		int triangles = (Integer) get(g, "triangleCount");
		short[] a = (short[]) get(g, "triangleVertexA"), b = (short[]) get(g, "triangleVertexB"), c = (short[]) get(g, "triangleVertexC");
		PACK.invoke(g, false, false, true, false, false);
		GlBuffer buffer = (GlBuffer) get(g, "colorBuffer");
		ByteArrayOutputStream bytes = new ByteArrayOutputStream();
		DataOutputStream out = new DataOutputStream(bytes);
		out.writeInt(triangles);
		for (int i = 0; i < triangles; i++) {
			for (int v : new int[] {a[i] & 65535, b[i] & 65535, c[i] & 65535}) {
				for (int k = 0; k < 4; k++) out.writeByte(buffer.buffer.get(buffer.pointer + v * buffer.stride + k));
			}
		}
		return bytes.toByteArray();
	}

	/** The generator both sides build the synthetic height arrays with. */
	static int seed;
	static int next() {
		seed = seed * 1103515245 + 12345;
		return (seed >>> 16) & 0x7fff;
	}
	/** kind 0: uneven; 1: level at -200; 2: gentle slope. */
	static int[][] heights(int kind, int lift) {
		int[][] h = new int[105][105];
		for (int x = 0; x < 105; x++) {
			for (int z = 0; z < 105; z++) {
				if (kind == 0) h[x][z] = -(next() % 96) * 8 - lift;
				else if (kind == 1) h[x][z] = -200 - lift;
				else h[x][z] = -(x * 5 + z * 3) - (next() % 4) - lift;
			}
		}
		return h;
	}

	static int base(int[][] h, int x, int z, int width, int length) {
		int west = x + (width >> 1), east = x + (width + 1 >> 1), south = z + (length >> 1), north = z + (length + 1 >> 1);
		return h[west][north] + h[east][south] + h[west][south] + h[east][north] >> 2;
	}

	static boolean has(LocType type, int shape) throws Exception {
		int[] shapes = (int[]) get(type, "shapes"), models = (int[]) get(type, "models");
		if (shapes == null) return shape == 10 && models != null && models.length > 0;
		for (int s : shapes) if (s == shape) return true;
		return false;
	}

	public static void main(String[] args) throws Exception {
		Path directory = Paths.get(args[0]);
		Js5 locs = archive(directory, 16), models = archive(directory, 7), textures = archive(directory, 26);
		LocTypeList.init(locs, models);
		Rasteriser.textureProvider = new Materials(textures.fetchFile(0, 0));
		GL_MODEL = LocType.class.getDeclaredMethod("getGlModel", int.class, boolean.class, int.class);
		GL_MODEL.setAccessible(true);

		// definition
		List<Integer> ids = new ArrayList<Integer>();
		for (int group = 0; group < locs.capacity(); group++) {
			int[] files = locs.getFileIds(group);
			if (files == null) continue;
			for (int f : files) ids.add(group << 8 | f);
		}
		Collections.sort(ids);
		MessageDigest all = MessageDigest.getInstance("SHA-256");
		int[] counts = new int[12];
		Map<Integer, LocType> types = new HashMap<Integer, LocType>();
		for (int id : ids) {
			LocType type = new LocType();
			type.id = id;
			byte[] data = locs.fetchFile(LocTypeList.getGroupId(id), LocTypeList.getFileId(id));
			if (data != null) type.decode(new Buffer(data));
			type.postDecode();
			if (type.breakroutefinding) {
				type.blockwalk = 0;
				type.blockrange = false;
			}
			types.put(id, type);
			int hill = (Byte) get(type, "hillskewType");
			String line = id + "," + type.walloff + "," + hill + "," + type.hillskewAmount + "," + (type.render ? 1 : 0) + "," + (type.clipped ? 1 : 0) + "," + (type.forcedecor ? 1 : 0) + "," + (type.dynamic ? 1 : 0) + "," + type.blockwalk + "," + type.interactable + "," + type.width + "," + type.length + "\n";
			all.update(line.getBytes("US-ASCII"));
			if (type.walloff != 16) counts[0]++;
			if (hill >= 1 && hill <= 5) counts[hill]++;
			if (type.render) counts[6]++;
			if (type.clipped) counts[7]++;
			if (type.forcedecor) counts[8]++;
			if (type.dynamic) counts[9]++;
			if (type.multiLocs != null) counts[10]++;
			if (type.hasanimation) counts[11]++;
			if (type.walloff != 16 || hill > 1 || type.render) {
				OUT.println("{\"case\":\"definition\",\"id\":" + id + ",\"walloff\":" + type.walloff + ",\"hill\":[" + hill + "," + type.hillskewAmount + "],\"render\":" + type.render + "}");
			}
		}
		OUT.println("{\"case\":\"definitions\",\"count\":" + ids.size() + ",\"sha256\":\"" + hex(all.digest()) + "\",\"walloff\":" + counts[0] + ",\"hill1\":" + counts[1] + ",\"hill2\":" + counts[2] + ",\"hill3\":" + counts[3] + ",\"hill4\":" + counts[4] + ",\"hill5\":" + counts[5] + ",\"render\":" + counts[6] + ",\"clipped\":" + counts[7] + ",\"forcedecor\":" + counts[8] + ",\"dynamic\":" + counts[9] + ",\"multiloc\":" + counts[10] + ",\"hasanimation\":" + counts[11] + "}");

		// glmodel: per shape, the first types of several kinds that have it.
		LinkedHashSet<Long> picks = new LinkedHashSet<Long>();
		for (int shape = 0; shape <= 22; shape++) {
			int[] quota = {2, 1, 1, 1, 1, 1};
			for (int id : ids) {
				LocType type = types.get(id);
				if (!has(type, shape)) continue;
				boolean mirror = (Boolean) get(type, "mirror");
				boolean resized = (Integer) get(type, "resizex") != 128 || (Integer) get(type, "resizey") != 128 || (Integer) get(type, "resizez") != 128;
				boolean moved = (Integer) get(type, "xoff") != 0 || (Integer) get(type, "yoff") != 0 || (Integer) get(type, "zoff") != 0;
				boolean recolored = get(type, "recol_s") != null, retextured = get(type, "retex_s") != null;
				int[] parts = (int[]) get(type, "models");
				int kind = mirror ? 1 : resized || moved ? 2 : recolored ? 3 : retextured ? 4 : get(type, "shapes") == null && parts.length > 1 ? 5 : 0;
				if (quota[kind] == 0) continue;
				quota[kind]--;
				picks.add((long) id << 8 | shape);
			}
		}
		int[][] level = heights(1, 0);
		for (long pick : picks) {
			int id = (int) (pick >> 8), shape = (int) (pick & 255);
			LocType type = LocTypeList.get(id);
			for (int rotation = 0; rotation < 8; rotation++) {
				String baked, animated;
				try {
					GlModel g = glModel(type, rotation, false, shape);
					baked = g == null ? "null" : "\"" + sha(canonical(g)) + "\"";
				} catch (Throwable e) {
					baked = "\"throws\"";
				}
				try {
					LocEntity entity = type.getAnimatedEntity(50 * 128 + 64, 50 * 128 + 64, null, -200, null, rotation, level, false, 0, null, 0, shape, 0);
					animated = entity == null || entity.model == null ? "null" : "\"" + sha(canonical((GlModel) entity.model)) + "\"";
				} catch (Throwable e) {
					animated = "\"throws\"";
				}
				OUT.println("{\"case\":\"glmodel\",\"id\":" + id + ",\"shape\":" + shape + ",\"rotation\":" + rotation + ",\"baked\":" + baked + ",\"animated\":" + animated + "}");
			}
			// A shape the type has no model for.
			int absent = -1;
			for (int s = 0; s <= 22 && absent == -1; s++) if (!has(type, s)) absent = s;
			if (absent != -1) {
				GlModel g = glModel(type, 0, false, absent);
				OUT.println("{\"case\":\"glmodel\",\"id\":" + id + ",\"shape\":" + absent + ",\"rotation\":0,\"baked\":" + (g == null ? "null" : "\"present\"") + ",\"animated\":null}");
			}
		}

		// entity: types with a hill skew through both entity builders.
		for (int hill = 1; hill <= 5; hill++) {
			int taken = 0;
			for (int id : ids) {
				LocType type = types.get(id);
				if ((Byte) get(type, "hillskewType") != hill || get(type, "models") == null) continue;
				int[] shapes = (int[]) get(type, "shapes");
				int shape = shapes == null ? 10 : shapes[0];
				// The baked entity keeps its vertices only for an interactable type.
				boolean keeps = type.interactable != 0 || type.dynamic;
				if (taken++ == 4) break;
				type = LocTypeList.get(id);
				for (int kind = 0; kind < 2; kind++) {
					seed = id * 31 + kind;
					int[][] ground = heights(kind, 0), above = heights(kind == 1 ? 2 : kind, 240);
					for (int rotation : shape == 10 ? new int[] {0, 1, 2, 3, 4, 5, 6, 7} : new int[] {0, 1, 2, 3}) {
						int width = (rotation & 1) != 0 ? type.length : type.width, length = (rotation & 1) != 0 ? type.width : type.length;
						for (int[] tile : new int[][] {{50, 50}, {0, 0}}) {
							int x = (width << 6) + (tile[0] << 7), z = (length << 6) + (tile[1] << 7);
							int height = base(ground, tile[0], tile[1], width, length);
							for (int upper = 0; upper < (hill >= 4 ? 2 : 1); upper++) {
								String baked = "null", animated;
								if (keeps) {
									try {
										LocEntity entity = type.getStaticEntity(rotation, x, ground, shape, height, upper == 0 ? above : null, false, null, false, z);
										baked = entity.model == null ? "null" : "\"" + sha(vertices((GlModel) entity.model)) + "\"";
									} catch (Throwable e) {
										baked = "\"throws\"";
									}
								}
								try {
									LocEntity entity = type.getAnimatedEntity(z, x, null, height, null, rotation, ground, false, 0, upper == 0 ? above : null, 0, shape, 0);
									animated = entity == null || entity.model == null ? "null" : "\"" + sha(vertices((GlModel) entity.model)) + "\"";
								} catch (Throwable e) {
									animated = "\"throws\"";
								}
								OUT.println("{\"case\":\"entity\",\"id\":" + id + ",\"shape\":" + shape + ",\"rotation\":" + rotation + ",\"heights\":" + kind + ",\"seed\":" + (id * 31 + kind) + ",\"tile\":[" + tile[0] + "," + tile[1] + "],\"above\":" + (upper == 0) + ",\"baked\":" + baked + ",\"animated\":" + animated + "}");
							}
						}
					}
				}
			}
		}

		// align: every type on a few real models, called directly.
		List<Long> references = new ArrayList<Long>();
		for (int shape : new int[] {10, 0, 22, 4, 9, 2}) {
			int taken = 0;
			for (long pick : picks) if ((pick & 255) == shape && taken++ < 1) references.add(pick);
		}
		int[][] amounts = {{0}, {0}, {0, 256, 2560, 16384, -32768, 1}, {0x2040, 0x0020, 0x4000, 0, -1}, {0}, {0}};
		for (long pick : references) {
			int id = (int) (pick >> 8), shape = (int) (pick & 255);
			LocType type = LocTypeList.get(id);
			for (int rotation : new int[] {0, 5}) {
				GlModel reference = glModel(type, rotation, false, shape);
				if (reference == null) continue;
				for (int kind = 0; kind < 2; kind++) {
					for (int hill = 1; hill <= 5; hill++) {
						for (int amount : amounts[hill]) {
							for (int[] tile : new int[][] {{50, 50}, {0, 0}, {103, 103}}) {
								for (int upper = 0; upper < (hill >= 4 ? 2 : 1); upper++) {
									int caseSeed = id * 131 + kind * 17 + hill * 5 + rotation;
									seed = caseSeed;
									int[][] ground = heights(kind, 0), above = heights(kind == 1 ? 2 : kind, 240);
									int x = 64 + (tile[0] << 7), z = 64 + (tile[1] << 7);
									int height = base(ground, tile[0], tile[1], 1, 1);
									String result;
									try {
										GlModel copy = (GlModel) reference.copyForLoc(true, true, true);
										copy.alignToTerrain(hill, amount, reference, ground, upper == 0 ? above : null, x, height, z);
										result = "\"" + sha(vertices(copy)) + "\"";
									} catch (ArithmeticException e) {
										result = "\"ArithmeticException\"";
									} catch (ArrayIndexOutOfBoundsException e) {
										result = "\"ArrayIndexOutOfBoundsException\"";
									}
									OUT.println("{\"case\":\"align\",\"id\":" + id + ",\"shape\":" + shape + ",\"rotation\":" + rotation + ",\"heights\":" + kind + ",\"seed\":" + caseSeed + ",\"type\":" + hill + ",\"amount\":" + amount + ",\"tile\":[" + tile[0] + "," + tile[1] + "],\"above\":" + (upper == 0) + ",\"vertices\":" + result + "}");
								}
							}
						}
					}
				}
			}
		}

		// multiloc
		int taken = 0;
		for (int id : ids) {
			LocType type = types.get(id);
			if (type.multiLocs == null) continue;
			// The first forty parents, then every 53rd.
			if (taken++ >= 40 && taken % 53 != 0) continue;
			int varbit = (Integer) get(type, "multiLocVarbit"), varp = (Integer) get(type, "multiLocVarp");
			int n = type.multiLocs.length;
			for (int value : new int[] {-1, 0, 1, 2, 3, n - 3, n - 2, n - 1, n, 255, Integer.MIN_VALUE, Integer.MAX_VALUE}) {
				VarpDomain.varbits.clear();
				Arrays.fill(VarpDomain.activeVarps, 0);
				if (varbit != -1) VarpDomain.varbits.put(varbit, value);
				else if (varp != -1) VarpDomain.activeVarps[varp] = value;
				String child;
				try {
					LocType chosen = type.getMultiLoc();
					child = chosen == null ? "-1" : Integer.toString(chosen.id);
				} catch (ArrayIndexOutOfBoundsException e) {
					child = "\"ArrayIndexOutOfBoundsException\"";
				}
				OUT.println("{\"case\":\"multiloc\",\"id\":" + id + ",\"varbit\":" + varbit + ",\"varp\":" + varp + ",\"value\":" + value + ",\"child\":" + child + "}");
			}
		}
		// sdcolour: the glmodel cases again with high detail lighting off, over
		// three light positions. Each model is built from emptied caches,
		// because packing stores every vertex's lightness in the normal-count
		// array a model shares with the cached one it was copied from.
		Preferences.highDetailLighting = false;
		for (int i = 0; i < 65536; i++) Rasteriser.palette[i] = i;
		PACK = GlModel.class.getDeclaredMethod("packInterleavedVertexData", boolean.class, boolean.class, boolean.class, boolean.class, boolean.class);
		PACK.setAccessible(true);
		int[][] suns = {{-50, -60, -50}, {-30, -50, 60}, {127, -8, -3}};
		for (long pick : picks) {
			int id = (int) (pick >> 8), shape = (int) (pick & 255);
			for (int rotation = 0; rotation < 8; rotation++) {
				for (int[] sun : suns) {
					for (int axis = 0; axis < 3; axis++) FogManager.light0Position[axis] = sun[axis];
					String baked, animated;
					try {
						LocTypeList.clear();
						GlModel g = glModel(LocTypeList.get(id), rotation, false, shape);
						baked = g == null ? "null" : "\"" + sha(colours(g)) + "\"";
					} catch (Throwable e) {
						baked = "\"throws\"";
					}
					try {
						LocTypeList.clear();
						LocEntity entity = LocTypeList.get(id).getAnimatedEntity(50 * 128 + 64, 50 * 128 + 64, null, -200, null, rotation, level, false, 0, null, 0, shape, 0);
						animated = entity == null || entity.model == null ? "null" : "\"" + sha(colours((GlModel) entity.model)) + "\"";
					} catch (Throwable e) {
						animated = "\"throws\"";
					}
					OUT.println("{\"case\":\"sdcolour\",\"id\":" + id + ",\"shape\":" + shape + ",\"rotation\":" + rotation + ",\"sun\":[" + sun[0] + "," + sun[1] + "," + sun[2] + "],\"baked\":" + baked + ",\"animated\":" + animated + "}");
				}
			}
		}
		OUT.flush();
		for (BufferedFile opened : OPEN) opened.close();
	}
}
