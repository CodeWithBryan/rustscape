package core.game.node.scenery;
import core.cache.def.impl.SceneryDefinition;
import core.game.node.Node;
import core.game.world.map.Direction;
import core.game.world.map.Location;
/** Adapter: the scenery of an operation: its id, location and the direction of its rotation. */
public class Scenery extends Node {
    private final int id;
    private final Location location;
    private final Direction direction;
    public Scenery(int id, Location location, int rotation) {
        this.id = id;
        this.location = location;
        // Scenery's constructor: Direction.get(rotation).
        this.direction = Direction.get(rotation);
    }
    @Override public int getId() { return id; }
    @Override public Location getLocation() { return location; }
    public Direction getDirection() { return direction; }
    public SceneryDefinition getDefinition() { return SceneryDefinition.forId(id); }
}
