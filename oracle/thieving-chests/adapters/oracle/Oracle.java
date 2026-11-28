package oracle;

import content.global.skill.thieving.ThievableChestPlugin;
import content.global.skill.thieving.ThievableChestPlugin.Chest;
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

/** Drives the unmodified chest source over the recording world, one JSON case per line. */
public final class Oracle {
    private static final Player PLAYER = new Player();

    public static void main(String[] args) throws Throwable {
        JSONParser parser = new JSONParser();
        new ThievableChestPlugin().newInstance(null);
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
        // The enum keeps each constant's respawn tick between cases: it is set back to 0 through setRespawn().
        W.trace = new JSONArray();
        Object ticks = W.state.get("ticks");
        for (Chest chest : Chest.values()) {
            W.state.put("ticks", -(int) (chest.getRespawn() / 0.6));
            chest.setRespawn();
        }
        W.state.put("ticks", ticks);
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
            case "registrations":
                return W.registrations;
            case "chests": {
                JSONArray all = new JSONArray();
                for (Chest chest : Chest.values()) {
                    JSONObject row = new JSONObject();
                    JSONArray ids = new JSONArray(), rewards = new JSONArray();
                    for (int id : chest.getObjectIds()) ids.add(id);
                    for (Item item : chest.getRewards()) {
                        JSONArray pair = new JSONArray();
                        pair.add(item.getId()); pair.add(item.getAmount());
                        rewards.add(pair);
                    }
                    row.put("name", chest.name()); row.put("ids", ids); row.put("level", chest.getLevel());
                    row.put("experience", W.bits(chest.getExperience())); row.put("rewards", rewards);
                    row.put("respawn", chest.getRespawn());
                    all.add(row);
                }
                return all;
            }
            case "forId": {
                Chest chest = Chest.forId(((Number) op.get("node")).intValue());
                return chest == null ? null : chest.name();
            }
            case "respawning":
                return Chest.valueOf((String) op.get("chest")).isRespawning();
            case "set":
                W.state.putAll((JSONObject) copy(op.get("state")));
                return null;
            case "handle":
                // The registered plugin, with a scenery of the id the case names.
                return W.handler.handle(PLAYER, new Scenery(((Number) op.get("node")).intValue()), (String) op.get("option"));
            case "update":
                // No pulse was submitted in this case: nothing to run.
                if (W.pulse == null) return "none";
                return W.pulse.update();
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
