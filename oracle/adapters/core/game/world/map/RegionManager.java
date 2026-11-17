package core.game.world.map;

/** Fail closed: these tests exercise Direction.canMoveFrom with an explicit supplier. */
public final class RegionManager {
    public static int getClippingFlag(int z, int x, int y) {
        throw new UnsupportedOperationException("RegionManager is outside this isolated oracle");
    }
}
