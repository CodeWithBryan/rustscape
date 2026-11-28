package core.game.node.entity;
import core.game.event.Event;
import core.game.event.ResourceProducedEvent;
import core.game.node.Node;
import core.game.node.scenery.Scenery;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import java.util.HashMap;
import java.util.Map;
import oracle.W;
/** Adapter: a recorded dispatch, the pulse manager, the location and an attribute store the case resets. */
public abstract class Entity extends Node {
    private final PulseManager pulses = new PulseManager();
    public final Map<String, Object> attributes = new HashMap<>();
    public PulseManager getPulseManager() { return pulses; }
    public Location getLocation() { return Location.ofPlayer(); }
    public void dispatch(Event event) {
        ResourceProducedEvent e = (ResourceProducedEvent) event;
        Scenery source = (Scenery) e.getSource();
        W.emit("produced", e.getItemId(), e.getAmount(), source.shown(), e.getOriginal());
    }
    /** Keys are stored as given, with any "/save:" prefix removed, as GameAttributes does. */
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        Object value = attributes.get(key);
        W.emit("getAttribute", key, value == null);
        return value == null ? fail : (T) value;
    }
    public void setAttribute(String key, Object value) {
        W.emit("setAttribute", key, value instanceof Scenery ? ((Scenery) value).shown() : value);
        attributes.put(key.replace("/save:", ""), value);
    }
    /** Adapter: run records the pulse and keeps it for the driver. */
    public static final class PulseManager {
        public void run(Pulse pulse) { W.run(pulse); }
    }
}
