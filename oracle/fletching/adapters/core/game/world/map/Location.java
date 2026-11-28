package core.game.world.map;
/** Adapter: three coordinates. */
public final class Location {
    private final int x, y, z;
    public Location(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
    public int getX() { return x; } public int getY() { return y; } public int getZ() { return z; }
}
