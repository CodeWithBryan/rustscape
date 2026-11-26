package oracle;

import core.api.utils.CameraShakeType;
import core.api.utils.PlayerCamera;
import core.api.utils.WeightedTable;
import core.game.node.entity.player.Player;
import core.net.packet.context.CameraContext;
import core.tools.RandomFunction;
import kotlin.Pair;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

import java.lang.reflect.Field;
import java.util.*;

/**
 * Runs one api-utils case (Boot gives every case a class loader of its own): the unchanged WeightedTable,
 * PlayerCamera, CameraUtils (CameraShakeType) and CameraContext. The two generators WeightedTable.roll draws from
 * (the JDK's shared Collections.shuffle generator and RandomFunction.RANDOM) are seeded by the case and record
 * their draws. {@code failAt} makes the n-th recorded call throw after it is recorded.
 */
public final class Support {
    public static final class Failure extends IllegalStateException {
        public Failure() { super("injected"); }
    }

    /** A java.util.Random that records the draws the originals make. */
    public static final class Recording extends Random {
        final String name;
        boolean ready;
        public Recording(String name, long seed) { super(seed); this.name = name; ready = true; }
        @Override public int nextInt(int bound) { if (ready) ev(name + ".nextInt", bound); return super.nextInt(bound); }
        @Override public double nextDouble() { if (ready) ev(name + ".nextDouble"); return super.nextDouble(); }
    }

    public static List<Object> events = new ArrayList<>();
    static int calls, failAt;
    static WeightedTable<Object> table = CallsKt.tableCreate(new ArrayList<>());
    static PlayerCamera camera;
    static final Player player = new Player();

    static int num(Object o) { return ((Number) o).intValue(); }

    static double weight(Object o) {
        return o instanceof String ? Double.parseDouble((String) o) : ((Number) o).doubleValue();
    }

    /** A double as its bits (NaN by name): JSON has no NaN or infinity. */
    static String bits(double d) {
        return Double.isNaN(d) ? "NaN" : String.format("%016x", Double.doubleToRawLongBits(d));
    }

    /** A table element as the case gives it: text, an int, or null. */
    static Object element(Object o) {
        return o instanceof Long ? (Object) ((Long) o).intValue() : o;
    }

    public static void ev(String name, Object... args) {
        List<Object> row = new ArrayList<>();
        row.add(name);
        row.addAll(Arrays.asList(args));
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    /** {@code PacketRepository.send(CameraViewPacket.class, context)}. */
    public static void sent(Class<?> packet, Object context) {
        CameraContext c = (CameraContext) context;
        ev("PacketRepository.send", packet.getSimpleName(), c.getPlayer() == player ? "player" : null, c.getType().name(), c.getType().opcode(),
            c.getX(), c.getY(), c.getHeight(), c.getSpeed(), c.getZoomSpeed());
    }

    static Object context(CameraContext c) {
        if (c == null) return null;
        return Arrays.asList(c.getType().name(), c.getX(), c.getY(), c.getHeight(), c.getSpeed(), c.getZoomSpeed());
    }

    @SuppressWarnings("unchecked")
    static Object op(JSONArray o) {
        String name = (String) o.get(0);
        switch (name) {
            case "add": return CallsKt.tableAdd(table, element(o.get(1)), weight(o.get(2)));
            case "addRaw": return CallsKt.tableAddRaw(table, element(o.get(1)), weight(o.get(2)));
            case "remove": return CallsKt.tableRemove(table, element(o.get(1)));
            case "removeAt": { Pair<Object, Double> p = CallsKt.tableRemoveAt(table, num(o.get(1))); return Arrays.asList(p.getFirst(), bits(p.getSecond())); }
            case "clear": CallsKt.tableClear(table); return null;
            case "roll": return CallsKt.tableRoll(table);
            case "create": {
                List<Pair<Object, Double>> pairs = new ArrayList<>();
                for (Object e : (JSONArray) o.get(1)) pairs.add(new Pair<>(element(((JSONArray) e).get(0)), weight(((JSONArray) e).get(1))));
                table = CallsKt.tableCreate(pairs);
                return null;
            }
            case "total": table.setTotalWeight(weight(o.get(1))); return null;
            case "camera": camera = new PlayerCamera(Boolean.TRUE.equals(o.get(1)) ? player : null); return null;
            case "setPosition": camera.setPosition(num(o.get(1)), num(o.get(2)), num(o.get(3))); return null;
            case "rotateTo": camera.rotateTo(num(o.get(1)), num(o.get(2)), num(o.get(3)), num(o.get(4))); return null;
            case "rotateBy": camera.rotateBy(num(o.get(1)), num(o.get(2)), num(o.get(3)), num(o.get(4))); return null;
            case "panTo": camera.panTo(num(o.get(1)), num(o.get(2)), num(o.get(3)), num(o.get(4))); return null;
            case "shake": camera.shake(num(o.get(1)), num(o.get(2)), num(o.get(3)), num(o.get(4)), num(o.get(5))); return null;
            case "reset": camera.reset(); return null;
            case "forget": camera.setCtx(null); return null;
            case "shakeTypes": {
                List<Object> out = new ArrayList<>();
                for (CameraShakeType t : CameraShakeType.values()) out.add(Arrays.asList(t.name(), t.ordinal()));
                return out;
            }
            case "cameraTypes": {
                List<Object> out = new ArrayList<>();
                for (CameraContext.CameraType t : CameraContext.CameraType.values()) out.add(Arrays.asList(t.name(), t.opcode()));
                return out;
            }
            default: throw new IllegalArgumentException(name);
        }
    }

    static String error(Throwable t) {
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        if (t instanceof IllegalStateException) return "Binding";
        return t.getClass().getName() + ": " + t.getMessage();
    }

    @SuppressWarnings("unchecked")
    public static String runCase(String line) throws Exception {
        JSONObject c = (JSONObject) new JSONParser().parse(line);
        failAt = c.containsKey("failAt") ? num(c.get("failAt")) : -1;
        Field shared = Collections.class.getDeclaredField("r");
        shared.setAccessible(true);
        shared.set(null, new Recording("Collections.shuffle", ((Number) c.get("shuffleSeed")).longValue()));
        RandomFunction.oracleSeed(((Number) c.get("randomSeed")).longValue());
        List<Object> steps = new ArrayList<>();
        for (Object o : (JSONArray) c.get("ops")) {
            Map<String, Object> step = new TreeMap<>();
            int before = events.size();
            try {
                step.put("result", op((JSONArray) o));
            } catch (Failure | NullPointerException | IndexOutOfBoundsException f) {
                step.put("error", error(f));
            }
            step.put("events", new ArrayList<>(events.subList(before, events.size())));
            List<Object> entries = new ArrayList<>();
            for (Pair<Object, Double> p : CallsKt.tableEntries(table)) entries.add(Arrays.asList(p.getFirst(), bits(p.getSecond())));
            step.put("table", entries);
            step.put("total", bits(table.getTotalWeight()));
            step.put("camera", camera == null ? null : context(camera.getCtx()));
            steps.add(step);
        }
        JSONObject out = new JSONObject();
        out.putAll(c);
        Map<String, Object> expected = new TreeMap<>();
        expected.put("steps", steps);
        expected.put("calls", calls);
        out.put("expected", expected);
        return out.toJSONString();
    }
}
