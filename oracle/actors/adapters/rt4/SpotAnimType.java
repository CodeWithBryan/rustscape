package rt4;

import java.util.ArrayList;
import java.util.List;

/** Oracle adapter: records the frame arguments ProjAnim asks a model for. */
public class SpotAnimType {
	public static final List<int[]> requests = new ArrayList<int[]>();
	public int seqId = -1;
	public boolean alignToTerrain;

	public final Model constructModel(int nextFrame, int frame, int frameClock) {
		requests.add(new int[] {nextFrame, frame, frameClock});
		return new Model();
	}
}