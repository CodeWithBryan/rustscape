package rt4;

/**
 * Adapter: GameShell.calcRenderDelta and mainInputLoop (lines 541-563) copied
 * verbatim because the class cannot be compiled alone.
 */
public class GameShell {
	public static long renderDelta = 0;
	public double calcRenderDelta(double value) {
		return value / 1_000_000_000.0d * (double) renderDelta * value;
	}

	public void mainInputLoop() {
		if (Keyboard.pressedKeys[Keyboard.KEY_UP] || Keyboard.pressedKeys[Keyboard.KEY_DOWN] || Keyboard.pressedKeys[Keyboard.KEY_LEFT] || Keyboard.pressedKeys[Keyboard.KEY_RIGHT]) {
			double vertical = calcRenderDelta(18.0d);
			if (Keyboard.pressedKeys[Keyboard.KEY_UP]) {
				Camera.pitchTarget += vertical;
			} else if (Keyboard.pressedKeys[Keyboard.KEY_DOWN]) {
				Camera.pitchTarget -= vertical;
			}

			double horizontal = calcRenderDelta(24.0d);
			if (Keyboard.pressedKeys[Keyboard.KEY_LEFT]) {
				Camera.yawTarget -= horizontal;
			} else if (Keyboard.pressedKeys[Keyboard.KEY_RIGHT]) {
				Camera.yawTarget += horizontal;
			}

			Camera.clampCameraAngle();
		}
	}
}
