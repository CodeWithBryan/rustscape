package oracle;

import content.global.handlers.iface.SpinningInterface;
import content.global.handlers.scenery.SpinSpinningWheelPlugin;
import content.global.skill.crafting.silver.SilverCraftingListener;
import content.global.skill.crafting.silver.SilverProduct;
import content.global.skill.crafting.spinning.SpinningItem;
import core.game.component.Component;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/** Drives the unmodified spinning and silver casting sources over the recording world, one JSON case per line. */
public final class Oracle {
    private static SpinSpinningWheelPlugin wheel;
    private static SpinningInterface spinning;

    public static void main(String[] args) throws Throwable {
        JSONParser parser = new JSONParser();
        wheel = new SpinSpinningWheelPlugin();
        wheel.newInstance(null);
        spinning = new SpinningInterface();
        spinning.newInstance(null);
        SilverCraftingListener silver = new SilverCraftingListener();
        silver.defineListeners();
        silver.defineInterfaceListeners();
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
        W.player.attributes.clear();
        Object spun = W.state.get("bowstrings");
        if (spun != null) W.player.attributes.put("diary:seers:bowstrings-spun", ((Number) spun).intValue());
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
        if (e instanceof NumberFormatException) return "Number";
        return e.getClass().getName();
    }

    private static int number(JSONObject op, String key) { return ((Number) op.get(key)).intValue(); }

    private static boolean has(int[] ids, int id) {
        for (int i : ids) if (i == id) return true;
        return false;
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
                for (W.Option o : W.options)
                    all.add(W.list("option", o.id, o.key, o.handler.getClass().getName()));
                for (W.Plugin p : W.plugins)
                    all.add(W.list("component", p.id, p.plugin.getClass().getName()));
                for (W.Registration r : W.registrations)
                    all.add(W.list("onUseWith", r.type, r.use.getClass().getName(), ints(r.used), ints(r.with)));
                for (W.Open o : W.opens)
                    all.add(W.list("onOpen", o.component, o.handler.getClass().getName()));
                for (W.Button b : W.buttons)
                    all.add(W.list("on", b.component, b.handler.getClass().getName()));
                return all;
            }
            case "tables": {
                JSONArray spins = new JSONArray(), silver = new JSONArray();
                for (SpinningItem s : SpinningItem.values())
                    spins.add(W.list(s.name(), s.getButton(), s.getNeed(), s.getProduct(), s.getLevel(), W.bits(s.getExp()),
                                     SpinningItem.forId(s.getButton()).name()));
                for (SilverProduct s : SilverProduct.values())
                    silver.add(W.list(s.name(), s.getButtonId(), s.getRequiredItemId(), s.getProducedItemId(), s.getAmountProduced(),
                                      s.getMinimumLevel(), W.bits(s.getXpReward()), SilverProduct.Companion.forButtonID(s.getButtonId()).name()));
                return W.list(spins, silver);
            }
            case "set": {
                JSONObject changes = (JSONObject) new JSONParser().parse(((JSONObject) op.get("state")).toJSONString());
                // The bowstring counter lives in the attribute store, so a change of it is written there.
                if (changes.containsKey("bowstrings")) {
                    Object spun = changes.remove("bowstrings");
                    if (spun == null) W.player.attributes.remove("diary:seers:bowstrings-spun");
                    else W.player.attributes.put("diary:seers:bowstrings-spun", ((Number) spun).intValue());
                }
                W.state.putAll(changes);
                return null;
            }
            case "wheel": {
                // SpinSpinningWheelPlugin.handle and getDestination with a scenery node of the id.
                Scenery node = new Scenery(number(op, "id"), 0, 0, 0);
                return W.list(wheel.handle(W.player, node, "spin"), wheel.getDestination(W.player, node) == null);
            }
            case "spinButton":
                return spinning.handle(W.player, new Component(459), number(op, "opcode"), number(op, "button"), 0, 0);
            case "use": {
                // A declared model of the use-with lookup: the pair as used only. The used node is an Item, the other a Scenery.
                int used = number(op, "used"), with = number(op, "with");
                for (W.Registration r : W.registrations)
                    if (has(r.used, used) && has(r.with, with))
                        return r.use.invoke(W.player, new Item(used), new Scenery(with, number(op, "x"), number(op, "y"), 0));
                return "unhandled";
            }
            case "open": {
                int component = number(op, "component");
                for (W.Open o : W.opens)
                    if (o.component == component)
                        return o.handler.invoke(W.player, new Component(component));
                return "unhandled";
            }
            case "button": {
                int component = number(op, "component");
                for (W.Button b : W.buttons)
                    if (b.component == component)
                        return b.handler.invoke(W.player, new Component(component), number(op, "opcode"), number(op, "button"), 0, 0);
                return "unhandled";
            }
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
