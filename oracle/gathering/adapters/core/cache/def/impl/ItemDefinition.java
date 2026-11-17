package core.cache.def.impl;
import oracle.Support;
public class ItemDefinition {
    private final int id;
    private ItemDefinition(int id) { this.id = id; }
    public static ItemDefinition forId(int id) { return new ItemDefinition(id); }
    public String getName() { return Support.emit("itemDefinition.name", Support.name(id), id); }
}
