package core.game.node.item;
import core.game.node.Node;
/** Adapter: id, amount and slot. */
public class Item extends Node {
    private final int id, amount;
    private int slot = -1;
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { this.id = id; this.amount = amount; }
    @Override public int getId() { return id; }
    public int getAmount() { return amount; }
    public int getSlot() { return slot; }
    public void setSlot(int slot) { this.slot = slot; }
}
