package core.game.node.scenery;
import oracle.W;
/** Adapter: records a replacement; what it does to the scene is not covered. */
public final class SceneryBuilder {
    public static boolean replace(Scenery remove, Scenery construct, int restoreTicks) {
        W.emit("replace", remove.getId(), construct.getId(), restoreTicks);
        return true;
    }
}
