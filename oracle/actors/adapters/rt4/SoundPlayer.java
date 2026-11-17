package rt4;

import java.util.ArrayList;
import java.util.List;

/** Oracle adapter: records sequence sound requests instead of playing them. */
public class SoundPlayer {
	public static final List<String> events = new ArrayList<String>();

	public static void playSeqSound(int y, SeqType seq, int x, boolean self, int frame) {
		events.add("[" + seq.id + "," + frame + "]");
	}
}