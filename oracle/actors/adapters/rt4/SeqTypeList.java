package rt4;

import java.util.HashMap;
import java.util.Map;

/** Oracle adapter: sequences the harness decoded with the unchanged SeqType. */
public class SeqTypeList {
	public static final Map<Integer, SeqType> types = new HashMap<Integer, SeqType>();

	public static SeqType get(int id) {
		return types.get(id);
	}

	public static AnimFrameset getAnimFrameset(int id) {
		return null;
	}
}