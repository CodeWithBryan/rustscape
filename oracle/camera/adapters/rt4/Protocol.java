package rt4;

/**
 * Adapter: the camera's lines of the drift block in Protocol's main loop
 * (lines 2922-2969), copied with the minimap's lines between them left out
 * and Math.random() taken as an argument so the oracle can choose it.
 */
public class Protocol {
	public static int cameraYOffsetDelta = 2;
	public static int cameraJitterTimer = 0;
	public static int yawDriftDelta = 1;

	public static void cameraDrift(double random) {
		int modelId;
		cameraJitterTimer++;
		if (cameraJitterTimer > 500) {
			cameraJitterTimer = 0;
			modelId = (int) (random * 8.0D);
			if ((modelId & 0x4) == 4) {
				Camera.yawDrift += yawDriftDelta;
			}
			if ((modelId & 0x2) == 2) {
				Camera.cameraOffsetY += cameraYOffsetDelta;
			}
			if ((modelId & 0x1) == 1) {
				Camera.cameraOffsetX += Camera.offsetXDelta;
			}
		}
		if (Camera.cameraOffsetX < -50) {
			Camera.offsetXDelta = 2;
		}
		if (Camera.cameraOffsetY < -55) {
			cameraYOffsetDelta = 2;
		}
		if (Camera.cameraOffsetY > 55) {
			cameraYOffsetDelta = -2;
		}
		if (Camera.yawDrift < -40) {
			yawDriftDelta = 1;
		}
		if (Camera.cameraOffsetX > 50) {
			Camera.offsetXDelta = -2;
		}
		if (Camera.yawDrift > 40) {
			yawDriftDelta = -1;
		}
	}
}