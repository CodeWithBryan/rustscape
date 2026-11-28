package core.cache.def.impl;
import java.util.AbstractMap;
import java.util.Map;
import java.util.Set;
/** Adapter: `forId(id).getHandlers().put(key, handler)` is recorded. */
public final class SceneryDefinition {
    private final int id;
    private SceneryDefinition(int id) { this.id = id; }
    public static SceneryDefinition forId(int id) { return new SceneryDefinition(id); }
    public Map<String, Object> getHandlers() {
        return new AbstractMap<String, Object>() {
            @Override public Object put(String key, Object value) { oracle.W.option(id, key, value); return null; }
            @Override public Set<Map.Entry<String, Object>> entrySet() { return java.util.Collections.emptySet(); }
        };
    }
}
