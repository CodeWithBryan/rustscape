import java.lang.reflect.Field;
import java.nio.ByteBuffer;

import rt4.FloType;
import rt4.FloTypeList;
import rt4.FogManager;
import rt4.GlRenderer;
import rt4.GlVertexBufferObject;
import rt4.Light;
import rt4.LightMesh;
import rt4.LightingManager;
import rt4.Scenery;
import rt4.SceneGraph;
import rt4.ShadowManager;
import rt4.SoftwareIndexedSprite;
import rt4.Sprites;
import rt4.Tile;

/**
 * Drives the unchanged LightingManager, LightMesh and ShadowManager, and the
 * SceneGraph methods the build extracts unchanged, over seeded inputs. The Rust
 * test regenerates the same inputs from the seeds printed here, so the two
 * generators must draw in the same order.
 */
public final class LightingOracle {
	static final class Rng {
		int state;

		Rng(int seed) {
			this.state = seed;
		}

		int next(int bound) {
			this.state = this.state * 1103515245 + 12345;
			return (this.state >>> 16 & 0x7fff) % bound;
		}
	}

	static long hash;

	static void mix(int value) {
		for (int i = 0; i < 4; i++) {
			hash ^= value >>> i * 8 & 0xff;
			hash *= 0x100000001b3L;
		}
	}

	static Light light(Rng rng, int index, int origin, int extent) {
		Light light = new Light();
		light.level = rng.next(4);
		light.extendsDown = rng.next(4) == 0;
		light.extendsUp = rng.next(4) == 0;
		light.radius = rng.next(5);
		light.x = (origin + rng.next(extent)) * 128 + rng.next(128);
		light.y = (origin + rng.next(extent)) * 128 + rng.next(128);
		light.z = rng.next(600) - 300;
		light.spanProfile = new short[light.radius * 2 + 1];
		for (int i = 0; i < light.spanProfile.length; i++) {
			int start = rng.next(8) == 0 ? -1 : rng.next(3) == 0 ? rng.next(light.radius * 2 + 1) : 0;
			int length = rng.next(3) == 0 ? rng.next(light.radius * 2 + 2) : light.radius * 2 + 1 - Math.max(start, 0);
			light.spanProfile[i] = (short) (start << 8 | length);
		}
		light.color = rng.next(4096) << 12 | rng.next(4096);
		light.quadraticAttenuation = (float) index;
		return light;
	}

	static int near(Rng rng, Light light, int fine) {
		int tile = (fine >> 7) + rng.next(light.radius * 2 + 3) - light.radius - 1;
		return Math.max(0, Math.min(103, tile));
	}

	static void select(int seed) throws Exception {
		Rng rng = new Rng(seed);
		int count = 1 + rng.next(60);
		LightingManager.initLightGrid();
		LightingManager.allocateLightArrays();
		LightingManager.clearLightGrid();
		Light[] lights = new Light[count];
		for (int i = 0; i < count; i++) {
			lights[i] = rng.next(3) == 0 ? light(rng, i, -3, 110) : light(rng, i, 40, 16);
			LightingManager.addLight(lights[i]);
		}
		LightingManager.buildLightGrid();
		LightingManager.initGlLightParameters();
		LightingManager.updateAllLightAnimations(0, false);
		Field activeField = LightingManager.class.getDeclaredField("activeLightIndices");
		activeField.setAccessible(true);
		int[] active = (int[]) activeField.get(null);
		StringBuilder log = GlRenderer.gl.log;
		log.setLength(0);
		StringBuilder out = new StringBuilder("select " + seed);
		int[] last = new int[3];
		for (int op = 0; op < 150; op++) {
			int kind = rng.next(10);
			Light target = lights[rng.next(count)];
			int plane = rng.next(4);
			int x = near(rng, target, target.x);
			int y = near(rng, target, target.y);
			if (kind <= 3) {
				last = new int[]{plane, x, y};
				LightingManager.updateLightsForTile(0, 0, 0, plane, x, y);
			} else if (kind <= 5) {
				int endX = Math.min(103, x + rng.next(4));
				int endY = Math.min(103, y + rng.next(4));
				LightingManager.updateLightsForRegion(0, 0, 0, plane, x, y, endX, endY);
			} else if (kind == 6) {
				int flags = rng.next(10) == 0 ? 3 : 1 << rng.next(8);
				LightingManager.updateLightsForWallPiece(flags, 0, 0, 0, plane, x, y);
			} else if (kind == 7) {
				int x2 = near(rng, target, target.x);
				int y2 = near(rng, target, target.y);
				LightingManager.pruneInactiveLights(plane, x, y, x2, y2);
			} else if (kind == 8) {
				if (rng.next(4) == 0) {
					LightingManager.resetActiveLights();
				} else {
					LightingManager.updateAllLightAnimations(1, false);
				}
			} else {
				LightingManager.updateLightsForTile(0, 0, 0, last[0], last[1], last[2]);
			}
			out.append(" |").append(log).append(" ;");
			for (int slot = 0; slot < 4; slot++) {
				out.append(slot == 0 ? " " : ",").append(active[slot]);
			}
			log.setLength(0);
		}
		System.out.println(out);
	}

