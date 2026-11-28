package oracle;

import content.global.skill.herblore.FinishedPotion;
import content.global.skill.herblore.HerbCleanListener;
import content.global.skill.herblore.Herbs;
import content.global.skill.herblore.PotionMixingListener;
import content.global.skill.herblore.UnfinishedPotion;
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

/** Drives the unmodified herblore sources over the recording world, one JSON case per line. */
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
        new HerbCleanListener().defineListeners();
        new PotionMixingListener().defineListeners();
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
    private static JSONArray run(JSONObject c) throws Exception {
        W.state = (JSONObject) parser().parse(((JSONObject) c.get("state")).toJSONString());
        W.handler = null;
        W.pulse = null;
        W.drawn = 0;
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

    private static JSONParser parser() { return new JSONParser(); }

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
    private static JSONArray list(Object... values) {
        JSONArray row = new JSONArray();
        for (Object v : values) row.add(v);
        return row;
    }

    @SuppressWarnings("unchecked")
    private static Object step(JSONObject op) throws Exception {
        String kind = (String) op.get("op");
        switch (kind) {
            case "registrations": {
                JSONArray all = new JSONArray();
                for (W.Registration r : W.registrations) {
                    if (r.option != null) {
                        JSONArray options = new JSONArray();
                        for (String o : r.options) options.add(o);
                        all.add(list("on", r.type, options));
                    } else {
                        JSONArray used = new JSONArray(), with = new JSONArray();
                        for (int i : r.used) used.add(i);
                        for (int i : r.with) with.add(i);
                        all.add(list("onUseWith", r.type, used, with));
                    }
                }
                return all;
            }
            case "tables": {
                JSONArray herbs = new JSONArray(), unfinished = new JSONArray(), finished = new JSONArray();
                for (Herbs h : Herbs.values())
                    herbs.add(list(h.name(), h.getHerb().getId(), W.bits(h.getExperience()), h.getLevel(), h.getProduct().getId()));
                for (UnfinishedPotion p : UnfinishedPotion.values())
                    unfinished.add(list(p.name(), p.getBase().getId(), p.getIngredient().getId(), p.getLevel(), p.getPotion().getId()));
                for (FinishedPotion p : FinishedPotion.values())
                    finished.add(list(p.name(), p.getUnfinished().getId(), p.getIngredient().getId(), p.getLevel(), W.bits(p.getExperience()), p.getPotion().getId()));
                return list(herbs, unfinished, finished);
            }
            case "canMake": {
                JSONArray all = new JSONArray();
                for (FinishedPotion p : FinishedPotion.values()) all.add(p.canMake(PLAYER));
                return all;
            }
            case "set":
                W.state.putAll((JSONObject) parser().parse(((JSONObject) op.get("state")).toJSONString()));
                return null;
            case "clean": {
                // The one generic "clean" registration, with an item of the id and slot the operation names.
                Item item = new Item(number(op, "id"));
                item.setSlot(number(op, "slot"));
                for (W.Registration r : W.registrations)
                    if (r.option != null) return r.option.invoke(PLAYER, item);
                return "unhandled";
            }
            case "use": {
                // A declared model of the use-with lookup: the pair as used, then flipped.
                int used = number(op, "used"), with = number(op, "with");
                for (W.Registration r : W.registrations)
                    if (r.use != null && has(r.used, used) && has(r.with, with))
                        return r.use.invoke(PLAYER, new Item(used), new Item(with));
                for (W.Registration r : W.registrations)
                    if (r.use != null && has(r.used, with) && has(r.with, used))
                        return r.use.invoke(PLAYER, new Item(with), new Item(used));
                return "unhandled";
            }
            case "create":
                if (W.handler == null) return "no handler";
                W.handler.create(number(op, "amount"), number(op, "index"));
                return null;
            case "getAll":
                if (W.handler == null) return "no handler";
                return W.handler.getAll(number(op, "index"));
            case "pulse": {
                // SkillPulse.pulse() of the pulse the last run kept: its answer, then its delay and running flag.
                if (W.pulse == null) return "no pulse";
                boolean done = W.pulse.pulse();
                return list(done, W.pulse.getDelay(), W.pulse.isRunning());
            }
            case "stop":
                if (W.pulse == null) return "no pulse";
                W.pulse.stop();
                return list(W.pulse.getDelay(), W.pulse.isRunning());
            default:
                throw new IllegalArgumentException(kind);
        }
    }
}
