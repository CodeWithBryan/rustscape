package content.global.skill.summoning.familiar;
import core.game.node.entity.npc.NPC;
import core.game.world.map.Location;
import oracle.Support;
/** A declared familiar: only its type and location are read by the fishing sources. */
public class Familiar extends NPC {
    public Familiar(Location l) { super(0, l); this.ref = 2000; }
    public boolean isBurdenBeast() { return false; }
    @Override public Location getLocation() { return Support.emit("node.location", rawLocation, ref); }
}