package core.game.node.item;

import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;

/** Compile-only: createDrop is outside this oracle. */
public class ItemPlugin {
	public boolean createDrop(Item item, Player player, NPC npc, Location location) {
		throw new UnsupportedOperationException();
	}

	public Item getItem(Item item, NPC npc) {
		throw new UnsupportedOperationException();
	}
}
