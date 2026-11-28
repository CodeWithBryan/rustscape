package core.game.node.entity;
import core.game.node.Node;
import oracle.W;
/** Adapter: the declared attributes, read and written with a record of each call. */
public abstract class Entity extends Node {
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fallback) {
        W.emit("attribute", key);
        Object v = ((org.json.simple.JSONObject) W.state.get("attributes")).get(key);
        // A declared whole number is the Integer the original would hold.
        if (v instanceof Long) v = ((Long) v).intValue();
        return v == null ? fallback : (T) v;
    }
    @SuppressWarnings("unchecked")
    public void setAttribute(String key, Object value) {
        W.emit("setAttribute", key, value);
        ((org.json.simple.JSONObject) W.state.get("attributes")).put(key, value instanceof Integer ? (Object) ((Integer) value).longValue() : value);
    }
}
