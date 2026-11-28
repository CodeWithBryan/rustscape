package core.cache.def.impl;
/** Adapter: a declared name, "Name Of " and the id. */
public final class ItemDefinition {
    private final int id;
    private ItemDefinition(int id) { this.id = id; }
    public static ItemDefinition forId(int id) { return new ItemDefinition(id); }
    public String getName() { oracle.W.emit("name", id); return "Name Of " + id; }
}
