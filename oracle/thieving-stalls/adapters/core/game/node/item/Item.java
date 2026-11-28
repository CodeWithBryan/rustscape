package core.game.node.item;
import core.game.node.Node;
import oracle.W;
/** Adapter: id and amount; the name is the case's declared one. */
public class Item extends Node {
    private final int id, amount;
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { this.id = id; this.amount = amount; }
    @Override public int getId() { return id; }
    @Override public boolean isActive() { return true; }
    public int getAmount() { return amount; }
    @Override public String getName() { return W.itemName(id); }
}
