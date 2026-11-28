package core.game.world;
import core.game.system.task.Pulse;
import oracle.W;
/** Adapter: the declared tick, and a pulser that keeps the submitted pulse for the driver. */
public final class GameWorld {
    public static final class Pulser {
        public void submit(Pulse pulse) { W.emit("submit"); W.pulse = pulse; }
    }
    private static final Pulser PULSER = new Pulser();
    public static int getTicks() { W.emit("ticks"); return W.number("ticks"); }
    public static Pulser getPulser() { return PULSER; }
}
