package core.game.node.item;
import oracle.Support;
public class Item {
    private int id, amount, charge;
    public Item(int id) { this(id, 1, 1000); }
    public Item(int id, int amount) { this(id, amount, 1000); }
    public Item(int id, int amount, int charge) { this.id = id; this.amount = amount; this.charge = charge; }
    public int getId() { return id; } public int getAmount() { return amount; } public int getCharge() { return charge; }
    public void setAmount(int amount) { this.amount = amount; }
    public String getName() { return Support.emit("item.name", Support.name(id), id); }
}
