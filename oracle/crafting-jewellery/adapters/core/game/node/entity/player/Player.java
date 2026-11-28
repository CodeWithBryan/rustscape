package core.game.node.entity.player;
import core.game.component.Component;
import core.game.node.entity.Entity;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final Inventory inventory = new Inventory();
    private final PacketDispatch packets = new PacketDispatch();
    private final InterfaceManager interfaces = new InterfaceManager();
    @Override public int getId() { return 0; }
    public boolean animate(Animation animation) {
        W.emit("playerAnimate", animation.getId(), animation.getDelay(), animation.getPriority() == null ? null : animation.getPriority().name());
        return true;
    }
    public void sendMessages(String... messages) {
        org.json.simple.JSONArray all = new org.json.simple.JSONArray();
        for (String m : messages) all.add(m);
        W.emit("messages", all);
    }
    public Skills getSkills() { return skills; }
    public Inventory getInventory() { return inventory; }
    public PacketDispatch getPacketDispatch() { return packets; }
    public InterfaceManager getInterfaceManager() { return interfaces; }
    /** Adapter: the declared inventory, a map of amounts. */
    public static final class Inventory {
        /** At least the amount is held. */
        public boolean contains(int id, int amount) { W.emit("contains", id, amount); return W.amount(id) >= amount; }
        /** Each item is removed when at least its amount is held; the answer is whether every one was. */
        public boolean remove(Item... items) {
            boolean all = true;
            org.json.simple.JSONArray ids = new org.json.simple.JSONArray();
            for (Item item : items) {
                ids.add(W.list(item.getId(), item.getAmount()));
                if (!W.take(item.getId(), item.getAmount())) all = false;
            }
            W.emit("remove", ids, all);
            return all;
        }
        /** An addition always succeeds. */
        public boolean add(Item item) { W.give(item.getId(), item.getAmount()); W.emit("add", item.getId(), item.getAmount()); return true; }
        public int getAmount(Item item) { W.emit("getAmount", item.getId()); return W.amount(item.getId()); }
    }
    public static final class PacketDispatch {
        public void sendMessage(String message) { W.emit("message", message); }
        public void sendInterfaceConfig(int iface, int child, boolean hide) { W.emit("interfaceConfig", iface, child, hide); }
        public void sendItemZoomOnInterface(int item, int zoom, int iface, int child) { W.emit("itemZoom", item, zoom, iface, child); }
    }
    public static final class InterfaceManager {
        public Component open(Component component) { W.emit("open", component.getId()); return component; }
        public boolean close() { W.emit("close"); return true; }
    }
}
