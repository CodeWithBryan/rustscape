package core.game.node.item;

import core.cache.def.impl.ItemDefinition;

/**
 * Recording item: it keeps the constructor arguments as given. The original
 * Item, its id/charge packing and its definition lookup are outside this oracle.
 */
public class Item {
	private final int id;
	private final int amount;
	private final int charge;

	public Item(int id) {
		this(id, 1, 1000);
	}

	public Item(int id, int amount) {
		this(id, amount, 1000);
	}

	public Item(int id, int amount, int charge) {
		this.id = id;
		this.amount = amount;
		this.charge = charge;
	}

	public int getId() {
		return id;
	}

	public int getAmount() {
		return amount;
	}

	public int getCharge() {
		return charge;
	}

	public String getName() {
		throw new UnsupportedOperationException();
	}

	public boolean hasItemPlugin() {
		throw new UnsupportedOperationException();
	}

	public ItemPlugin getPlugin() {
		throw new UnsupportedOperationException();
	}

	public ItemDefinition getDefinition() {
		throw new UnsupportedOperationException();
	}

	public int getNoteChange() {
		throw new UnsupportedOperationException();
	}
}
