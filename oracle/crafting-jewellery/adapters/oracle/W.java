package oracle;

import content.global.skill.crafting.jewellery.JewelleryCrafting.JewelleryItem;
import content.global.skill.crafting.jewellery.JewelleryPulse;
import core.game.component.ComponentPlugin;
import core.game.interaction.UseWithHandler;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.SkillPulse;
import core.game.system.task.Pulse;
import java.util.ArrayList;
import java.util.List;
import kotlin.Unit;
import kotlin.jvm.functions.Function1;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/**
 * The recording world of one case. Answers come from the case's declared
 * state; they are service inputs, not ports of the original subsystems.
 */
public final class W {
    public static final class Handler {
        public int id, type; public UseWithHandler handler;
    }
    public static final class Plugin {
        public int id; public ComponentPlugin plugin;
    }

    public static final Player player = new Player();
    public static final List<Handler> handlers = new ArrayList<>();
    public static final List<Plugin> plugins = new ArrayList<>();
    public static JSONObject state;
    public static JSONArray trace;
    /** The handler of the case's last amount prompt. */
    public static Function1<Object, Unit> input;
    /** The pulse the case's last PulseManager.run kept. */
    public static Pulse pulse;

    @SuppressWarnings("unchecked")
    public static void emit(String name, Object... args) {
        JSONArray row = new JSONArray();
        row.add(name);
        for (Object a : args) row.add(a);
        trace.add(row);
    }
    @SuppressWarnings("unchecked")
    public static JSONArray list(Object... values) {
        JSONArray row = new JSONArray();
        for (Object v : values) row.add(v);
        return row;
    }
    public static void addHandler(int id, int type, UseWithHandler handler) {
        Handler h = new Handler();
        h.id = id; h.type = type; h.handler = handler;
        handlers.add(h);
    }
    public static void component(int id, ComponentPlugin plugin) {
        Plugin p = new Plugin();
        p.id = id; p.plugin = plugin;
        plugins.add(p);
    }
    public static String bits(double value) { return String.format("d:%016x", Double.doubleToRawLongBits(value)); }
    public static int number(String key) { return ((Number) state.get(key)).intValue(); }
    public static boolean flag(String key) { return Boolean.TRUE.equals(state.get(key)); }
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
    /**
     * PulseManager.run: the pulse's fields as constructed (private ones read by reflection), then its start(), as
     * the original manager calls it; the pulse is kept whether or not it is running.
     */
    public static void run(Pulse run) {
        try {
            emit("run", ((JewelleryItem) field(JewelleryPulse.class, run, "type")).name(), field(JewelleryPulse.class, run, "amount"),
                 field(JewelleryPulse.class, run, "ticks"), field(SkillPulse.class, run, "node") == null, run.getDelay(), run.isRunning());
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
        run.start();
        emit("started", run.isRunning());
        pulse = run;
    }
}
