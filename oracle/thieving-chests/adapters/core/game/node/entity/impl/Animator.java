package core.game.node.entity.impl;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: records the animator's animation with its priority. */
public final class Animator {
    public enum Priority { LOW, MID, HIGH, VERY_HIGH }
    public boolean animate(Animation animation) {
        W.emit("animator", animation.getId(), animation.getPriority().name());
        return true;
    }
    public void forceAnimation(Animation animation) { W.emit("forceAnimation", animation.getId()); }
}
