// Runs the unchanged rt4.LoadingBar against recording stand-ins for the
// renderer and prints one line per case: what it asked to be drawn.
//
// From the repository root (JDK on PATH):
//   javac --release 8 -nowarn -d <out> -sourcepath reference/client/deob-annotations/src/main/java
//       reference/client/client/src/main/java/rt4/LoadingBar.java
//       oracle/title/adapters/rt4/*.java oracle/title/TitleOracle.java
//   java -cp <out> TitleOracle > crates/client/tests/title/loading_bar.txt
//
// crates/client/src/title.rs reproduces every line (LoadingBar::layout).
import rt4.*;

public class TitleOracle {
	public static void main(String[] args) {
		int[][] canvases = {{765, 503}, {766, 504}, {800, 600}, {1024, 768}, {1281, 721}, {1920, 1080}, {3, 5}, {2561, 1441}};
		int[] percents = {0, 1, 33, 50, 95, 96, 97, 100};
		int[] logos = {360, 361, 1};
		Font font = new Font();
		for (int[] canvas : canvases) {
			for (int percent : percents) {
				for (int logo : logos) {
					// The hardware renderer always draws the background.
					for (int mode = 0; mode < 3; mode++) {
						GameShell.canvasWidth = canvas[0];
						GameShell.canvasHeight = canvas[1];
						client.mainLoadPercentage = percent;
						TitleScreen.logo.width = logo;
						GlRenderer.enabled = mode == 0;
						Log.out.setLength(0);
						LoadingBar.render(mode == 1, font);
						System.out.print("canvas=" + canvas[0] + "," + canvas[1] + " percent=" + percent + " logo=" + logo + " mode=" + mode + " :" + Log.out + "\n");
					}
				}
			}
		}
	}
}