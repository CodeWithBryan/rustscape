package core.game.node;
/** Adapter: an id, a name and the active flag Pulse asks its checks for. */
public abstract class Node {
    public abstract int getId();
    public String getName() { return null; }
    public boolean isActive() { return true; }
    public core.game.node.item.Item asItem() { return (core.game.node.item.Item) this; }
}
