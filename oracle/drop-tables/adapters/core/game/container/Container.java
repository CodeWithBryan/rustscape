package core.game.container;

import core.game.node.entity.player.Player;
import core.game.node.item.Item;

/** Recording inventory answers for WeightBasedTable.canRoll. */
public class Container {
	private final Player player;

	public Container(Player player) {
		this.player = player;
	}

	public boolean hasSpaceFor(Item... items) {
		return oracle.Support.hasSpaceFor(player, items);
	}

	public boolean isFull() {
		return oracle.Support.isFull(player);
	}
}
