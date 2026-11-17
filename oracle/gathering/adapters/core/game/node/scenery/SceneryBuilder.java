package core.game.node.scenery;
import oracle.Support;
public class SceneryBuilder {
    public static boolean replace(Scenery remove, Scenery construct, int restoreTicks) {
        return Support.emit("sceneryBuilder.replace", true, Support.ref(remove), Support.ref(construct), restoreTicks);
    }
}
