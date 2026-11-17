package core.cache.def.impl;

import java.util.HashMap;

/** CS2Mapping.forId: a recorded cache read answered from the case. */
public class CS2Mapping {
	private final HashMap<Integer, Object> map;

	public CS2Mapping(HashMap<Integer, Object> map) {
		this.map = map;
	}

	public static CS2Mapping forId(int id) {
		return oracle.Support.cs2(id);
	}

	public HashMap<Integer, Object> getMap() {
		return map;
	}
}