package oracle;

import content.global.skill.crafting.gem.GemCutPulse;
import content.global.skill.crafting.gem.Gems;
import content.global.skill.crafting.limestone.ChiselLimestonePulse;
import core.game.dialogue.SkillDialogueHandler;
import core.game.interaction.UseWithHandler;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.SkillPulse;
import core.game.system.task.Pulse;
import java.util.ArrayList;
import java.util.List;
import kotlin.jvm.functions.Function3;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/**
 * The recording world of one case. Answers come from the case's declared
 * state; they are service inputs, not ports of the original subsystems.
 */
public final class W {
    public static final class Registration {
        public String type; public int[] used, with;
        public Function3<Player, Node, Node, Boolean> use;
    }
    public static final class Handler {
        public int id, type; public UseWithHandler handler;
    }

    public static final List<Registration> registrations = new ArrayList<>();
    public static final List<Handler> handlers = new ArrayList<>();
    public static JSONObject state;
    public static JSONArray trace;
    public static SkillDialogueHandler handler;
    /** The pulse the case's last PulseManager.run kept. */
    public static Pulse pulse;

    @SuppressWarnings("unchecked")
    public static void emit(String name, Object... args) {
        JSONArray row = new JSONArray();
        row.add(name);
        for (Object a : args) row.add(a);
        trace.add(row);
    }
    public static void registerUse(String type, int[] used, int[] with, Function3<Player, Node, Node, Boolean> handler) {
        Registration r = new Registration();
        r.type = type; r.used = used; r.with = with; r.use = handler;
        registrations.add(r);
    }
    public static void addHandler(int id, int type, UseWithHandler handler) {
        Handler h = new Handler();
        h.id = id; h.type = type; h.handler = handler;
        handlers.add(h);
    }
    public static String bits(double value) { return String.format("d:%016x", Double.doubleToRawLongBits(value)); }
    public static int number(String key) { return ((Number) state.get(key)).intValue(); }
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
    private static Object field(Class<?> owner, Object of, String name) throws ReflectiveOperationException {
        java.lang.reflect.Field f = owner.getDeclaredField(name);
        f.setAccessible(true);
        return f.get(of);
    }
    /** PulseManager.run: the pulse's fields as constructed (private ones read by reflection), then its start(). */
    public static void run(Pulse run) {
        try {
            if (run instanceof GemCutPulse) {
                emit("run", "gem", ((Gems) field(GemCutPulse.class, run, "gem")).name(), field(GemCutPulse.class, run, "amount"),
                     ((Node) field(SkillPulse.class, run, "node")).getId(), run.getDelay(), run.isRunning());
            } else {
                emit("run", "limestone", field(ChiselLimestonePulse.class, run, "ticks"),
                     ((Node) field(SkillPulse.class, run, "node")).getId(), run.getDelay(), run.isRunning());
            }
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
}
