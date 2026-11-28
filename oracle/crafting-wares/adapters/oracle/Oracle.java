package oracle;

import content.global.skill.crafting.BattlestaffListener;
import content.global.skill.crafting.BattlestaffListener.BattlestaffProduct;
import content.global.skill.crafting.glass.GlassCraftingListener;
import content.global.skill.crafting.glass.GlassMakeListener;
import content.global.skill.crafting.glass.GlassProduct;
import content.global.skill.crafting.leather.SpikyVambraces;
import core.game.component.Component;
import core.game.interaction.NodeUsageEvent;
import core.game.interaction.UseWithHandler;
import core.game.node.item.Item;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified battlestaff, glass and spiky vambraces sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static UseWithHandler vambraces;

    public static void main(String[] args) throws Throwable {
        JSONParser parser = new JSONParser();
        new BattlestaffListener().defineListeners();
        GlassCraftingListener glass = new GlassCraftingListener();
        glass.defineListeners();
        glass.defineInterfaceListeners();
        new GlassMakeListener().defineListeners();
        vambraces = new SpikyVambraces();
        vambraces.newInstance(null);
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
                for (W.Button b : W.buttons)
                    all.add(list("on", b.component, b.handler.getClass().getName()));
                all.add(list("allowed", vambraces.getClass().getSimpleName(), ints(vambraces.allowedNodes)));
                for (W.Handler h : W.handlers)
                    all.add(list("addHandler", h.handler.getClass().getSimpleName(), h.id, h.type));
                return all;
            }
            case "tables": {
                JSONArray staves = new JSONArray(), glass = new JSONArray();
                for (BattlestaffProduct s : BattlestaffProduct.values())
                    staves.add(list(s.name(), s.getRequiredOrbItemId(), s.getProducedItemId(), s.getAmountProduced(), s.getMinimumLevel(),
                                    W.bits(s.getExperience()), BattlestaffProduct.Companion.getProductMap().get(s.getRequiredOrbItemId()).name()));
                for (GlassProduct g : GlassProduct.values())
                    glass.add(list(g.name(), g.getButtonId(), g.getProducedItemId(), g.getAmount(), g.getMinimumLevel(), W.bits(g.getExperience()),
                                   GlassProduct.Companion.forButtonID(g.getButtonId()).name()));
                return list(staves, glass);
            }
            case "set":
                W.state.putAll((JSONObject) new JSONParser().parse(((JSONObject) op.get("state")).toJSONString()));
                return null;
            case "handle":
                // SpikyVambraces.handle with an event of two items, not UseWithHandler.run.
                return vambraces.handle(new NodeUsageEvent(W.player, 0, new Item(number(op, "used")), new Item(number(op, "with"))));
            case "use": {
                // A declared model of the use-with lookup: the pair as used, then flipped. Both nodes are Items here.
                int used = number(op, "used"), with = number(op, "with");
                for (W.Registration r : W.registrations)
                    if (has(r.used, used) && has(r.with, with))
                        return r.use.invoke(W.player, new Item(used), new Item(with));
                for (W.Registration r : W.registrations)
                    if (has(r.used, with) && has(r.with, used))
                        return r.use.invoke(W.player, new Item(with), new Item(used));
                return "unhandled";
            }
            case "button": {
                int component = number(op, "component");
                for (W.Button b : W.buttons)
                    if (b.component == component)
                        return b.handler.invoke(W.player, new Component(component), number(op, "opcode"), number(op, "button"), 0, 0);
                return "unhandled";
            }
            case "input":
                if (W.input == null) return "no input";
                W.input.invoke(number(op, "value"));
                return null;
            case "create":
                if (W.handler == null) return "no handler";
                W.handler.create(number(op, "amount"), number(op, "index"));
                return null;
            case "getAll":
                if (W.handler == null) return "no handler";
                return W.handler.getAll(number(op, "index"));
            case "pulse": {
                // pulse() of the pulse the last run kept: its answer, then its delay.
                if (W.pulse == null) return "no pulse";
                boolean done = W.pulse.pulse();
                return list(done, W.pulse.getDelay());
            }
            default:
                throw new IllegalArgumentException(kind);
        }
    }
}
