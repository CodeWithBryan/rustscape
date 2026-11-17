package core.game.node;

import core.game.world.map.Location;

/** A plain node: a location from the case (HintIconContext reads it for a non-entity target). */
public class Node {
	protected int index;
	protected Location location;

	public Node() {
	}

	public Node(Location location) {
		this.location = location;
	}

	public Location getLocation() {
		oracle.Support.emit("location", oracle.Support.label(this));
		return location;
	}

	public int getIndex() {
		oracle.Support.emit("index", oracle.Support.label(this));
		return index;
	}

	public String getName() {
		return "node";
	}
}
