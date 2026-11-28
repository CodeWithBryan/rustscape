package core.game.node.entity.player;
import content.data.Quests;
import core.game.node.entity.Entity;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final Inventory inventory = new Inventory();
    private final PacketDispatch packets = new PacketDispatch();
    private final QuestRepository quests = new QuestRepository();
    private final PulseManager pulses = new PulseManager();
    @Override public int getId() { return 0; }
    public void sendMessage(String message) { W.emit("message", message); }
    public boolean animate(Animation animation) {
        if (animation.getPriority() == null) W.emit("animate", animation.getId());
        else W.emit("animate", animation.getId(), animation.getPriority().name());
        return true;
    }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fallback) {
        W.emit("attribute", key);
        Object v = ((org.json.simple.JSONObject) W.state.get("attributes")).get(key);
        return v == null ? fallback : (T) v;
    }
    public Skills getSkills() { return skills; }
    public Inventory getInventory() { return inventory; }
    public PacketDispatch getPacketDispatch() { return packets; }
    public QuestRepository getQuestRepository() { return quests; }
    public PulseManager getPulseManager() { return pulses; }
    /** Adapter: the declared inventory, a map of amounts. */
    public static final class Inventory {
        public boolean containsItem(Item item) { W.emit("contains", item.getId()); return W.amount(item.getId()) >= item.getAmount(); }
        /** Each removal succeeds when at least the amount is held. */
        public boolean remove(Item... items) {
            boolean all = true;
            org.json.simple.JSONArray ids = new org.json.simple.JSONArray();
            for (Item item : items) {
                ids.add(item.getId());
                if (!W.take(item.getId(), item.getAmount())) all = false;
            }
            W.emit("remove", ids, all);
            return all;
        }
        /** An addition always succeeds. */
        public boolean add(Item... items) {
            for (Item item : items) { W.give(item.getId(), item.getAmount()); W.emit("add", item.getId(), item.getAmount()); }
            return true;
        }
    }
    public static final class PacketDispatch {
        public void sendMessage(String message) { W.emit("message", message); }
    }
    /** Adapter: the declared stage of a quest; isComplete is the original's comparison for a registered quest. */
    public static final class QuestRepository {
        public int getStage(Quests quest) { return W.questStage(quest.name()); }
        public boolean isComplete(Quests quest) { return W.questStage(quest.name()) >= 100; }
    }
    /** Adapter: run records the pulse, starts it and keeps it for the driver. */
    public static final class PulseManager {
        public void run(Pulse pulse) { W.run(pulse); }
    }
}
