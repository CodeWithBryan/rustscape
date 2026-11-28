package core.game.node.scenery;
import core.game.node.Node;
import core.game.world.map.Location;
import oracle.W;
/** Adapter: the scenery of an operation, with the declared activity. */
public class Scenery extends Node {
    private final int id;
    public Scenery(int id) { this.id = id; }
    @Override public int getId() { return id; }
    @Override public boolean isActive() { W.emit("active", "node"); return W.flag("nodeActive"); }
    public Location getLocation() { return new Location(); }
    public Scenery transform(int id) { return new Scenery(id); }
}
