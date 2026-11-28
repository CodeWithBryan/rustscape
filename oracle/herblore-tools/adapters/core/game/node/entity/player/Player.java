package core.game.node.entity.player;
import content.data.Quests;
import core.game.node.entity.Entity;
import core.game.node.entity.impl.Animator;
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
    private final Animator animator = new Animator();
    @Override public int getId() { return 0; }
    public boolean animate(Animation animation) {
        if (animation.getPriority() == null) W.emit("animate", animation.getId());
        else W.emit("animate", animation.getId(), animation.getPriority().name());
        return true;
    }
    public Skills getSkills() { return skills; }
    public Inventory getInventory() { return inventory; }
    public PacketDispatch getPacketDispatch() { return packets; }
    public QuestRepository getQuestRepository() { return quests; }
    public PulseManager getPulseManager() { return pulses; }
    public Animator getAnimator() { return animator; }
    /** Adapter: the declared inventory, a map of amounts. */
    public static final class Inventory {
        public boolean containsItem(Item item) {
            W.emit("contains", item.getId(), item.getAmount());
            return W.amount(item.getId()) >= item.getAmount();
        }
        /** A removal succeeds when at least the amount is held. */
        public boolean remove(Item item) {
            boolean removed = W.take(item.getId(), item.getAmount());
            W.emit("remove", item.getId(), item.getAmount(), removed);
            return removed;
        }
        /** An addition always succeeds. */
        public boolean add(Item item) {
            W.give(item.getId(), item.getAmount());
            W.emit("add", item.getId(), item.getAmount());
            return true;
        }
        public int getAmount(Item item) { W.emit("getAmount", item.getId()); return W.amount(item.getId()); }
        public int getAmount(int id) { W.emit("amount", id); return W.amount(id); }
    }
    public static final class PacketDispatch {
        public void sendMessage(String message) { W.emit("message", message); }
    }
    /** Adapter: isComplete is the original's comparison for a registered quest, over the declared stage. */
    public static final class QuestRepository {
        public boolean isComplete(Quests quest) { return W.questStage(quest.name()) >= 100; }
    }
    /** Adapter: run records the pulse, starts it and keeps it for the driver. */
    public static final class PulseManager {
        public void run(Pulse pulse) { W.run(pulse); }
    }
}
