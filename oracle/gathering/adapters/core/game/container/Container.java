package core.game.container;
import core.game.node.item.Item;
import oracle.Support;
public class Container {
    private final boolean equipment;
    public Container(boolean equipment) { this.equipment = equipment; }
    public int freeSlots() { return Support.emit("inventory.freeSlots", Support.freeSlots()); }
    public boolean contains(int id, int amount) { return Support.emit("inventory.contains", Support.amount(id) >= amount, id, amount); }
    public boolean add(Item item) { return Support.emit("inventory.add", Support.add(item.getId(), item.getAmount()), item.getId(), item.getAmount()); }
    public boolean hasSpaceFor(Item item) { return Support.emit("inventory.hasSpaceFor", Support.space(item.getId(), item.getAmount()), item.getId(), item.getAmount()); }
    public Item getNew(int slot) { Integer id = Support.equipment.get(slot); Support.emit("equipment.getNew", id == null ? 0 : id, slot); return new Item(id == null ? 0 : id); }
    public Item get(int slot) { Integer id = Support.equipment.get(slot); Support.emit("equipment.get", id, slot); return id == null ? null : new Item(id); }
}
