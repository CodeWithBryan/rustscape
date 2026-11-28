package oracle;

import content.global.skill.thieving.Stall;
import content.global.skill.thieving.StallPeek;
import content.global.skill.thieving.ThievingOptionPlugin;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified stall sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static final Player PLAYER = new Player();

    public static void main(String[] args) throws Throwable {
        JSONParser parser = new JSONParser();
        new ThievingOptionPlugin().newInstance(null);
        // A pulse body that throws is printed by Pulse.update; the log is not part of the corpus.
        System.setErr(new PrintStream(java.io.OutputStream.nullOutputStream()));
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
        W.state = (JSONObject) copy(c.get("state"));
        W.pulse = null;
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

    @SuppressWarnings("unchecked")
    private static Object step(JSONObject op) {
        String kind = (String) op.get("op");
        switch (kind) {
            case "registrations": {
                JSONArray all = new JSONArray();
                all.addAll(W.options);
                return all;
            }
            case "stalls": {
                JSONArray all = new JSONArray();
                for (Stall s : Stall.values()) {
                    JSONObject row = new JSONObject();
                    JSONArray full = new JSONArray(), empty = new JSONArray(), rewards = new JSONArray();
                    full.addAll(s.getFullIDs());
                    empty.addAll(StallPeek.emptyIds(s));
                    for (Item item : s.getRewards()) {
                        JSONArray pair = new JSONArray();
                        pair.add(item.getId()); pair.add(item.getAmount());
                        rewards.add(pair);
                    }
                    row.put("name", s.name()); row.put("full", full); row.put("empty", empty);
                    row.put("level", s.getLevel()); row.put("rewards", rewards);
                    row.put("experience", W.bits(s.getExperience())); row.put("delay", s.getDelay());
                    row.put("msgItem", StallPeek.msgItem(s));
                    all.add(row);
                }
                return all;
            }
            case "forObject": {
                Stall s = Stall.forObject(new Scenery(((Number) op.get("node")).intValue()));
                return s == null ? null : s.name();
            }
            case "empty": {
                int id = ((Number) op.get("node")).intValue();
                return Stall.valueOf((String) op.get("stall")).getEmpty(id);
            }
            case "set":
                W.state.putAll((JSONObject) copy(op.get("state")));
                return null;
            case "handle":
                // The registered plugin, with a scenery of the id the case names.
                return W.handler.handle(PLAYER, new Scenery(((Number) op.get("node")).intValue()), (String) op.get("option"));
            case "update":
                return W.pulse.update();
            case "stop":
                W.pulse.stop();
                return null;
            default:
                throw new IllegalArgumentException(kind);
        }
    }

    /** A deep copy through the JSON text, so a case never shares state. */
    private static Object copy(Object value) {
        try {
            return new JSONParser().parse(JSONObject.toJSONString(java.util.Collections.singletonMap("v", value)).replaceFirst("^\\{\"v\":", "").replaceFirst("\\}$", ""));
        } catch (org.json.simple.parser.ParseException e) {
            throw new IllegalStateException(e);
        }
    }
}
