package core.game.node.entity.player;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import core.game.world.map.Location;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final Inventory inventory = new Inventory();
    private final PacketDispatch packets = new PacketDispatch();
    private final ImpactHandler impact = new ImpactHandler();
    @Override public int getId() { return 0; }
    @Override public Location getLocation() { return W.location("player"); }
    public void sendMessage(String message) { W.emit("message", message); }
    public Skills getSkills() { return skills; }
    public Inventory getInventory() { return inventory; }
    public PacketDispatch getPacketDispatch() { return packets; }
    public ImpactHandler getImpactHandler() { return impact; }
    public static final class Inventory {
        public boolean containsItem(Item item) { W.emit("containsItem", item.getId()); return W.flag("lockpick"); }
    }
    public static final class PacketDispatch {
        public void sendMessage(String message) { W.emit("message", message); }
    }
}
