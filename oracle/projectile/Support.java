package oracle;

import core.api.ContentAPIKt;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.world.map.Location;
import core.game.world.map.Point;
import core.game.world.map.path.Path;
import core.game.world.map.path.Pathfinder;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.*;

public final class Support {
    private static JSONObject row;
    private static Location start, end;
    private static List<List<Object>> queries;
    private static Map<List<Object>, Integer> flags;

    private static int num(Object value) { return ((Number)value).intValue(); }
    private static int field(String key, int fallback) {
        return row.containsKey(key) ? num(row.get(key)) : fallback;
    }
    private static Location location(Object value) {
        if (value == null) return null;
        var xy = (JSONArray)value;
        return new Location(num(xy.get(0)), num(xy.get(1)), num(xy.get(2)));
    }
    public static int flag(boolean projectile, int z, int x, int y) {
        queries.add(List.of(projectile,z,x,y));
        if (queries.size() == field("mutateAt", -1)) {
            var target = "start".equals(row.get("mutateTarget")) ? start : end;
            var replace = location(row.get("replace"));
            target.replace(replace.getX(), replace.getY(), replace.getZ());
        }
        if (queries.size() == field("failAt", -1)) throw new IllegalStateException("flag read");
        return flags.getOrDefault(List.of(projectile,x,y), field(projectile ? "defaultProjectile" : "defaultWalking", 0));
    }
    private static Map<String, Object> result(JSONObject input) {
        row = input;
        start = location(row.get("start"));
        end = location(row.get("end"));
        queries = new ArrayList<>();
        flags = new HashMap<>();
        if (row.get("flags") instanceof JSONArray) for (Object item : (JSONArray) row.get("flags")) {
            var a = (JSONArray)item;
            flags.put(List.of(a.get(0), num(a.get(1)), num(a.get(2))), num(a.get(3)));
        }
        var output = new HashMap<String,Object>();
        output.put("queries",queries);
        try {
            if (row.get("kind") instanceof String) {
                String kind = (String) row.get("kind");
                var source = new Entity();
                source.size = field("sourceSize", 1); source.location = start;
                Node victim = null;
                if (end != null) {
                    victim = new Node(); victim.size = field("victimSize", 1); victim.location = end;
                }
                output.put("reachable", kind.equals("los") ? ContentAPIKt.hasLineOfSight(source, victim)
                    : CombatSwingHandler.isProjectileClipped(source, victim, Boolean.TRUE.equals(row.get("close"))));
            } else {
                // Most parameters (including the supplier) are ignored by the original.
                Path path = Pathfinder.PROJECTILE.find(start,99,end,-1,2,-3,4,5,true,
                    (z,x,y) -> {throw new AssertionError("projectile unexpectedly consulted supplier");});
                var points = new ArrayList<List<Integer>>();
                for (Point p : path.getPoints()) points.add(List.of(p.getX(), p.getY(), p.getDirection().value));
                output.put("successful",path.isSuccessful());
                output.put("moveNear",path.isMoveNear());
                output.put("points",points);
            }
        } catch (NullPointerException failure) {
            output.put("error", "Null");
        } catch (IllegalStateException failure) {
            output.put("error", "Binding");
        }
        return output;
    }
    public static void main(String[] args) throws Exception {
        try (var reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                var input = (JSONObject)new JSONParser().parse(line);
                var output = result(input);
                System.out.println(JSONObject.toJSONString(Map.of("input", input, "expected", output)));
            }
        }
    }
}
