package core.cache.def.impl;

/** Name service for WeightBasedTable.toString; the rest only lets NPCDropTables compile. */
public class ItemDefinition {
	private final int id;

	private ItemDefinition(int id) {
		this.id = id;
	}

	public static ItemDefinition forId(int id) {
		return new ItemDefinition(id);
	}

	public String getName() {
		return oracle.Support.itemName(id);
	}

	public boolean isStackable() {
		throw new UnsupportedOperationException();
	}

	public boolean isUnnoted() {
		throw new UnsupportedOperationException();
	}

	public boolean isTradeable() {
		throw new UnsupportedOperationException();
	}
}
