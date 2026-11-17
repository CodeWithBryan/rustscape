package rt4;

/** Oracle adapter: the local player table. */
public class PlayerList {
	public static Player self;
	public static int selfId = 17;
	public static int size;
	public static final Player[] players = new Player[2048];
	public static final int[] ids = new int[2048];
}