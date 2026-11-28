package core.cache.def.impl;
import org.json.simple.JSONObject;
/** Adapter: a declared name of the id (the corpus's table, or the case's own name or null for it), not the cache's. */
public final class ItemDefinition {
    public static final String[][] NAMES = {
        /* NAMES */
    };
    private final int id;
    private ItemDefinition(int id) { this.id = id; }
    public static ItemDefinition forId(int id) { return new ItemDefinition(id); }
    public String getName() {
        oracle.W.emit("name", id);
        String key = Integer.toString(id);
        JSONObject renamed = (JSONObject) oracle.W.state.get("renamed");
        if (renamed.containsKey(key)) return (String) renamed.get(key);
        for (String[] row : NAMES) if (row[0].equals(key)) return row[1];
        return null;
    }
}
