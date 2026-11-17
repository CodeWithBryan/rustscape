package core.game.node.entity.npc;

import core.game.world.map.Direction;
import core.game.world.map.Location;
import java.util.HashMap;
import java.util.Map;

/** An NPC as the trade option and its destination override read it: id, attributes, direction, location. */
@SuppressWarnings("unchecked")
public class NPC extends core.game.node.entity.Entity {
	private final int id;
	private final Map<String, Object> attributes = new HashMap<>();

	public NPC(int id, Object facingBooth, Direction direction, Location location) {
		super("npc", location);
		this.id = id;
		if (facingBooth != null) {
			attributes.put("facing_booth", facingBooth);
		}
		this.direction = direction;
	}

	@Override
	public int getId() {
		return id;
	}

	@Override
	public <T> T getAttribute(String key, T fail) {
		oracle.Support.emit("npcAttribute", id, key);
		Object value = attributes.get(key);
		return value != null ? (T) value : fail;
	}

	@Override
	public void setAttribute(String key, Object value) {
		attributes.put(key, value);
	}

	@Override
	public void removeAttribute(String key) {
		attributes.remove(key);
	}
}
