package core.game.world.map;
import core.game.node.entity.npc.NPC;
import java.util.ArrayList;
import java.util.List;
import oracle.W;
import org.json.simple.JSONArray;
/** Adapter: the declared NPCs, in order. */
public final class RegionManager {
    public static List<NPC> getLocalNPCs(Location location, int distance) {
        W.emit("localNPCs", distance);
        List<NPC> all = new ArrayList<>();
        JSONArray npcs = (JSONArray) W.state.get("npcs");
        for (int i = 0; i < npcs.size(); i++) {
            JSONArray row = (JSONArray) npcs.get(i);
            all.add(new NPC(i, ((Number) row.get(0)).intValue(), Boolean.TRUE.equals(row.get(1))));
        }
        return all;
    }
}
