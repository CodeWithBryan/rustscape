package core.game.node.entity.npc;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
import oracle.Support;
public class NPC extends Entity {
    public boolean rawInvisible;
    public NPC(int id, Location l) { this.rawId = id; this.rawLocation = l; this.ref = -3; }
    /** The reply carries the identity of the Location object: FishingPulse compares it with !==. */
    @Override public Location getLocation() {
        Support.emit("npc.location", Support.list(rawLocation.x, rawLocation.y, rawLocation.z, Support.identity(rawLocation)), ref);
        return rawLocation;
    }
    public boolean isInvisible() { return Support.emit("npc.isInvisible", rawInvisible, ref); }
    /** Compiled against by the original Skills in the binding corpus; never reached for a player. */
    public Definition getDefinition() { return new Definition(); }
    public static class Definition { public int getCombatLevel() { throw new UnsupportedOperationException(); } }
}
