package rt4;

import org.openrs2.deob.annotation.OriginalArg;
import org.openrs2.deob.annotation.OriginalMember;
import org.openrs2.deob.annotation.Pc;

/**
 * Adapter: the fields Camera reads, with pieces of the original ScriptRunner
 * copied verbatim because the class cannot be compiled alone:
 * calculateViewportBounds (lines 142-204), projectToScreen (1263-1295) and the
 * follow-camera branch of renderGameScene (229-239, as followCamera). The four
 * edge computations of GlRenderer.setupPerspectiveView (lines 564-567 and
 * 582-585) are copied as perspectiveEdges, writing the Rasteriser fields here.
 */
public class ScriptRunner {
	public static short nearZoom = 256;
	public static short farZoom = 205;
	public static short minZoom = 1;
	public static short maxZoom = 32767;
	public static int viewportWidth = 0;
	public static int viewportHeight = 0;
	public static int viewportTop = 0;
	public static int viewportLeft = 0;
	public static int viewportZoom = 0;
	public static short maxFov = 32767;
	public static short minFov = 1;
	public static short minViewDistance = 256;
	public static short maxViewDistance = 320;
	public static int screenY = -1;
	public static int screenX = -1;
	public static int screenLowerX;
	public static int screenUpperX;
	public static int screenLowerY;
	public static int screenUpperY;
	@OriginalMember(owner = "client!ja", name = "a", descriptor = "(IIIIIZ)V")
	public static void calculateViewportBounds(@OriginalArg(0) int width, @OriginalArg(2) int top, @OriginalArg(3) int height, @OriginalArg(4) int left, @OriginalArg(5) boolean fillBackground) {
		if (width < 1) {
			width = 1;
		}
		if (height < 1) {
			height = 1;
		}
		if (GlRenderer.enabled) {
			@Pc(25) int heightDelta = height - 334;
			if (heightDelta < 0) {
				heightDelta = 0;
			} else if (heightDelta > 100) {
				heightDelta = 100;
			}
			@Pc(51) int zoom = heightDelta * (farZoom - nearZoom) / 100 + nearZoom;
			if (minZoom > zoom) {
				zoom = minZoom;
			} else if (maxZoom < zoom) {
				zoom = maxZoom;
			}
			@Pc(73) int fov = zoom * height * 512 / (width * 334);
			@Pc(115) int adjustedDim;
			@Pc(122) int padding;
			@Pc(86) short fovLimit;
			if (fov < minFov) {
				fovLimit = minFov;
				zoom = width * 334 * fovLimit / (height * 512);
				if (maxZoom < zoom) {
					zoom = maxZoom;
					adjustedDim = height * 512 * zoom / (fovLimit * 334);
					padding = (width - adjustedDim) / 2;
					if (fillBackground) {
						GlRaster.resetClipRegion();
						GlRaster.fillRect(left, top, padding, height, 0);
						GlRaster.fillRect(width + left - padding, top, padding, height, 0);
					}
					left += padding;
					width -= padding * 2;
				}
			} else if (maxFov < fov) {
				fovLimit = maxFov;
				zoom = fovLimit * width * 334 / (height * 512);
				if (minZoom > zoom) {
					zoom = minZoom;
					adjustedDim = fovLimit * width * 334 / (zoom * 512);
					padding = (height - adjustedDim) / 2;
					if (fillBackground) {
						GlRaster.resetClipRegion();
						GlRaster.fillRect(left, top, width, padding, 0);
						GlRaster.fillRect(left, top + height - padding, width, padding, 0);
					}
					height -= padding * 2;
					top += padding;
				}
			}
			viewportZoom = zoom * height / 334;
		}
		viewportWidth = (short) width;
		viewportHeight = (short) height;
		viewportTop = top;
		viewportLeft = left;
	}

	@OriginalMember(owner = "client!q", name = "a", descriptor = "(IIIIIIBI)V")
	public static void projectToScreen(@OriginalArg(0) int halfWidth, @OriginalArg(1) int zoomH, @OriginalArg(2) int yFine, @OriginalArg(3) int projHeight, @OriginalArg(4) int halfHeight, @OriginalArg(5) int xFine, @OriginalArg(7) int zoomV) {
		if (xFine < 128 || yFine < 128 || xFine > 13056 || yFine > 13056) {
			screenY = -1;
			screenX = -1;
			return;
		}
		@Pc(38) int heightAboveTile = SceneGraph.getTileHeight(Player.plane, xFine, yFine) - projHeight;
		@Pc(42) int dy = yFine - Camera.renderY;
		@Pc(46) int dz = heightAboveTile - Camera.renderZ;
		@Pc(50) int dx = xFine - Camera.renderX;
		@Pc(54) int sinPitch = MathUtils.sin[Camera.cameraPitch];
		@Pc(58) int cosPitch = MathUtils.cos[Camera.cameraPitch];
		@Pc(62) int sinYaw = MathUtils.sin[Camera.cameraYaw];
		@Pc(66) int cosYaw = MathUtils.cos[Camera.cameraYaw];
		@Pc(76) int rotatedX = dx * cosYaw + sinYaw * dy >> 16;
		@Pc(87) int rotatedZ = dy * cosYaw - sinYaw * dx >> 16;
		@Pc(89) int screenDx = rotatedX;
		@Pc(99) int screenDy = cosPitch * dz - rotatedZ * sinPitch >> 16;
		@Pc(113) int depth = rotatedZ * cosPitch + dz * sinPitch >> 16;
		if (depth < 50) {
			screenY = -1;
			screenX = -1;
		} else if (GlRenderer.enabled) {
			@Pc(150) int scaledHalfWidth = zoomH * 512 >> 8;
			screenX = scaledHalfWidth * screenDx / depth + halfWidth;
			@Pc(164) int scaledHalfHeight = zoomV * 512 >> 8;
			screenY = scaledHalfHeight * screenDy / depth + halfHeight;
		} else {
			screenX = (screenDx << 9) / depth + halfWidth;
			screenY = (screenDy << 9) / depth + halfHeight;
		}
	}

	public static void followCamera(int height) {
		@Pc(59) int savedRenderX;
		@Pc(57) int savedRenderZ;
		if (Camera.cameraType == 1) {
			savedRenderZ = Camera.yawDrift + (int) Camera.yawTarget & 0x7FF;
			savedRenderX = (int) Camera.pitchTarget;
			if (savedRenderX < Camera.maxPitchDistance / 256) {
				savedRenderX = Camera.maxPitchDistance / 256;
			}
			if (Camera.customCameraActive[4] && Camera.cameraAmplitude[4] + 128 > savedRenderX) {
				savedRenderX = Camera.cameraAmplitude[4] + 128;
			}
			Camera.calculateRenderPosition(Camera.cameraX, height, SceneGraph.getTileHeight(Player.plane, PlayerList.self.xFine, PlayerList.self.yFine) - 50, Camera.ZOOM - -(savedRenderX * 3), savedRenderZ, Camera.cameraY, savedRenderX);
		}
	}

	public static void perspectiveEdges(int x, int y, int width, int height, int centerX, int centerY, int scaleX, int scaleY) {
		@Pc(7) int screenLeft = (x - centerX << 8) / scaleX;
		@Pc(17) int screenRight = (x + width - centerX << 8) / scaleX;
		@Pc(25) int screenTop = (y - centerY << 8) / scaleY;
		@Pc(35) int screenBottom = (y + height - centerY << 8) / scaleY;
		screenLowerX = screenLeft;
		screenUpperX = screenRight;
		screenLowerY = screenTop;
		screenUpperY = screenBottom;
	}
}
