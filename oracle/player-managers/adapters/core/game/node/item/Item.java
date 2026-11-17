package core.game.node.item;

import core.cache.def.impl.ItemDefinition;
import core.game.node.Node;
import oracle.Support;

/** An item id and amount; its name is a case input, getAlchemyValue() the original text. */
public class Item extends Node {
	private final int id;
	private final int amount;
	private final ItemDefinition definition;

	public Item(int id) {
		this(id, 1);
	}

	public Item(int id, int amount) {
		this.id = id;
		this.amount = amount;
		this.definition = ItemDefinition.forId(id);
	}

	public int getId() {
		return id;
	}

	public int getAmount() {
		return amount;
	}

	public String getName() {
		Support.emit("itemName", id);
		return Support.itemName(id);
	}

	/* ORIGINAL_ITEM_ALCHEMY_VALUE */
}
