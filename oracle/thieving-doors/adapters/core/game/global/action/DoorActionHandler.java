package core.game.global.action;
import core.game.node.entity.Entity;
import core.game.node.scenery.Scenery;
import core.game.world.map.Location;
import oracle.W;
/** Adapter: records the call and answers the operation's declared tile. */
public final class DoorActionHandler {
    public static Location getDestination(Entity entity, Scenery object) {
        W.emit("doorDestination");
        return W.location("destination");
    }
}
