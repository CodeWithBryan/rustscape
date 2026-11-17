package core.game.node.entity.npc;

import core.game.node.entity.Entity;
import core.game.world.map.Location;

/** The NPC whose table is rolled: an id and the recording behaviour. */
public class NPC extends Entity {
	public final int id;
	public NPCBehavior behavior = new NPCBehavior();

	public NPC(int id) {
		this.id = id;
	}

	public int getId() {
		return id;
	}

	public Location getDropLocation() {
		throw new UnsupportedOperationException();
	}

	public Location getLocation() {
		throw new UnsupportedOperationException();
	}

	public Properties getProperties() {
		throw new UnsupportedOperationException();
	}

	public static class Properties {
		public boolean isMultiZone() {
			throw new UnsupportedOperationException();
		}
	}
}
