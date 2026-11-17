package core.game.node.entity.npc;

import core.game.node.entity.Entity;
import java.util.Map;

/** An NPC of the case: an entity that is not a player. */
public class NPC extends Entity {
	public NPC(int slot, Map spec) {
		super(slot, spec);
	}
}
