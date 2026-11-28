package oracle;

import core.game.dialogue.SkillDialogueHandler;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.system.task.Pulse;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function3;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/**
 * The recording world of one case. Answers come from the case's declared
 * state; they are service inputs, not ports of the original subsystems.
 */
public final class W {
    public static final class Registration {
        public String type; public String[] options; public int[] used, with;
        public Function2<Player, Node, Boolean> option; public Function3<Player, Node, Node, Boolean> use;
    }

    public static final List<Registration> registrations = new ArrayList<>();
    public static final Map<Integer, String> names = new HashMap<>();
    public static JSONObject state;
    public static JSONArray trace;
    public static SkillDialogueHandler handler;
    /** The pulse the case's last PulseManager.run kept. */
    public static Pulse pulse;
    /** The draws RANDOM.nextInt answers with, in order. */
    public static int drawn;

    @SuppressWarnings("unchecked")
    public static void emit(String name, Object... args) {
        JSONArray row = new JSONArray();
        row.add(name);
        for (Object a : args) row.add(a);
        trace.add(row);
    }
    public static void registerOption(String type, String[] options, Function2<Player, Node, Boolean> handler) {
        Registration r = new Registration();
        r.type = type; r.options = options; r.option = handler;
        registrations.add(r);
    }
    public static void registerUse(String type, int[] used, int[] with, Function3<Player, Node, Node, Boolean> handler) {
        Registration r = new Registration();
        r.type = type; r.used = used; r.with = with; r.use = handler;
        registrations.add(r);
    }
    /** RANDOM.nextInt(max): the next declared draw, reduced into the bound. */
    public static int draw(int max) {
        JSONArray draws = (JSONArray) state.get("draws");
        int value = (int) (((Number) draws.get(drawn++ % draws.size())).longValue() % max);
        emit("random", max, value);
        return value;
    }
    public static String bits(double value) { return String.format("d:%016x", Double.doubleToRawLongBits(value)); }
    public static int number(String key) { return ((Number) state.get(key)).intValue(); }
    public static boolean listed(String key, String value) { return ((JSONArray) state.get(key)).contains(value); }
    public static int questStage(String quest) {
        emit("questStage", quest);
        Object v = ((JSONObject) state.get("quests")).get(quest);
        return v == null ? 0 : ((Number) v).intValue();
    }
    public static int amount(int id) {
        Object v = ((JSONObject) state.get("held")).get(Integer.toString(id));
        return v == null ? 0 : ((Number) v).intValue();
    }
    @SuppressWarnings("unchecked")
    public static boolean take(int id, int amount) {
        int held = amount(id);
        if (held < amount) return false;
        ((JSONObject) state.get("held")).put(Integer.toString(id), (long) (held - amount));
        return true;
    }
    @SuppressWarnings("unchecked")
    public static void give(int id, int amount) {
        ((JSONObject) state.get("held")).put(Integer.toString(id), (long) (amount(id) + amount));
    }
    public static int[] consumable(int id) {
        JSONArray ids = (JSONArray) ((JSONObject) state.get("consumables")).get(Integer.toString(id));
        emit("consumable", id, ids == null ? null : ids.get(0));
        if (ids == null) return null;
        int[] out = new int[ids.size()];
        for (int i = 0; i < out.length; i++) out[i] = ((Number) ids.get(i)).intValue();
        return out;
    }
    /** PulseManager.run: the pulse's potion and amount (private fields read by reflection), then its start(). */
    public static void run(Pulse run) {
        try {
            java.lang.reflect.Field potion = run.getClass().getDeclaredField("potion"), amount = run.getClass().getDeclaredField("amount");
            potion.setAccessible(true); amount.setAccessible(true);
            content.global.skill.herblore.GenericPotion p = (content.global.skill.herblore.GenericPotion) potion.get(run);
            emit("run", p.getBase().getId(), p.getIngredient().getId(), p.getLevel(), bits(p.getExperience()), p.getProduct().getId(), amount.get(run));
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
        run.start();
        emit("started", run.isRunning());
        pulse = run;
    }
    @SuppressWarnings("unchecked")
    public static void open(SkillDialogueHandler opened, String type, List<Integer> items) {
        handler = opened;
        JSONArray ids = new JSONArray();
        ids.addAll(items);
        emit("open", type, ids);
    }
    public static String name(int id) {
        JSONObject own = (JSONObject) state.get("names");
        String key = Integer.toString(id);
        if (own != null && own.containsKey(key)) return (String) own.get(key);
        return names.get(id);
    }
}
