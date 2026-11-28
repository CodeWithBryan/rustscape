package oracle;

import content.global.skill.fletching.arrow.ArrowListeners;
import content.global.skill.fletching.bolts.BoltListeners;
import content.global.skill.fletching.crossbow.LimbingListener;
import content.global.skill.fletching.darts.DartListeners;
import content.global.skill.fletching.gem.GemBoltListeners;
import content.global.skill.fletching.grapple.GrappleListeners;
import content.global.skill.fletching.log.LogCraftableListeners;
import content.global.skill.fletching.stringing.StringingListeners;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.Map;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified fletching sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static final Player PLAYER = new Player();

    public static void main(String[] args) throws Exception {
        JSONParser parser = new JSONParser();
        JSONObject names = (JSONObject) parser.parse(new String(Files.readAllBytes(Paths.get(args[0])), StandardCharsets.UTF_8));
        for (Object o : names.entrySet()) {
            Map.Entry<?, ?> e = (Map.Entry<?, ?>) o;
            W.names.put(Integer.parseInt((String) e.getKey()), (String) e.getValue());
        }
        // The classes register once, in this declared order.
        new LogCraftableListeners().defineListeners();
        new StringingListeners().defineListeners();
        new ArrowListeners().defineListeners();
        new BoltListeners().defineListeners();
        new DartListeners().defineListeners();
        new LimbingListener().defineListeners();
        new GemBoltListeners().defineListeners();
        new GrappleListeners().defineListeners();
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintStream out = new PrintStream(System.out, false, "UTF-8");
        for (String line; (line = in.readLine()) != null; ) {
            if (line.isEmpty()) continue;
            JSONObject c = (JSONObject) parser.parse(line);
            c.put("expected", run(c));
            out.println(c.toJSONString());
        }
        out.flush();
    }

    @SuppressWarnings("unchecked")
    private static JSONArray run(JSONObject c) {
        W.state = (JSONObject) new JSONParserCopy().copy(c.get("state"));
        W.handler = null;
        W.scripts.clear();
        JSONArray results = new JSONArray();
        for (Object o : (JSONArray) c.get("ops")) {
            JSONObject op = (JSONObject) o;
            W.trace = new JSONArray();
            JSONObject result = new JSONObject();
            try {
                result.put("result", step(op));
            } catch (RuntimeException e) {
                result.put("throws", failure(e));
            }
            result.put("trace", W.trace);
            results.add(result);
        }
        return results;
    }

    private static String failure(RuntimeException e) {
        if (e instanceof NullPointerException) return "Null";
        if (e instanceof IndexOutOfBoundsException) return "Index";
        if (e instanceof ClassCastException) return "Cast";
        return e.getClass().getName();
    }

    private static int number(JSONObject op, String key) { return ((Number) op.get(key)).intValue(); }

    private static boolean has(int[] ids, int id) {
        for (int i : ids) if (i == id) return true;
        return false;
    }

    @SuppressWarnings("unchecked")
    private static Object step(JSONObject op) {
        String kind = (String) op.get("op");
        switch (kind) {
            case "registrations": {
                JSONArray all = new JSONArray();
                for (W.Registration r : W.registrations) {
                    JSONArray row = new JSONArray(), used = new JSONArray(), with = new JSONArray();
                    for (int i : r.used) used.add(i);
                    for (int i : r.with) with.add(i);
                    row.add(r.type); row.add(used); row.add(with);
                    all.add(row);
                }
                return all;
            }
            case "set":
                W.state.putAll((JSONObject) new JSONParserCopy().copy(op.get("state")));
                return null;
            case "use": {
                // A declared model of the use-with lookup: the pair as used, then flipped.
                int used = number(op, "used"), with = number(op, "with");
                for (W.Registration r : W.registrations)
                    if (has(r.used, used) && has(r.with, with))
                        return r.handler.invoke(PLAYER, new Item(used), new Item(with));
                for (W.Registration r : W.registrations)
                    if (has(r.used, with) && has(r.with, used))
                        return r.handler.invoke(PLAYER, new Item(with), new Item(used));
                return "unhandled";
            }
            case "create":
                if (W.handler == null) return "no handler";
                W.handler.create(number(op, "amount"), number(op, "index"));
                return null;
            case "getAll":
                if (W.handler == null) return "no handler";
                return W.handler.getAll(number(op, "index"));
            case "script":
                // The script the case names by its place in the queueing order, or the last one queued.
                int n = op.containsKey("n") ? number(op, "n") : W.scripts.size() - 1;
                if (n < 0 || n >= W.scripts.size()) return "no script";
                return W.scripts.get(n).invoke(number(op, "stage"));
            default:
                throw new IllegalArgumentException(kind);
        }
    }

    /** A deep copy through the JSON text, so a case never shares state. */
    private static final class JSONParserCopy {
        Object copy(Object value) {
            try {
                return new JSONParser().parse(JSONObject.toJSONString(java.util.Collections.singletonMap("v", value)).replaceFirst("^\\{\"v\":", "").replaceFirst("\\}$", ""));
            } catch (org.json.simple.parser.ParseException e) {
                throw new IllegalStateException(e);
            }
        }
    }
}
