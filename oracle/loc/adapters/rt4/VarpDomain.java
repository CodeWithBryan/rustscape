package rt4;

/** Oracle adapter: the two variable reads of LocType.getMultiLoc, with values the oracle sets. */
public final class VarpDomain {
	public static int[] activeVarps = new int[4000];
	public static final java.util.Map<Integer, Integer> varbits = new java.util.HashMap<Integer, Integer>();
	public static int getVarbit(int id) {
		Integer value = varbits.get(id);
		return value == null ? 0 : value;
	}
}