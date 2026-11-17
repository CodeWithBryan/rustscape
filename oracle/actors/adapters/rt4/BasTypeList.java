package rt4;

import java.util.HashMap;
import java.util.Map;

/** Oracle adapter: BAS records the harness decoded with the unchanged BasType. */
public class BasTypeList {
	public static final Map<Integer, BasType> types = new HashMap<Integer, BasType>();

	public static BasType get(int id) {
		return types.get(id);
	}
}