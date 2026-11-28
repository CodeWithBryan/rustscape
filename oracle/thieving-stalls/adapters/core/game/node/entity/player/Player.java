package core.game.node.entity.player;
import core.game.event.Event;
import core.game.event.ResourceProducedEvent;
import core.game.node.entity.Entity;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import oracle.W;
import org.json.simple.JSONObject;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final Inventory inventory = new Inventory();
    private final PacketDispatch packets = new PacketDispatch();
    private final PulseManager pulses = new PulseManager();
    private final Locks locks = new Locks();
    private final SavedData saved = new SavedData();
    @Override public int getId() { return 0; }
    @Override public boolean isActive() { W.emit("active", "player"); return W.flag("playerActive"); }
    public boolean inCombat() { W.emit("inCombat"); return W.flag("inCombat"); }
    public void sendMessage(String message) { W.emit("message", message); }
    @SuppressWarnings("unchecked")
    public void setAttribute(String key, Object value) { W.emit("setAttribute", key, value); ((JSONObject) W.state.get("attr")).put(key, value); }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fallback) {
        W.emit("getAttribute", key);
        Object value = ((JSONObject) W.state.get("attr")).get(key);
        if (value == null) return fallback;
        return (T) (value instanceof Number ? (Object) ((Number) value).intValue() : value);
    }
    public boolean animate(Animation animation) { W.emit("animate", animation.getId(), animation.getPriority().name()); return true; }
    public void dispatch(Event event) {
        ResourceProducedEvent e = (ResourceProducedEvent) event;
        W.emit("dispatch", "ResourceProducedEvent", e.getItemId(), e.getAmount(), e.getSource().getId(), e.getOriginal());
    }
    public Skills getSkills() { return skills; }
    public Inventory getInventory() { return inventory; }
    public PacketDispatch getPacketDispatch() { return packets; }
    public PulseManager getPulseManager() { return pulses; }
    public Locks getLocks() { return locks; }
    public SavedData getSavedData() { return saved; }
    public Location getLocation() { return new Location(); }
    public static final class Inventory {
        public int freeSlots() { W.emit("freeSlots"); return W.number("freeSlots"); }
        /** Records an addition; its answer is not read by the pulse. */
        public boolean add(Item item) { W.emit("add", item.getId(), item.getAmount()); return true; }
    }
    public static final class PacketDispatch {
        public void sendMessage(String message) { W.emit("message", message); }
    }
    /** PulseManager.run: the pulse is started and, while it runs, submitted; what the manager clears is not covered. */
    public static final class PulseManager {
        public void run(Pulse pulse) {
            W.emit("run");
            W.pulse = pulse;
            pulse.start();
            if (pulse.isRunning()) W.emit("submit");
        }
    }
    public static final class Locks {
        public void lockInteractions(int ticks) { W.emit("lockInteractions", ticks); }
    }
    public static final class SavedData {
        public GlobalData getGlobalData() { return new GlobalData(); }
    }
    /** Records how far ahead of the clock the stamp is, to the second. */
    public static final class GlobalData {
        public void setSilkSteal(long time) {
            long ahead = time - System.currentTimeMillis();
            W.emit("silkSteal", Math.round(ahead / 1000.0) * 1000);
        }
    }
}
