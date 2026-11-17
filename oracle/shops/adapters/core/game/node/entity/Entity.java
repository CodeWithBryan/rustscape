package core.game.node.entity;

/** The Entity attribute members the shop classes reach (GameAttributes semantics in each subclass). */
public abstract class Entity extends core.game.node.Node {
	public Entity(String name, core.game.world.map.Location location) {
		super(name, location);
	}

	public abstract <T> T getAttribute(String key, T fail);

	public abstract void setAttribute(String key, Object value);

	public abstract void removeAttribute(String key);
}
