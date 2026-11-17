package content.global.skill.summoning.familiar;
import core.game.world.map.Location;
import oracle.Support;
public class Forager extends Familiar {
    public Forager(Location l) { super(l); }
    public void handlePassiveAction() { Support.emit("forager.handlePassiveAction", null, ref); }
}