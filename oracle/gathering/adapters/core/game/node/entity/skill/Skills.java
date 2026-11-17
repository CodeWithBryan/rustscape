package core.game.node.entity.skill;
import oracle.Support;
public class Skills {
    /* SKILL_CONSTANTS */
    public int getLevel(int s) { return Support.emit("skills.getLevel", Support.dynamic[s], s); }
    public int getStaticLevel(int s) { return Support.emit("skills.getStaticLevel", Support.fixed[s], s); }
    public void addExperience(int s, double xp, boolean playerMod) { Support.emit("skills.addExperience3", null, s, xp, playerMod); }
    public void addExperience(int s, double xp) { Support.emit("skills.addExperience2", null, s, xp); }
}
