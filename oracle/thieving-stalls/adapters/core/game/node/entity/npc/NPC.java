package core.game.node.entity.npc;
import core.game.node.entity.Entity;
import oracle.W;
/** Adapter: a declared NPC by its place in the list. */
public final class NPC extends Entity {
    private final int index, id; private final boolean attacking;
    private final Properties properties = new Properties();
    public NPC(int index, int id, boolean attacking) { this.index = index; this.id = id; this.attacking = attacking; }
    @Override public int getId() { return id; }
    @Override public boolean isActive() { return true; }
    public boolean sendChat(String text) { W.emit("chat", index, text); return true; }
    public Properties getProperties() { return properties; }
    public final class Properties {
        public CombatPulse getCombatPulse() { return new CombatPulse(); }
    }
    public final class CombatPulse {
        public boolean isAttacking() { W.emit("attacking", index); return attacking; }
        public void attack(core.game.node.Node victim) { W.emit("attack", index); }
    }
}
