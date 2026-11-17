package core.game.node.item;

import core.game.node.entity.player.Player;
import core.game.world.map.Location;

/** Compile-only: ground items are outside this oracle. */
public class GroundItemManager {
	public static GroundItem create(Item item, Location location, Player player) {
		throw new UnsupportedOperationException();
	}

	public static GroundItem create(Item item, Location location) {
		throw new UnsupportedOperationException();
	}
}
