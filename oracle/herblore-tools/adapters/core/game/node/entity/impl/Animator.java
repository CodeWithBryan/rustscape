package core.game.node.entity.impl;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: the priority constants SkillPulse names and a recorded animate. */
public final class Animator {
    public enum Priority { LOW, MID, HIGH, VERY_HIGH }
    public boolean animate(Animation animation) { W.emit("animator", animation.getId()); return true; }
}
