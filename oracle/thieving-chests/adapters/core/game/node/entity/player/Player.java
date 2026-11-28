package core.game.node.entity.player;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final Inventory inventory = new Inventory();
    private final PacketDispatch packets = new PacketDispatch();
    private final ImpactHandler impact = new ImpactHandler();
    private final Animator animator = new Animator();
    @Override public int getId() { return 0; }
    @Override public boolean isActive() { W.emit("active", "player"); return W.flag("playerActive"); }
    @Override public void lock(int ticks) { W.emit("lock", ticks); }
    public void sendMessage(String message) { W.emit("message", message); }
    public void sendMessage(String message, int ticks) { W.emit("messageLater", message, ticks); }
    public boolean animate(Animation animation) { W.emit("animate", animation.getId(), animation.getPriority().name()); return true; }
    public boolean faceLocation(Location location) { W.emit("faceLocation"); return true; }
    public Skills getSkills() { return skills; }
    public Inventory getInventory() { return inventory; }
    public PacketDispatch getPacketDispatch() { return packets; }
    public ImpactHandler getImpactHandler() { return impact; }
    public Animator getAnimator() { return animator; }
    public static final class Inventory {
        public int freeSlots() { W.emit("freeSlots"); return W.number("freeSlots"); }
        /** Records an addition for the player; what does not fit is not covered. */
        public boolean add(Item item, Player player) { W.emit("addOrDrop", item.getId(), item.getAmount()); return true; }
    }
    public static final class PacketDispatch {
        public void sendMessage(String message) { W.emit("message", message); }
    }
}
