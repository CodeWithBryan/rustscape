package rt4;

import java.util.ArrayList;
import java.util.List;

/** Oracle adapter: flat terrain, and a record of every entity handed to the scene. */
public class SceneGraph {
	public static final List<String> added = new ArrayList<String>();

	public static int getTileHeight(int plane, int x, int y) {
		return 0;
	}

	public static boolean add(int plane, int x, int y, int height, int radius, Entity entity, int angle, long key, boolean stretches) {
		added.add("[0," + (key >>> 32) + "," + (key & 0xFFFFFFFFL) + "," + x + "," + y + "," + radius + "," + angle + "," + (stretches ? 1 : 0) + "]");
		return true;
	}
}