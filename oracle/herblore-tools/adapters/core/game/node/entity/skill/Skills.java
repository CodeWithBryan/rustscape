package core.game.node.entity.skill;
import oracle.W;
/** Adapter: the pinned skill constants, the declared levels and a recorded experience reward. */
public final class Skills {
    /* SKILL_CONSTANTS */
    public int getLevel(int skill) { W.emit("level", skill); return W.number("level"); }
    public int getStaticLevel(int skill) { W.emit("staticLevel", skill); return W.number("staticLevel"); }
    public void addExperience(int skill, double experience, boolean playerMod) { W.emit("xp", skill, W.bits(experience), playerMod); }
    public void addExperience(int skill, double experience) { W.emit("xp", skill, W.bits(experience)); }
}
