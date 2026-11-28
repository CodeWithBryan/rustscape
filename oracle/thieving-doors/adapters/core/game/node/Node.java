package core.game.node;
import core.game.world.map.Location;
/** Adapter: an id and a location. */
public abstract class Node {
    public abstract int getId();
    public abstract Location getLocation();
}
