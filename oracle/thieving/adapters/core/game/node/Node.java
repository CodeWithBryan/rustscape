package core.game.node;
/** Adapter: an id. */
public abstract class Node {
    public abstract int getId();
    public core.game.node.entity.npc.NPC asNpc() { return (core.game.node.entity.npc.NPC) this; }
}
