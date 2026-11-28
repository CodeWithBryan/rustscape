package core.game.node;
/** Adapter: an id, a name and whether the node is active. */
public abstract class Node {
    public abstract int getId();
    public abstract boolean isActive();
    public String getName() { return null; }
}
