package core.game.world.map;
/** Adapter: three coordinates, with the original's create and equals. */
public final class Location {
    private final int x, y, z;
    public Location(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public int getX() { return x; }
    public int getY() { return y; }
    public int getZ() { return z; }
    @Override public boolean equals(Object other) {
        if (!(other instanceof Location)) return false;
        Location loc = (Location) other;
        return loc.x == x && loc.y == y && loc.z == z;
    }
    @Override public int hashCode() { return z << 30 | x << 15 | y; }
}
