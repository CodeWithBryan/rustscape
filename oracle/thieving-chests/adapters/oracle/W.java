package oracle;

import core.game.interaction.OptionHandler;
import core.game.system.task.Pulse;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/**
 * The recording world of one case. Answers come from the case's declared
 * state; they are service inputs, not ports of the original subsystems.
 */
public final class W {
    public static final JSONArray registrations = new JSONArray();
    public static OptionHandler handler;
    public static JSONObject state;
    public static JSONArray trace;
    /** The pulse the pulser was last given. */
    public static Pulse pulse;

    @SuppressWarnings("unchecked")
    public static void emit(String name, Object... args) {
        JSONArray row = new JSONArray();
        row.add(name);
        for (Object a : args) row.add(a);
        trace.add(row);
    }
    @SuppressWarnings("unchecked")
    public static void register(int id, String key, OptionHandler plugin) {
        JSONArray row = new JSONArray();
        row.add(id); row.add(key);
        registrations.add(row);
        handler = plugin;
    }
    public static String bits(double value) { return String.format("d:%016x", Double.doubleToRawLongBits(value)); }
    public static boolean flag(String key) { return Boolean.TRUE.equals(state.get(key)); }
    public static int number(String key) { return ((Number) state.get(key)).intValue(); }
}
