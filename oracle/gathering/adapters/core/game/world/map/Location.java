package core.game.world.map;
public class Location {
    public final int x, y, z;
    public Location(int x, int y) { this(x, y, 0); }
    public Location(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public int getX() { return x; } public int getY() { return y; } public int getZ() { return z; }
    public int getRegionId() { return x >> 6 << 8 | y >> 6; }
    public Location transform(int dx, int dy, int dz) { return new Location(x + dx, y + dy, z + dz); }
    /** The pinned expressions, spliced into LocationOriginal.kt. */
    public Location transform(Direction dir) { return LocationOriginalKt.originalTransform(this, dir, 1); }
    public boolean withinMaxnormDistance(Location other, int dist) { return LocationOriginalKt.originalWithinMaxnormDistance(this, other, dist); }
    @Override public boolean equals(Object o) { return o instanceof Location && ((Location) o).x == x && ((Location) o).y == y && ((Location) o).z == z; }
    @Override public int hashCode() { return z << 30 | x << 15 | y; }
}
