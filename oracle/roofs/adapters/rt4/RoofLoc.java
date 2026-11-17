package rt4;

import java.util.HashMap;

/**
 * What SceneGraph.addLoc needs of a LocType to decide whether it has an entity
 * to place. The values are supplied by the generator from the Rust decoder;
 * the original LocType is not run.
 */
public final class RoofLoc {
    private static final HashMap<Integer, RoofLoc> TYPES = new HashMap<Integer, RoofLoc>();

    public int width;
    public int length;
    /** LocType.render (opcode 82). */
    public boolean render;
    /** anim != -1 || multiLocs != null || dynamic: addLoc makes a Loc. */
    public boolean animated;
    public int models;
    /** LocType.shapes, or null. */
    public int[] shapes;

    public static void put(int id, RoofLoc type) {
        TYPES.put(id, type);
    }

    public static void clear() {
        TYPES.clear();
    }

    public static RoofLoc get(int id) {
        RoofLoc type = TYPES.get(id);
        if (type == null) {
            throw new IllegalStateException("no facts for location " + id);
        }
        return type;
    }

    /**
     * Null exactly where LocType.getStaticEntity leaves the model null:
     * getGlModel returns null without shapes unless the shape is 10 and there
     * are models, and with shapes unless the shape is listed. Models are taken
     * to be present in the cache.
     */
    public Entity entity(int shape) {
        if (this.animated) {
            return new Entity();
        }
        if (this.shapes == null) {
            if (shape != 10 || this.models == 0) {
                return null;
            }
            return new Entity();
        }
        for (int i = 0; i < this.shapes.length; i++) {
            if (this.shapes[i] == shape) {
                return new Entity();
            }
        }
        return null;
    }
}
