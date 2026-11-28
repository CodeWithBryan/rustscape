package oracle;

import core.game.dialogue.SkillDialogueHandler;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function3;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/**
 * The recording world of one case. Answers come from the case's declared
 * state; they are service inputs, not ports of the original subsystems.
 */
public final class W {
    public static final class Registration {
        public String type; public int[] used, with; public Function3<Player, Node, Node, Boolean> handler;
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
    public static final Map<Integer, String> names = new HashMap<>();
    public static JSONObject state;
    public static JSONArray trace;
    public static SkillDialogueHandler handler;
    /** Every script the case queued, in order. */
    public static final List<Function1<Integer, Boolean>> scripts = new ArrayList<>();

    @SuppressWarnings("unchecked")
    public static void emit(String name, Object... args) {
        JSONArray row = new JSONArray();
        row.add(name);
        for (Object a : args) row.add(a);
        trace.add(row);
    }
    public static void register(String type, int[] used, int[] with, Function3<Player, Node, Node, Boolean> handler) {
        Registration r = new Registration();
        r.type = type; r.used = used; r.with = with; r.handler = handler;
        registrations.add(r);
    }
    public static boolean flag(String key) { return Boolean.TRUE.equals(state.get(key)); }
    public static int level(int skill) { return ((Number) state.get("level")).intValue(); }
    public static int questStage(String quest) {
        Object v = ((JSONObject) state.get("quests")).get(quest);
        return v == null ? 0 : ((Number) v).intValue();
    }
    public static int amount(int id) {
        Object v = ((JSONObject) state.get("held")).get(Integer.toString(id));
        return v == null ? 0 : ((Number) v).intValue();
    }
    /** The inventory stand-in: a removal succeeds when at least the amount is held. */
    @SuppressWarnings("unchecked")
    public static boolean remove(int id, int amount) {
        int held = amount(id);
        boolean ok = held >= amount;
        if (ok) ((JSONObject) state.get("held")).put(Integer.toString(id), (long) (held - amount));
        emit("remove", id, amount, ok);
        return ok;
    }
    /** The inventory stand-in: an addition always succeeds. */
    @SuppressWarnings("unchecked")
    public static boolean add(int id, int amount) {
        ((JSONObject) state.get("held")).put(Integer.toString(id), (long) (amount(id) + amount));
        emit("add", id, amount);
        return true;
    }
    public static void xp(int skill, double amount) {
        emit("xp", skill, String.format("d:%016x", Double.doubleToRawLongBits(amount)));
    }
    private static String key(String attribute) {
        return attribute.startsWith("/save:") ? attribute.substring(6) : attribute;
    }
    /** The attribute stand-in: one map, the "/save:" prefix dropped from keys. */
    @SuppressWarnings("unchecked")
    public static <T> T attribute(String attribute, T fallback) {
        Object v = ((JSONObject) state.get("attributes")).get(key(attribute));
        return v == null ? fallback : (T) v;
    }
    @SuppressWarnings("unchecked")
    public static void setAttribute(String attribute, Object value) {
        ((JSONObject) state.get("attributes")).put(key(attribute), value);
        emit("setAttribute", attribute, value);
    }
    public static void queue(int delay, String strength, boolean persist, Function1<Integer, Boolean> body) {
        scripts.add(body);
        emit("queue", delay, strength, persist);
    }
    @SuppressWarnings("unchecked")
    public static void open(SkillDialogueHandler opened, String type, List<Integer> items) {
        handler = opened;
        JSONArray ids = new JSONArray();
        ids.addAll(items);
        emit("open", type, ids);
    }
    public static Location location() {
        JSONArray at = (JSONArray) state.get("at");
        return new Location(((Number) at.get(0)).intValue(), ((Number) at.get(1)).intValue(), ((Number) at.get(2)).intValue());
    }
    public static String name(int id) {
        JSONObject own = (JSONObject) state.get("names");
        String key = Integer.toString(id);
        if (own != null && own.containsKey(key)) return (String) own.get(key);
        return names.get(id);
    }
}
