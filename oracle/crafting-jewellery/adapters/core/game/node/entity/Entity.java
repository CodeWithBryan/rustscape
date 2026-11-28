package core.game.node.entity;
import core.game.node.Node;
import core.game.system.task.Pulse;
import oracle.W;
/** Adapter: the pulse manager. */
public abstract class Entity extends Node {
    private final PulseManager pulses = new PulseManager();
    public PulseManager getPulseManager() { return pulses; }
    /** Adapter: run records the pulse and keeps it for the driver. */
    public static final class PulseManager {
        public void run(Pulse pulse) { W.run(pulse); }
    }
}
