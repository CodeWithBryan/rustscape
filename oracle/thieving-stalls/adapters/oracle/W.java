package oracle;

import core.game.interaction.OptionHandler;
import core.game.system.task.Pulse;
import java.util.ArrayList;
import java.util.List;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/**
 * The recording world of one case. Answers come from the case's declared
 * state; they are service inputs, not ports of the original subsystems.
 */
public final class W {
    /** RandomFunction.RANDOM: each draw is the case's next declared one. */
    public static final class Draws extends java.util.Random {
        @Override public int nextInt(int bound) {
            JSONArray draws = (JSONArray) state.get("draws");
            int v = ((Number) draws.remove(0)).intValue();
            emit("random", bound, v);
            return v;
        }
    }

    public static final List<String> options = new ArrayList<>();
    public static OptionHandler handler;
    public static JSONObject state;
    public static JSONArray trace;
    /** The pulse the player's pulse manager was last given. */
    public static Pulse pulse;

    @SuppressWarnings("unchecked")
    public static void emit(String name, Object... args) {
        JSONArray row = new JSONArray();
        row.add(name);
        for (Object a : args) row.add(a);
        trace.add(row);
    }
    public static void register(String name, OptionHandler plugin) {
        options.add(name);
        handler = plugin;
    }
    public static String bits(double value) { return String.format("d:%016x", Double.doubleToRawLongBits(value)); }
    public static boolean flag(String key) { return Boolean.TRUE.equals(state.get(key)); }
    public static int number(String key) { return ((Number) state.get(key)).intValue(); }
    /** An item's declared name, or the one the case declares for every item ("*"); otherwise "Item" and its id. */
    public static String itemName(int id) {
        JSONObject names = (JSONObject) state.get("names");
        Object name = names.get(Integer.toString(id));
        if (name == null) name = names.get("*");
        return name == null ? "Item " + id : (String) name;
    }
}
