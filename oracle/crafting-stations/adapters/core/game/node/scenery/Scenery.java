package core.game.node.scenery;
import core.game.node.Node;
/** Adapter: an id with coordinates, or an id with a type and a rotation (the listener's fallback). */
public final class Scenery extends Node {
    public final int id, x, y, z;
    public final boolean typed;
    public Scenery(int id, int x, int y, int z) { this.id = id; this.x = x; this.y = y; this.z = z; this.typed = false; }
    public Scenery(int id, int type, int rotation) { this.id = id; this.x = type; this.y = rotation; this.z = 0; this.typed = true; }
    @Override public int getId() { return id; }
    /** What the trace shows of it. */
    public Object shown() {
        return typed ? oracle.W.list("typed", id, x, y) : oracle.W.list(id, x, y, z);
    }
}
