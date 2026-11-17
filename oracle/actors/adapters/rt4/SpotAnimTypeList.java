package rt4;

import java.util.HashMap;
import java.util.Map;

/** Oracle adapter: spot animations name only their sequence here. */
public class SpotAnimTypeList {
	public static final Map<Integer, SpotAnimType> types = new HashMap<Integer, SpotAnimType>();

	public static SpotAnimType get(int id) {
		SpotAnimType type = types.get(id);
		return type == null ? new SpotAnimType() : type;
	}
}