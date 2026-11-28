package core.game.node.entity.npc;
import core.game.node.entity.Entity;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: the NPC of an operation, with the declared attack animation. */
public final class NPC extends Entity {
    private final int id;
    private final Properties properties = new Properties();
    public NPC(int id) { this.id = id; }
    @Override public int getId() { return id; }
    public boolean face(Entity entity) { W.emit("face", entity == null ? null : (entity instanceof NPC ? "npc" : "player")); return true; }
    public Properties getProperties() { return properties; }
    public static final class Properties {
        public Animation getAttackAnimation() { return new Animation(W.number("attackAnimation")); }
    }
}
