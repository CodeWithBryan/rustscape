package rt4;

import com.jogamp.opengl.GL2;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Runs the unchanged UnderwaterMaterialRenderer, WaterMaterialRenderer,
 * MaterialManager and GlTile against a GL2 that records every call, and
 * prints the calls of each case as one JSON line.
 *
 * <p>Display lists are kept as the calls compiled into them and replayed in
 * place by glCallList, so a trace reads as the state changes a driver would
 * see. Nothing is rendered: what a combiner, a texture lookup or the fog
 * computes from this state is not observed.
 */
public final class UnderwaterMaterialOracle {
	private static final List<String> trace = new ArrayList<>();
	private static final Map<Integer, List<String>> lists = new HashMap<>();
	private static Integer compiling;
	private static int nextList = 1;
	private static int nextTexture = 1;

	static void note(String name, Object... args) {
		add(format(name, args));
	}

	private static void add(String call) {
		if (compiling != null) {
			lists.get(compiling).add(call);
		} else {
			trace.add(call);
		}
	}

	private static String number(float value) {
		if (Float.isNaN(value) || Float.isInfinite(value)) {
			return "\"" + value + "\"";
		}
		return Float.toString(value);
	}

	private static String format(String name, Object[] args) {
		StringBuilder text = new StringBuilder("{\"c\":\"").append(name).append("\",\"a\":[");
		for (int i = 0; args != null && i < args.length; i++) {
			if (i > 0) {
				text.append(',');
			}
			Object arg = args[i];
			if (arg instanceof Float) {
				text.append(number((Float) arg));
			} else if (arg instanceof float[]) {
				text.append('[');
				float[] values = (float[]) arg;
				for (int j = 0; j < values.length; j++) {
					text.append(j > 0 ? "," : "").append(number(values[j]));
				}
				text.append(']');
			} else if (arg instanceof int[]) {
				text.append('[');
				int[] values = (int[]) arg;
				for (int j = 0; j < values.length; j++) {
					text.append(j > 0 ? "," : "").append(values[j]);
				}
				text.append(']');
			} else if (arg instanceof ByteBuffer) {
				ByteBuffer buffer = ((ByteBuffer) arg).duplicate();
				if (buffer.remaining() <= 64) {
					text.append('[');
					for (int j = 0; buffer.hasRemaining(); j++) {
						text.append(j > 0 ? "," : "").append(buffer.get() & 0xFF);
					}
					text.append(']');
				} else {
					text.append("{\"bytes\":").append(buffer.remaining()).append('}');
				}
			} else {
				text.append(arg);
			}
		}
		return text.append("]}").toString();
	}

	private static void emit(String name, String extra) {
		StringBuilder line = new StringBuilder("{\"case\":\"").append(name).append('"').append(extra).append(",\"trace\":[");
		for (int i = 0; i < trace.size(); i++) {
			line.append(i > 0 ? "," : "").append(trace.get(i));
		}
		System.out.println(line.append("]}"));
		trace.clear();
	}

	private static String state() {
		return ",\"currentType\":" + MaterialManager.currentType + ",\"currentArg\":" + MaterialManager.currentArg;
	}