	static void mesh(int seed) throws Exception {
		Rng rng = new Rng(seed);
		int level = rng.next(4);
		int[][] heights = new int[105][105];
		for (int x = 0; x < 105; x++) {
			for (int y = 0; y < 105; y++) {
				heights[x][y] = -rng.next(20) * 8;
			}
		}
		byte[][] underlays = new byte[104][104], overlays = new byte[104][104];
		byte[][] shapes = new byte[104][104], angles = new byte[104][104];
		SceneGraph.tiles = new Tile[4][104][104];
		for (int x = 0; x < 104; x++) {
			for (int y = 0; y < 104; y++) {
				underlays[x][y] = (byte) rng.next(4);
				int overlay = rng.next(5);
				overlays[x][y] = (byte) (overlay < 3 ? 0 : overlay - 2);
				shapes[x][y] = (byte) rng.next(12);
				angles[x][y] = (byte) rng.next(4);
				if (rng.next(12) == 0) {
					Tile tile = new Tile();
					if (rng.next(6) == 0) {
						Scenery other = new Scenery();
						other.key = 2L << 29 | 9L << 14 | (long) rng.next(4) << 20;
						other.xMin = x - 1;
						other.yMin = y;
						tile.scenery[tile.sceneryLen++] = other;
					}
					Scenery scenery = new Scenery();
					long shape = rng.next(4) == 0 ? 10 : 9;
					scenery.key = 2L << 29 | shape << 14 | (long) rng.next(4) << 20;
					scenery.xMin = x;
					scenery.yMin = y;
					tile.scenery[tile.sceneryLen++] = scenery;
					SceneGraph.tiles[level][x][y] = tile;
				}
			}
		}
		FloTypeList.types = new FloType[]{new FloType(), new FloType()};
		FloTypeList.types[0].baseColor = 100;
		FloTypeList.types[1].baseColor = -1;
		int count = 16;
		Light[] lights = new Light[count];
		for (int i = 0; i < count; i++) {
			lights[i] = light(rng, i, -3, 110);
			if (rng.next(4) != 0) {
				lights[i].level = level;
			}
			// The loader leaves a height in y: above the terrain it is negative, and
			// only footprints reaching the scene's first rows are then meshed.
			if (rng.next(3) != 0) {
				lights[i].y = -rng.next(300) - 1;
				lights[i].z = rng.next(500);
			}
		}
		// SceneGraph.buildScene lines 1083-1116: the level's corner normals.
		float[][] normalsY = new float[105][105];
		float[][] normalsX = new float[105][105];
		float[][] normalsZ = new float[105][105];
		for (int m = 1; m <= 103; m++) {
			for (int n = 1; n <= 103; n++) {
				int z = heights[n][m + 1] - heights[n][m - 1];
				int p = heights[n + 1][m] - heights[n - 1][m];
				float normalLen = (float) Math.sqrt(p * p + z * z + 65536);
				normalsY[n][m] = (float) p / normalLen;
				normalsX[n][m] = -256.0F / normalLen;
				normalsZ[n][m] = (float) z / normalLen;
			}
		}
		// The call at SceneGraph.buildScene line 1106, argument for argument.
		SceneGraph.buildGlLighting(normalsZ, underlays, angles, lights, level, count, normalsX, shapes, overlays, heights, normalsY);
		Field vertexField = LightMesh.class.getDeclaredField("vertexVbo");
		Field indexField = LightMesh.class.getDeclaredField("indexVbo");
		vertexField.setAccessible(true);
		indexField.setAccessible(true);
		StringBuilder out = new StringBuilder("mesh " + seed);
		for (int i = 0; i < count; i++) {
			LightMesh mesh = lights[i].mesh;
			if (mesh == null) {
				out.append(" -");
				continue;
			}
			ByteBuffer vertices = ((GlVertexBufferObject) vertexField.get(mesh)).data;
			ByteBuffer indices = ((GlVertexBufferObject) indexField.get(mesh)).data;
			hash = 0xcbf29ce484222325L;
			StringBuilder full = new StringBuilder();
			for (int v = 0; v < mesh.vertexCount; v++) {
				for (int c = 0; c < 3; c++) {
					int channel = vertices.get(v * 16 + c) & 0xff;
					mix(channel);
					full.append(' ').append(channel);
				}
				for (int c = 0; c < 3; c++) {
					int position = (int) vertices.getFloat(v * 16 + 4 + c * 4);
					mix(position);
					full.append(' ').append(position);
				}
			}
			full.append(" /");
			for (int n = 0; n < mesh.indexCount; n++) {
				int index = indices.getInt(n * 4);
				mix(index);
				full.append(' ').append(index);
			}
			out.append(' ').append(mesh.vertexCount).append(':').append(mesh.indexCount).append(':').append(Long.toHexString(hash));
			if (seed % 8 == 0) {
				System.out.println("meshdump " + seed + " " + i + full);
			}
		}
		System.out.println(out);
	}

