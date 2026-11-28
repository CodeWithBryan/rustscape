package core.game.node.item;
import core.game.node.Node;
import oracle.W;
/** Adapter: id, amount and the declared name (null without one). */
public class Item extends Node {
    private final int id, amount;
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { this.id = id; this.amount = amount; }
    @Override public int getId() { return id; }
    public int getAmount() { return amount; }
    @Override public String getName() { return W.name(id); }
}