	public static void main(String[] args) {
		InvocationHandler recorder = new InvocationHandler() {
			public Object invoke(Object proxy, Method method, Object[] a) {
				String name = method.getName();
				if (name.equals("glGenLists")) {
					int first = nextList;
					nextList += (Integer) a[0];
					return first;
				}
				if (name.equals("glGenTextures")) {
					int[] ids = (int[]) a[1];
					for (int i = 0; i < (Integer) a[0]; i++) {
						ids[(Integer) a[2] + i] = nextTexture++;
					}
					return null;
				}
				if (name.equals("glNewList")) {
					compiling = (Integer) a[0];
					lists.put(compiling, new ArrayList<String>());
					return null;
				}
				if (name.equals("glEndList")) {
					compiling = null;
					return null;
				}
				if (name.equals("glCallList")) {
					for (String call : lists.get((Integer) a[0])) {
						add(call);
					}
					return null;
				}
				add(format(name, a));
				return null;
			}
		};
		GlRenderer.gl = (GL2) Proxy.newProxyInstance(GL2.class.getClassLoader(), new Class<?>[]{GL2.class}, recorder);
		Rasteriser.textureProvider = new TextureProvider() {
			public int getMaterialType(int id) {
				// Texture 5 is a water texture, texture 6 is not.
				return id == 5 ? 4 : 0;
			}

			public int getTextureBrightness(int id) {
				throw new UnsupportedOperationException();
			}

			public int getTextureSpeed(int id) {
				throw new UnsupportedOperationException();
			}

			public boolean isTextureRepeating(int id) {
				throw new UnsupportedOperationException();
			}

			public int getAverageColor(int id) {
				throw new UnsupportedOperationException();
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

		// MaterialManager.init: the noise volumes and the six renderers. The
		// underwater renderer makes its gradient texture and its two lists.
		MaterialManager.init();
		emit("init", ",\"use3DTexture\":" + UnderwaterMaterialRenderer.use3DTexture + ",\"secondaryUnit\":" + UnderwaterMaterialRenderer.getSecondaryTextureUnit());

		// The frame: camera, clock and atmosphere, then SceneGraph.setShapedTile
		// entering the underwater pass.
		FogManager.ambient = 1.1523438F;
		FogManager.diffuse = 0.69921875F;
		FogManager.color = 0xFFE0C0;
		GlRenderer.animationClock = 37;
		MaterialManager.setCameraTransform(300, 4000, -900, 5000, 700);
		WaterMaterialRenderer.setUnderwaterColor(0x122B3D);
		emit("colour-unbound", state());
		MaterialManager.setMaterial(-1, 3);
		emit("bind-floor", state());
		MaterialManager.renderingUnderwater = true;

		// While rendering under water no texture's material replaces it.
		MaterialManager.setMaterial(1, 4);
		MaterialManager.setMaterial(0, 0);
		MaterialManager.setMaterial(2, 5);
		MaterialManager.setMaterial(3, 1);
		emit("locked", state());

		// SceneGraph.renderTile for an underwater tile: its fog range, then
		// minus the surface height as the argument.
		MaterialManager.setUnderwaterFogRange(128);
		emit("range-128", state());
		MaterialManager.setMaterial(200, 3);
		emit("scenery-200", state());
		WaterMaterialRenderer.setUnderwaterColor(0x17281B);
		emit("colour-bound", state());
		MaterialManager.setUnderwaterFogRange(48);
		emit("range-48", state());
		GlRenderer.animationClock = 38;
		MaterialManager.setMaterial(0, 3);
		emit("scenery-0", state());

		// SceneGraph.setPlainTile leaving the pass.
		MaterialManager.renderingUnderwater = false;
		MaterialManager.setMaterial(0, 0);
		emit("leave", state());

		// UnderwaterMaterialRenderer.applyFogFade over one surface group of a
		// water texture with two faces, one above an underwater tile and one
		// not, and one group of another texture.
		SceneGraph.surfaceHdTiles = new GlTile[][]{{group(5, 0x122B3D), group(6, 0x445566)}};
		SceneGraph.tiles = new Tile[1][104][104];
		SceneGraph.tiles[0][1][1] = new Tile();
		SceneGraph.tiles[0][1][1].visible = true;
		UnderwaterMaterialRenderer.applyFogFade();
		emit("backdrop", ",\"viewDistance\":" + GlobalConfig.VIEW_DISTANCE + ",\"viewFadeDistance\":" + GlobalConfig.VIEW_FADE_DISTANCE);
	}

	/** A surface overlay group with a quad on tile (1, 1) and one on (2, 1). */
	private static GlTile group(int texture, int waterColor) {
		GlTile tile = new GlTile(texture, 128.0F, true, false, waterColor);
		tile.faceCapacity = 2;
		tile.vertexCapacity = 8;
		tile.allocateBuffers();
		for (int x = 1; x <= 2; x++) {
			int[] corners = new int[4];
			int[][] at = {{0, 0}, {128, 0}, {128, 128}, {0, 128}};
			for (int i = 0; i < 4; i++) {
				// Distinct colours keep the two quads' shared corners apart.
				corners[i] = tile.addVertex(x * 128 + at[i][0], -40, 128 + at[i][1], 0.0F, -1.0F, 0.0F, x << 8 | 0xFF, 0.0F);
			}
			tile.addFace(0, x, 1, corners, null, false);
		}
		tile.uploadVertexData();
		return tile;
	}
}