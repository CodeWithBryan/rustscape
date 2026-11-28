package oracle;

import content.global.handlers.iface.JewelleryInterface;
import content.global.skill.crafting.JewelleryCraftPlugin;
import content.global.skill.crafting.jewellery.JewelleryCrafting;
import content.global.skill.crafting.jewellery.JewelleryCrafting.JewelleryItem;
import core.game.component.Component;
import core.game.interaction.NodeUsageEvent;
import core.game.node.item.Item;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified jewellery casting sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static JewelleryCraftPlugin plugin;
    private static JewelleryInterface iface;

    public static void main(String[] args) throws Throwable {
        JSONParser parser = new JSONParser();
        plugin = new JewelleryCraftPlugin();
        plugin.newInstance(null);
        iface = new JewelleryInterface();
        iface.newInstance(null);
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
        W.pulse = null;
        W.input = null;
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
                all.add(W.list("allowed", plugin.getClass().getName(), ints(plugin.allowedNodes)));
                for (W.Handler h : W.handlers)
                    all.add(W.list("addHandler", h.handler.getClass().getName(), h.id, h.type));
                for (W.Plugin p : W.plugins)
                    all.add(W.list("component", p.id, p.plugin.getClass().getName()));
                return all;
            }
            case "tables": {
                JSONArray rows = new JSONArray();
                for (JewelleryItem j : JewelleryItem.values())
                    rows.add(W.list(j.name(), j.getLevel(), W.bits(j.getExperience()), j.getComponentId(), j.getSendItem(), ints(j.getItems()),
                                    JewelleryCrafting.mouldFor(j.name()), JewelleryItem.forProduct(j.getSendItem()).name()));
                return rows;
            }
            case "names": {
                JSONArray rows = new JSONArray();
                for (String[] row : core.cache.def.impl.ItemDefinition.NAMES) rows.add(W.list(Integer.parseInt(row[0]), row[1]));
                return rows;
            }
            case "set":
                W.state.putAll((JSONObject) new JSONParser().parse(((JSONObject) op.get("state")).toJSONString()));
                return null;
            case "handle":
                // JewelleryCraftPlugin.handle with an event of two items, not UseWithHandler.run.
                return plugin.handle(new NodeUsageEvent(W.player, 0, new Item(number(op, "used")), new Item(number(op, "with"))));
            case "button":
                return iface.handle(W.player, new Component(446), number(op, "opcode"), number(op, "button"), 0, 0);
            case "input": {
                // The last amount prompt's handler, with an Integer or a String.
                if (W.input == null) return "no input";
                Object value = op.get("value");
                W.input.invoke(value instanceof Number ? (Object) ((Number) value).intValue() : value);
                return null;
            }
            case "pulse": {
                // pulse() of the pulse the last run kept: its answer, its delay and whether it is running.
                if (W.pulse == null) return "no pulse";
                boolean done = W.pulse.pulse();
                return W.list(done, W.pulse.getDelay(), W.pulse.isRunning());
            }
            case "stop": {
                if (W.pulse == null) return "no pulse";
                W.pulse.stop();
                return W.pulse.isRunning();
            }
            default:
                throw new IllegalArgumentException(kind);
        }
    }
}
