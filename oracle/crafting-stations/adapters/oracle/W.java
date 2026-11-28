package oracle;

import content.global.skill.crafting.silver.SilverCraftingPulse;
import content.global.skill.crafting.silver.SilverProduct;
import content.global.skill.crafting.spinning.SpinningItem;
import content.global.skill.crafting.spinning.SpinningPulse;
import core.game.component.Component;
import core.game.component.ComponentPlugin;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.SkillPulse;
import core.game.node.scenery.Scenery;
import core.game.system.task.Pulse;
import java.util.ArrayList;
import java.util.List;
import kotlin.Unit;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function3;
import kotlin.jvm.functions.Function6;
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
    public static final class Button {
        public int component;
        public Function6<Player, Component, Integer, Integer, Integer, Integer, Boolean> handler;
    }
    public static final class Open {
        public int component;
        public Function2<Player, Component, Boolean> handler;
    }
    public static final class Option {
        public int id; public String key; public Object handler;
    }
    public static final class Plugin {
        public int id; public ComponentPlugin plugin;
    }

    public static final Player player = new Player();
    public static final List<Registration> registrations = new ArrayList<>();
    public static final List<Button> buttons = new ArrayList<>();
    public static final List<Open> opens = new ArrayList<>();
    public static final List<Option> options = new ArrayList<>();
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
    public static void registerUse(String type, int[] used, int[] with, Function3<Player, Node, Node, Boolean> handler) {
        Registration r = new Registration();
        r.type = type; r.used = used; r.with = with; r.use = handler;
        registrations.add(r);
    }
    public static void registerButton(int component, Function6<Player, Component, Integer, Integer, Integer, Integer, Boolean> handler) {
        Button b = new Button();
        b.component = component; b.handler = handler;
        buttons.add(b);
    }
    public static void registerOpen(int component, Function2<Player, Component, Boolean> handler) {
        Open o = new Open();
        o.component = component; o.handler = handler;
        opens.add(o);
    }
    public static void option(int id, String key, Object handler) {
        Option o = new Option();
        o.id = id; o.key = key; o.handler = handler;
        options.add(o);
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
            if (run instanceof SpinningPulse) {
                emit("run", "spin", ((SpinningItem) field(SpinningPulse.class, run, "type")).name(), field(SpinningPulse.class, run, "ammount"),
                     field(SpinningPulse.class, run, "ticks"), ((Node) field(SkillPulse.class, run, "node")).getId(),
                     ((core.game.node.item.Item) field(SkillPulse.class, run, "node")).getAmount(), run.getDelay(), run.isRunning());
            } else if (run instanceof SilverCraftingPulse) {
                emit("run", "silver", ((SilverProduct) field(SilverCraftingPulse.class, run, "product")).name(),
                     ((Scenery) field(SilverCraftingPulse.class, run, "furnace")).shown(), field(SilverCraftingPulse.class, run, "amount"),
                     field(SilverCraftingPulse.class, run, "player") == player, run.getDelay(), run.isRunning());
            } else {
                emit("run", run.getClass().getName());
            }
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
        run.start();
        emit("started", run.isRunning());
        pulse = run;
    }
}
