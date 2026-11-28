package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.W;
/** Adapter: records a manual hit and the disabled ticks. */
public final class ImpactHandler {
    public enum HitsplatType { MISS, NORMAL, POISON, DISEASE, NORMAL_1, VENOM }
    public Object manualHit(Entity source, int hit, HitsplatType type) {
        W.emit("manualHit", hit, type.name());
        return null;
    }
    public void setDisabledTicks(int ticks) { W.emit("disabledTicks", ticks); }
}
