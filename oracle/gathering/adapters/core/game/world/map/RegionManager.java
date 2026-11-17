package core.game.world.map;
import core.game.node.entity.player.Player;
import oracle.Support;
public class RegionManager {
    public static java.util.List<Player> getLocalPlayers(Location l, int distance) {
        java.util.List<Player> out = new java.util.ArrayList<>();
        java.util.List<Integer> refs = new java.util.ArrayList<>();
        for (int id : Support.locals) { Player p = id == 0 ? Support.player : Support.others.get(id - 1); out.add(p); refs.add(p.ref); }
        Support.emit("region.localPlayers", refs, l, distance);
        return out;
    }
}
