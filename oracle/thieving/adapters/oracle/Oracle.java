package oracle;

import content.global.skill.thieving.Pickpockets;
import content.global.skill.thieving.ThievingListeners;
import core.api.utils.WeightedItem;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified pickpocketing sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static final Player PLAYER = new Player();

    public static void main(String[] args) throws Exception {
        JSONParser parser = new JSONParser();
        new ThievingListeners().defineListeners();
        // The enum's tables are made here, once; each is named after its constant for the log.
        for (Pickpockets p : Pickpockets.values()) p.getTable().setOwner(p.name());
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
                for (W.Registration r : W.registrations) {
                    JSONArray row = new JSONArray(), options = new JSONArray();
                    for (String option : r.options) options.add(option);
                    row.add(r.type); row.add(options);
                    all.add(row);
                }
                return all;
            }
            case "tables": {
                JSONArray all = new JSONArray();
                for (Pickpockets p : Pickpockets.values()) {
                    JSONObject row = new JSONObject();
                    JSONArray ids = new JSONArray(), entries = new JSONArray();
                    for (int id : p.getIds()) ids.add(id);
                    for (WeightedItem e : p.getTable().getEntries()) {
                        JSONArray entry = new JSONArray();
                        entry.add(e.getId()); entry.add(e.getMinAmt()); entry.add(e.getMaxAmt());
                        entry.add(W.bits(e.getWeight())); entry.add(e.getGuaranteed());
                        entries.add(entry);
                    }
                    row.put("name", p.name()); row.put("ids", ids); row.put("level", p.getRequiredLevel());
                    row.put("low", W.bits(p.getLow())); row.put("high", W.bits(p.getHigh()));
                    row.put("experience", W.bits(p.getExperience()));
                    row.put("stunDamageMin", p.getStunDamageMin()); row.put("stunDamageMax", p.getStunDamageMax());
                    row.put("stunTime", p.getStunTime()); row.put("entries", entries);
                    all.add(row);
                }
                return all;
            }
            case "forID": {
                Pickpockets p = Pickpockets.forID(((Number) op.get("npc")).intValue());
                return p == null ? null : p.name();
            }
            case "set":
                W.state.putAll((JSONObject) copy(op.get("state")));
                return null;
            case "pickpocket":
                // The one registration's lambda, with an NPC of the id the case names.
                return W.registrations.get(0).handler.invoke(PLAYER, new NPC(((Number) op.get("npc")).intValue()));
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
