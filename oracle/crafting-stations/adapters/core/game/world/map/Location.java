package core.game.world.map;
import oracle.W;
/** Adapter: coordinates; the player's location answers from declared case state. */
public final class Location {
    public final int x, y, z;
    private final boolean players;
    private Location(int x, int y, int z, boolean players) { this.x = x; this.y = y; this.z = z; this.players = players; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z, false); }
    public static Location ofPlayer() { return new Location(0, 0, 0, true); }
    public boolean withinDistance(Location other) {
        W.emit("withinDistance", other.x, other.y, other.z);
        return W.flag("near");
    }
    public int getRegionId() { W.emit("regionId"); return W.number("region"); }
}
