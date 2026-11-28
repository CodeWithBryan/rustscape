package core.net.packet.context;
import core.game.node.entity.player.Player;
/** Adapter. */
public final class ChildPositionContext {
    public final int interfaceId, childId, x, y;
    public ChildPositionContext(Player player, int interfaceId, int childId, int positionX, int positionY) {
        this.interfaceId = interfaceId; this.childId = childId; this.x = positionX; this.y = positionY;
    }
}
