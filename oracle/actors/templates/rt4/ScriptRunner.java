package rt4;

import org.openrs2.deob.annotation.OriginalArg;
import org.openrs2.deob.annotation.OriginalMember;
import org.openrs2.deob.annotation.Pc;

/**
 * Oracle adapter: only the scene-insertion methods of the reference
 * ScriptRunner.java, copied verbatim when the oracle is generated. The
 * attached-entity insertion is recorded instead of performed.
 */
public class ScriptRunner {
	public static final int[][] tileEntityCounts = new int[104][104];

	public static boolean addAttachedEntityToScene(int plane, int xFine, int yFine, int tileHeight, Entity entity, int angle, long key, int x0, int y0, int x1, int y1) {
		SceneGraph.added.add("[1," + (key >>> 32) + "," + (key & 0xFFFFFFFFL) + "," + xFine + "," + yFine + ",0," + angle + ",0]");
		return true;
	}

	//@EXTRACT ScriptRunner.java public static void clearTileEntityCounts(

	//@EXTRACT ScriptRunner.java public static void addPlayersToScene(

	//@EXTRACT ScriptRunner.java public static void addNpcsToScene(
}
