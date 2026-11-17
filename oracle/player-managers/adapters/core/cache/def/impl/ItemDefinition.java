package core.cache.def.impl;

import oracle.Support;

/** getAlchemyValue(high) answers from the case's alchemy table (0 when absent). */
public class ItemDefinition {
	private final int id;

	private ItemDefinition(int id) {
		this.id = id;
	}

	public static ItemDefinition forId(int id) {
		return new ItemDefinition(id);
	}

	public int getAlchemyValue(boolean high) {
		Support.emit("alchemy", id, high);
		return Support.alchemy(id);
	}
}
