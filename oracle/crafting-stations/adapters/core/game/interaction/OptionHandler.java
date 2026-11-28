package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import core.plugin.Plugin;
/** Adapter: the abstract handle and the destination a subclass may override. */
public abstract class OptionHandler implements Plugin<Object> {
    public abstract boolean handle(Player player, Node node, String option);
    public Location getDestination(Node n, Node node) { return null; }
    @Override public Object fireEvent(String identifier, Object... args) { return null; }
}
