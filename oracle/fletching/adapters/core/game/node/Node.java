package core.game.node;
import core.game.world.map.Location;
/** Adapter: an id, a name and a location. */
public abstract class Node {
    public abstract int getId();
    public String getName() { return null; }
    public Location getLocation() { return null; }
    public core.game.node.item.Item asItem() { return (core.game.node.item.Item) this; }
}
