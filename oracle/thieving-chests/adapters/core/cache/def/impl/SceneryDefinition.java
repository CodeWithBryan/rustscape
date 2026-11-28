package core.cache.def.impl;
import java.util.HashMap;
import java.util.Map;
import oracle.W;
/** Adapter: keeps what a plugin puts on a definition's handlers, in order, for the driver. */
public final class SceneryDefinition {
    private final int id;
    private SceneryDefinition(int id) { this.id = id; }
    public static SceneryDefinition forId(int id) { return new SceneryDefinition(id); }
    public Map<String, Object> getHandlers() {
        return new HashMap<String, Object>() {
            @Override public Object put(String key, Object value) {
                W.register(id, key, (core.game.interaction.OptionHandler) value);
                return null;
            }
        };
    }
}
