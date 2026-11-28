package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.W;
/** Adapter: records a manual hit on the player. */
public final class ImpactHandler {
    public enum HitsplatType { MISS, NORMAL, POISON, DISEASE, NORMAL_1, VENOM }
    public Object manualHit(Entity source, int hit, HitsplatType type) {
        W.emit("manualHit", source.getId(), hit, type.name());
        return null;
    }
}
