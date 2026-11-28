package oracle;

import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import java.util.ArrayList;
import java.util.List;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/**
 * The recording world of one case. Answers come from the case's declared
 * state; they are service inputs, not ports of the original subsystems.
 */
public final class W {
    public static final class Registration {
        public String type; public String[] options; public Function2<Player, Node, Boolean> handler;
    }
    /** RandomFunction.RANDOM: each draw is the case's next declared one. */
    public static final class Draws extends java.util.Random {
        @Override public int nextInt(int bound) {
            JSONArray draws = (JSONArray) state.get("draws");
            int v = ((Number) draws.remove(0)).intValue();
            emit("random", bound, v);
            return v;
        }
    }

    public static final List<Registration> registrations = new ArrayList<>();
    public static JSONObject state;
    public static JSONArray trace;

    @SuppressWarnings("unchecked")
    public static void emit(String name, Object... args) {
        JSONArray row = new JSONArray();
        row.add(name);
        for (Object a : args) {
            if (a instanceof List && !(a instanceof JSONArray)) {
                JSONArray copy = new JSONArray();
                copy.addAll((List<?>) a);
                row.add(copy);
            } else {
                row.add(a);
            }
        }
        trace.add(row);
    }
    public static void register(String type, String[] options, Function2<Player, Node, Boolean> handler) {
        Registration r = new Registration();
        r.type = type; r.options = options; r.handler = handler;
        registrations.add(r);
    }
    public static String bits(double value) { return String.format("d:%016x", Double.doubleToRawLongBits(value)); }
    public static boolean flag(String key) { return Boolean.TRUE.equals(state.get(key)); }
    public static int number(String key) { return ((Number) state.get(key)).intValue(); }
    /** RandomFunction.randomDouble(min, max): the declared chance. */
    public static double chance(double min, double max) {
        emit("randomDouble", bits(min), bits(max));
        return ((Number) state.get("chance")).doubleValue();
    }
    /** A roll's items: the declared loot. */
    public static ArrayList<Item> loot() {
        ArrayList<Item> items = new ArrayList<>();
        for (Object o : (JSONArray) state.get("loot")) {
            JSONArray pair = (JSONArray) o;
            items.add(new Item(((Number) pair.get(0)).intValue(), ((Number) pair.get(1)).intValue()));
        }
        return items;
    }
}
