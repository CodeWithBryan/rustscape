package core.game.node.entity.skill;
import oracle.W;
/** Adapter: the pinned skill constants, the declared level and a recorded experience reward. */
public final class Skills {
    /* SKILL_CONSTANTS */
    public int getLevel(int skill) { W.emit("level", skill); return W.number("level"); }
    public void addExperience(int skill, double experience) { W.emit("xp", skill, W.bits(experience)); }
}
