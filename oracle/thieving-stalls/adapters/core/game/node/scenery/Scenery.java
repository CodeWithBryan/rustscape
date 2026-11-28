package core.game.node.scenery;
import core.game.node.Node;
import oracle.W;
/** Adapter: the scenery of an operation, with the declared name and activity. */
public class Scenery extends Node {
    private final int id;
    public Scenery(int id) { this.id = id; }
    @Override public int getId() { return id; }
    @Override public boolean isActive() { W.emit("active", "node"); return W.flag("nodeActive"); }
    @Override public String getName() { W.emit("name"); return (String) W.state.get("nodeName"); }
    public Scenery transform(int id) { return new Scenery(id); }
}
