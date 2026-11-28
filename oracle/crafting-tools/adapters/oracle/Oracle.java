package oracle;

import content.global.skill.crafting.AmuletStringingPlugin;
import content.global.skill.crafting.ChiselLimestonePlugin;
import content.global.skill.crafting.GemCutPlugin;
import content.global.skill.crafting.gem.Gems;
import content.global.skill.crafting.jewellery.JewelleryCrafting.JewelleryItem;
import content.global.skill.crafting.silver.SilverProduct;
import content.global.skill.crafting.silver.SilverStringingListener;
import core.game.interaction.NodeUsageEvent;
import core.game.interaction.UseWithHandler;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified gem cutting, limestone, amulet stringing and silver stringing sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static final Player PLAYER = new Player();
    private static final java.util.Map<String, UseWithHandler> PLUGINS = new java.util.HashMap<>();

    public static void main(String[] args) throws Throwable {
        JSONParser parser = new JSONParser();
        // Two of the three newInstance bodies answer null: the instances are kept here.
        new SilverStringingListener().defineListeners();
        UseWithHandler amulet = new AmuletStringingPlugin(), limestone = new ChiselLimestonePlugin(), gem = new GemCutPlugin();
        amulet.newInstance(null);
        limestone.newInstance(null);
        gem.newInstance(null);
        PLUGINS.put("amulet", amulet);
        PLUGINS.put("limestone", limestone);
        PLUGINS.put("gem", gem);
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
        W.state = (JSONObject) new JSONParser().parse(((JSONObject) c.get("state")).toJSONString());
        W.handler = null;
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
    private static JSONArray ints(int[] values) {
        JSONArray row = new JSONArray();
        for (int v : values) row.add(v);
        return row;
    }

    @SuppressWarnings("unchecked")
    private static Object step(JSONObject op) throws Exception {
        String kind = (String) op.get("op");
        switch (kind) {
            case "registrations": {
                JSONArray all = new JSONArray();
                for (W.Registration r : W.registrations)
                    all.add(list("onUseWith", r.type, r.use.getClass().getName(), ints(r.used), ints(r.with)));
                for (W.Handler h : W.handlers)
                    all.add(list("addHandler", h.handler.getClass().getSimpleName(), ints(h.handler.allowedNodes), h.id, h.type));
                return all;
            }
            case "tables": {
                JSONArray gems = new JSONArray(), jewellery = new JSONArray(), silver = new JSONArray();
                for (Gems g : Gems.values())
                    gems.add(list(g.name(), g.getUncut().getId(), g.getUncut().getAmount(), g.getGem().getId(), g.getGem().getAmount(), g.getLevel(),
                                  g.getAnimation().getId(), g.getAnimation().getDelay(), g.getAnimation().getPriority().name(), W.bits(g.getExp())));
                for (JewelleryItem j : JewelleryItem.values())
                    jewellery.add(list(j.name(), j.getLevel(), j.getSendItem(), JewelleryItem.forProduct(j.getSendItem()).name()));
                for (SilverProduct s : SilverProduct.values())
                    silver.add(list(s.name(), s.getProducedItemId(), s.getStrungId(), SilverProduct.Companion.forProductID(s.getProducedItemId()).name()));
                return list(gems, jewellery, silver);
            }
            case "set":
                W.state.putAll((JSONObject) new JSONParser().parse(((JSONObject) op.get("state")).toJSONString()));
                return null;
            case "handle": {
                // UseWithHandler.handle of the named plugin with an event of two items, not UseWithHandler.run.
                UseWithHandler plugin = PLUGINS.get((String) op.get("plugin"));
                return plugin.handle(new NodeUsageEvent(PLAYER, 0, new Item(number(op, "used")), new Item(number(op, "with"))));
            }
            case "use": {
                // A declared model of the use-with lookup: the pair as used, then flipped.
                int used = number(op, "used"), with = number(op, "with");
                for (W.Registration r : W.registrations)
                    if (has(r.used, used) && has(r.with, with))
                        return r.use.invoke(PLAYER, new Item(used), new Item(with));
                for (W.Registration r : W.registrations)
                    if (has(r.used, with) && has(r.with, used))
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
