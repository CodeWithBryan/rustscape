package core.game.node.entity;
import core.game.event.Event;
import core.game.event.ResourceProducedEvent;
import core.game.node.Node;
import core.game.system.task.Pulse;
import oracle.W;
/** Adapter: a recorded dispatch and the pulse manager. */
public abstract class Entity extends Node {
    private final PulseManager pulses = new PulseManager();
    public PulseManager getPulseManager() { return pulses; }
    public void dispatch(Event event) {
        ResourceProducedEvent e = (ResourceProducedEvent) event;
        W.emit("produced", e.getItemId(), e.getAmount(), e.getSource() == W.player, e.getOriginal());
    }
    /** Adapter: run records the pulse and keeps it for the driver. */
    public static final class PulseManager {
        public void run(Pulse pulse) { W.run(pulse); }
    }
}
