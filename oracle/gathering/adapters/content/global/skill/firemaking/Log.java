package content.global.skill.firemaking;
import oracle.Support;
public class Log {
    private final double xp;
    private Log(double xp) { this.xp = xp; }
    public static Log forId(int id) { Double xp = Support.fireXp.get(id); Support.emit("firemakingLog.forId", xp, id); return xp == null ? null : new Log(xp); }
    public double getXp() { return xp; }
}
