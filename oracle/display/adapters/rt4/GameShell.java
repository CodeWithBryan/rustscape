package rt4;

import java.awt.*;

/**
 * Oracle adapter: the GameShell statics DisplayMode reads and writes, with two methods copied from
 * reference/client/client/src/main/java/rt4/GameShell.java, whose class is an applet that cannot be
 * compiled alone.
 *
 * Both are copied without the deobfuscator's annotations. updateCanvasSize is lines 184-226, otherwise
 * as they stand. paintFrameLetterbox is lines 229-269 with the four
 * g.fillRect calls replaced by OracleLog.fill, the Graphics lookup dropped (a frame that is never shown
 * has none) and one OracleLog.add at its start; the arithmetic and the conditions are the original's.
 */
public class GameShell {
	public static SignLink signLink;
	public static Frame frame;
	public static Frame fullScreenFrame;
	public static Canvas canvas;
	public static int canvasWidth;
	public static int canvasHeight;
	public static int leftMargin = 0;
	public static int topMargin = 0;
	public static volatile boolean replaceCanvas = false;
	public static volatile boolean fullRedraw = true;
	public static int frameHeight;
	public static int frameWidth;
	public static Thread thread;

	public static void updateCanvasSize() {
		Container container;
		if (fullScreenFrame != null) {
			container = fullScreenFrame;
		} else if (frame == null) {
			container = signLink.applet;
		} else {
			container = frame;
		}
		frameWidth = container.getSize().width;
		frameHeight = container.getSize().height;
		Insets insets;
		if (container == frame) {
			insets = frame.getInsets();
			frameHeight -= insets.bottom + insets.top;
			frameWidth -= insets.right + insets.left;
		}
		if (DisplayMode.getWindowMode() >= 2) {
			canvasWidth = frameWidth;
			leftMargin = 0;
			topMargin = 0;
			canvasHeight = frameHeight;
		} else {
			topMargin = 0;
			leftMargin = (frameWidth - 765) / 2;
			canvasHeight = 503;
			canvasWidth = 765;
		}
		if (GlRenderer.enabled) {
			GlRenderer.setCanvasSize(canvasWidth, canvasHeight);
		}
		canvas.setSize(canvasWidth, canvasHeight);
		if (container == frame) {
			insets = frame.getInsets();
			canvas.setLocation(insets.left + leftMargin, topMargin + insets.top);
		} else {
			canvas.setLocation(leftMargin, topMargin);
		}
		if (InterfaceList.topLevelInterface != -1) {
			InterfaceList.layoutTopLevel(true);
		}
		paintFrameLetterbox();
	}

	public static void paintFrameLetterbox() {
		OracleLog.add("letterbox");
		int top = topMargin;
		int left = leftMargin;
		int bottom = frameHeight - canvasHeight - top;
		int right = frameWidth - left - canvasWidth;
		if (left <= 0 && right <= 0 && top <= 0 && bottom <= 0) {
			return;
		}
		try {
			Container container;
			if (fullScreenFrame != null) {
				container = fullScreenFrame;
			} else if (frame == null) {
				container = signLink.applet;
			} else {
				container = frame;
			}
			int insetsTop = 0;
			int insetsLeft = 0;
			if (frame == container) {
				Insets insets = frame.getInsets();
				insetsLeft = insets.left;
				insetsTop = insets.top;
			}
			if (left > 0) {
				OracleLog.fill(insetsLeft, insetsTop, left, frameHeight);
			}
			if (top > 0) {
				OracleLog.fill(insetsLeft, insetsTop, frameWidth, top);
			}
			if (right > 0) {
				OracleLog.fill(insetsLeft + frameWidth - right, insetsTop, right, frameHeight);
			}
			if (bottom > 0) {
				OracleLog.fill(insetsLeft, insetsTop + frameHeight - bottom, frameWidth, bottom);
			}
		} catch (Exception ex) {
		}
	}
}
