package core.game.node.entity.player;
import core.game.component.Component;
import core.game.container.impl.EquipmentContainer;
import core.game.node.entity.Entity;
import core.game.node.entity.player.link.diary.DiaryType;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final Inventory inventory = new Inventory();
    private final EquipmentContainer equipment = new EquipmentContainer();
    private final PacketDispatch packets = new PacketDispatch();
    private final InterfaceManager interfaces = new InterfaceManager();
    private final AchievementDiaryManager diaries = new AchievementDiaryManager();
    @Override public int getId() { return 0; }
    public boolean animate(Animation animation) {
        W.emit("playerAnimate", animation.getId(), animation.getDelay(), animation.getPriority() == null ? null : animation.getPriority().name());
        return true;
    }
    public Skills getSkills() { return skills; }
    public Inventory getInventory() { return inventory; }
    public EquipmentContainer getEquipment() { return equipment; }
    public PacketDispatch getPacketDispatch() { return packets; }
    public InterfaceManager getInterfaceManager() { return interfaces; }
    public AchievementDiaryManager getAchievementDiaryManager() { return diaries; }
    /** Adapter: the declared inventory, a map of amounts. */
    public static final class Inventory {
        public boolean contains(int id, int amount) { W.emit("contains", id, amount); return W.amount(id) >= amount; }
        /** A removal succeeds when at least the amount is held. */
        public boolean remove(Item item) {
            boolean removed = W.take(item.getId(), item.getAmount());
            W.emit("remove", item.getId(), item.getAmount(), removed);
            return removed;
        }
        /** An addition always succeeds. */
        public boolean add(Item item) { W.give(item.getId(), item.getAmount()); W.emit("add", item.getId(), item.getAmount()); return true; }
        public int getAmount(Item item) { W.emit("getAmount", item.getId()); return W.amount(item.getId()); }
    }
    public static final class PacketDispatch {
        public void sendMessage(String message) { W.emit("message", message); }
    }
    public static final class InterfaceManager {
        public Component open(Component component) { W.emit("open", component.getId()); return component; }
        public boolean close() { W.emit("close"); return true; }
    }
    /** Adapter: declared completion and a recorded finishTask. */
    public static final class AchievementDiaryManager {
        public Diary getDiary(DiaryType type) { return new Diary(type); }
        public void finishTask(Player player, DiaryType type, int level, int index) {
            W.emit("diary", player == W.player, type.name(), level, index);
        }
    }
    public static final class Diary {
        private final DiaryType type;
        Diary(DiaryType type) { this.type = type; }
        public boolean isComplete(int level) { W.emit("isComplete", type.name(), level); return W.flag("seersHard"); }
        public boolean isComplete(int level, int index) { W.emit("isComplete", type.name(), level, index); return W.flag("taskDone"); }
    }
}
