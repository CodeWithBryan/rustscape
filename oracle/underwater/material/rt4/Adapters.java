package rt4;

import com.jogamp.opengl.GL2;

import java.nio.ByteBuffer;

// Oracle adapters for the underwater material differential: what the
// unchanged UnderwaterMaterialRenderer, WaterMaterialRenderer, MaterialManager
// and GlTile name besides OpenGL. GlRenderer's state setters only note that
// they were called, in order with the recorded GL calls. Nothing here contains
// original logic.

class GlRenderer {
	static GL2 gl;
	// A card with at least three texture units and 3D textures, the case the
	// client's shader reproduces.
	static int maxTextureUnits = 4;
	static boolean extTexture3dSupported = true;
	static int animationClock;
	// uploadVertexData writes big-endian floats and hands them to the VBO stub.
	static boolean bigEndian = true;
	static boolean arbVboSupported = true;

	static void setTextureId(int id) {
		UnderwaterMaterialOracle.note("GlRenderer.setTextureId", id);
	}

	static void setTextureCombineRgbMode(int mode) {
		UnderwaterMaterialOracle.note("GlRenderer.setTextureCombineRgbMode", mode);
	}

	static void setTextureCombineAlphaMode(int mode) {
		UnderwaterMaterialOracle.note("GlRenderer.setTextureCombineAlphaMode", mode);
	}

	static void resetTextureMatrix() {
		UnderwaterMaterialOracle.note("GlRenderer.resetTextureMatrix");
	}

	static void setLightingEnabled(boolean enabled) {
		UnderwaterMaterialOracle.note("GlRenderer.setLightingEnabled", enabled ? 1 : 0);
	}

	static void restoreLighting() {
		UnderwaterMaterialOracle.note("GlRenderer.restoreLighting");
	}

	static void disableDepthMask() {
		UnderwaterMaterialOracle.note("GlRenderer.disableDepthMask");
	}

	static void enableDepthMask() {
		UnderwaterMaterialOracle.note("GlRenderer.enableDepthMask");
	}

	static void setDepthLayer(float layer) {
		UnderwaterMaterialOracle.note("GlRenderer.setDepthLayer", layer);
	}
}

class GlCleaner {
	static int onCardTexture;
}

class Preferences {
	static boolean highWaterDetail = true;
	static boolean highDetailLighting = true;
}

class FogManager {
	static float ambient;
	static float diffuse;
	static int color;

	static float getLightingModelAmbient() {
		return ambient;
	}

	static float getLight0Diffuse() {
		return diffuse;
	}

	static int getLightColor() {
		return color;
	}
}

class SceneGraph {
	static GlTile[][] surfaceHdTiles;
	static Tile[][][] tiles;
}

class Tile {
	boolean visible;
}

class GlVertexBufferObject {
	void setArrayBuffer(ByteBuffer buffer) {
	}

	void bindArray() {
		UnderwaterMaterialOracle.note("GlVertexBufferObject.bindArray");
	}
}

class Rasteriser {
	static TextureProvider textureProvider;
}

/** The four renderers MaterialManager.init also constructs, inert here. */
abstract class InertRenderer implements MaterialRenderer {
	public void unbind() {
	}

	public void bind() {
	}

	public void setArgument(int argument) {
	}

	public int getFlags() {
		return 0;
	}
}

class SpecularMaterialRenderer extends InertRenderer {
}

class LiquidMaterialRenderer extends InertRenderer {
}

class WaterfallMaterialRenderer extends InertRenderer {
}

class UnlitMaterialRenderer extends InertRenderer {
}