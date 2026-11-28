package core.game.node.entity.player;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final Animator animator = new Animator();
    private final ImpactHandler impactHandler = new ImpactHandler();
    private final Equipment equipment = new Equipment();
    private final Inventory inventory = new Inventory();
    @Override public int getId() { return 0; }
    public boolean inCombat() { W.emit("inCombat"); return W.flag("inCombat"); }
    public void sendMessage(String message) { W.emit("message", message); }
    public Skills getSkills() { return skills; }
    public Animator getAnimator() { return animator; }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public Equipment getEquipment() { return equipment; }
    public Inventory getInventory() { return inventory; }
    public void lock(int ticks) { W.emit("lock", ticks); }
    /** Adapter: the declared answer for the gloves. */
    public static final class Equipment {
        public boolean contains(int id, int amount) { W.emit("equipped", id, amount); return W.flag("gloves"); }
    }
    /** Adapter: records an addition; its answer is not read by the listener. */
    public static final class Inventory {
        public boolean add(Item item) { W.emit("add", item.getId(), item.getAmount()); return true; }
    }
}