	static void shadow(int seed) {
		Rng rng = new Rng(seed);
		FogManager.lightX = rng.next(600) - 300;
		FogManager.lightZ = rng.next(600) - 300;
		Sprites.floorShadows = new SoftwareIndexedSprite[13];
		for (int i = 0; i < 13; i++) {
			Sprites.floorShadows[i] = new SoftwareIndexedSprite(16, 16, 0);
			for (int p = 0; p < 256; p++) {
				Sprites.floorShadows[i].pixels[p] = (byte) rng.next(2);
			}
		}
		ShadowManager.init();
		for (int i = 0; i < 400; i++) {
			int shape = rng.next(13);
			int orientation = rng.next(4);
			boolean flag1 = rng.next(2) == 0;
			boolean flag2 = rng.next(2) == 0;
			int tileX = rng.next(104);
			int tileY = rng.next(104);
			int base = -rng.next(700);
			ShadowManager.addFloorShadow(shape, orientation, flag1, flag2, tileX, tileY, base - rng.next(16), base - rng.next(16), base - rng.next(16), base - rng.next(16));
		}
		hash = 0xcbf29ce484222325L;
		int covered = 0;
		for (byte pixel : ShadowManager.shadowMapImage.pixels) {
			mix(pixel & 0xff);
			if (pixel != 0) {
				covered++;
			}
		}
		StringBuilder out = new StringBuilder("shadow " + seed + " " + covered + " " + Long.toHexString(hash) + " ");
		for (int i = 0; i < 80; i++) {
			SoftwareIndexedSprite sprite = new SoftwareIndexedSprite(1 + rng.next(40), 1 + rng.next(40), 0);
			sprite.xOffset = rng.next(9) - 4;
			sprite.yOffset = rng.next(9) - 4;
			int fineX = rng.next(104 * 128);
			int fineZ = rng.next(104 * 128);
			int height = -rng.next(700);
			try {
				out.append(ShadowManager.isObjectInShadow(sprite, fineX, height, fineZ) ? 'T' : 'F');
			} catch (ArrayIndexOutOfBoundsException e) {
				out.append('X');
			}
		}
		System.out.println(out);
	}

	public static void main(String[] args) throws Exception {
		for (int seed = 1; seed <= 40; seed++) {
			select(seed);
		}
		for (int seed = 1; seed <= 24; seed++) {
			mesh(seed);
		}
		for (int seed = 1; seed <= 12; seed++) {
			shadow(seed);
		}
	}
}
