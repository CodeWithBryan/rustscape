package oracle;

import java.io.*;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.game.node.entity.player.Player;
import core.game.node.scenery.Scenery;
import core.game.world.map.Location;
import core.game.interaction.*;
import core.net.packet.in.Packet;
import core.net.packet.PacketProcessor;
import content.global.handlers.scenery.DoorManagingPlugin;

public class Support {
    static Map input;
    static Player player;
    public static Map<String, Scenery> objects;
    public static Map<String, InteractPlugin> interactions;
    public static String lookup;
    public static Integer varbit;
    public static boolean listeners, plugin;
    public static boolean permitted;
    public static Integer properties;
    public static Properties[] props;
    public static Location destination;
    static Map<String, Integer> registered;
    static DoorManagingPlugin doorHandler;
    static List<Object> events;
    static Map<String, Integer> calls;
    static int created;
    public static class Failure extends RuntimeException {}
    static int number(Object v) { return ((Number) v).intValue(); }
    static int value(Map m, String key, int d) { return m.get(key) == null ? d : number(m.get(key)); }
    static Location location(Object v) {
        if (v == null) return null;
        List a = (List) v;
        return new Location(number(a.get(0)), number(a.get(1)), number(a.get(2)));
    }
    public static Object coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    public static class Properties {
        public int key; public Location teleport;
        Properties(int k) { key = k; }
        public void setTeleportLocation(Location l) { teleport = l; emit("teleport", key, coord(l)); }
    }
    public static void register(String option, Object handler) { emit("register", option, 1); registered.put(option, 1); }
    public static void emit(String name, Object... args) {
        List<Object> row = new ArrayList<>(); row.add(name); row.addAll(Arrays.asList(args)); events.add(row);
        int count = calls.merge(name, 1, Integer::sum);
        for (Object h : (List) input.getOrDefault("hooks", Collections.emptyList())) {
            Map hook = (Map) h;
            if (name.equals(hook.get("event")) && count == value(hook, "at", 1))
                for (Object action : (List) hook.get("actions")) act((Map) action);
        }
    }
    static void options(String key, List values, boolean initial) {
        InteractPlugin p = interactions.get(key);
        if (!initial) p.generation++;
        p.options = new Option[values.size()];
        for (int i = 0; i < values.size(); i++) {
            if (values.get(i) == null) continue;
            Map v = (Map) values.get(i);
            p.options[i] = new Option((String) v.get("name"), value(v, "index", i), key + ":" + p.generation + ":" + i);
        }
    }
    static void add(String key, int id, Location l) {
        Scenery s = new Scenery(); s.key = key; s.id = id; s.location = l; s.child = s.interactionKey = key;
        objects.put(key, s);
        InteractPlugin p = new InteractPlugin(); p.key = key; interactions.put(key, p);
        Map<String, Object> open = new TreeMap<>(); open.put("name", "Open");
        options(key, Arrays.asList(open, null, null, null, null), true);
    }
    public static void construct(Scenery s, int id, Location l) {
        String key = "created" + created++;
        add(key, id, l);
        Scenery initial = objects.get(key);
        s.key = key; s.id = id; s.location = l; s.child = s.interactionKey = key; s.definition = initial.definition;
        objects.put(key, s);
        emit("construct", key, id, coord(l));
    }
    static void set(Map m, boolean initial) {
        if (m.containsKey("location")) player.location = location(m.get("location"));
        if (m.containsKey("lookup")) lookup = (String) m.get("lookup");
        if (m.containsKey("varbit")) varbit = m.get("varbit") == null ? null : number(m.get("varbit"));
        if (m.containsKey("listeners")) listeners = Boolean.TRUE.equals(m.get("listeners"));
        if (m.containsKey("plugin")) plugin = Boolean.TRUE.equals(m.get("plugin"));
        if (m.containsKey("permitted")) permitted = Boolean.TRUE.equals(m.get("permitted"));
        if (m.containsKey("properties")) properties = m.get("properties") == null ? null : number(m.get("properties"));
        Map values = (Map) m.getOrDefault("objects", Collections.emptyMap());
        for (Object k : values.keySet()) {
            Scenery s = objects.get(k); Map v = (Map) values.get(k);
            if (v.containsKey("id")) s.id = number(v.get("id"));
            if (v.containsKey("active")) s.active = Boolean.TRUE.equals(v.get("active"));
            if (v.containsKey("name")) s.name = (String) v.get("name");
            if (v.containsKey("shape")) s.shape = number(v.get("shape"));
            if (v.containsKey("location")) s.location = location(v.get("location"));
            if (v.containsKey("direction")) s.direction = Scenery.Direction.values()[number(v.get("direction"))];
            if (v.containsKey("child")) s.child = (String) v.get("child");
            if (v.containsKey("interaction")) s.interactionKey = (String) v.get("interaction");
            if (v.containsKey("definition")) s.definition = null;
            if (v.containsKey("configFile")) s.definition.configFileId = number(v.get("configFile"));
            if (v.containsKey("configId")) s.definition.configId = number(v.get("configId"));
        }
        values = (Map) m.getOrDefault("options", Collections.emptyMap());
        for (Object k : values.keySet()) options((String) k, (List) values.get(k), initial);
    }
    static Object act(Map a) {
        switch ((String) a.get("op")) {
            case "fail": throw new Failure();
            case "set": set(a, false); return null;
            case "register":
                try { return doorHandler.newInstance(null) == doorHandler ? 1 : 0; }
                catch (RuntimeException e) { throw e; }
                catch (Throwable e) { throw new RuntimeException(e); }
            case "doorHandle": case "doorDestination": {
                Player p = input.containsKey("player") && input.get("player") == null ? null : player;
                String key = (String) a.getOrDefault("object", "base");
                Scenery s = key == null ? null : objects.get(key);
                if (a.get("op").equals("doorHandle")) return doorHandler.handle(p, s, (String) a.get("option"));
                return coord(doorHandler.getDestination(p, s));
            }
            case "process":
                Packet.SceneryAction pkt = new Packet.SceneryAction();
                pkt.player = input.containsKey("player") && input.get("player") == null ? null : player;
                pkt.id = value(a, "id", value(input, "id", 12348));
                pkt.x = value(a, "x", 3207); pkt.y = value(a, "y", 3217);
                pkt.optIndex = value(a, "index", value(input, "index", 0));
                PacketProcessor.handle(pkt); return null;
            default: throw new IllegalArgumentException();
        }
    }
    static Object snapshot(Object value, String error) {
        Map<String, Object> result = new TreeMap<>(), os = new TreeMap<>(), is = new TreeMap<>();
        for (Scenery s : objects.values()) {
            List<Object> v = new ArrayList<>(Arrays.asList(s.id, s.active, coord(s.location), s.direction.ordinal(), s.child, s.interactionKey));
            v.add(s.definition == null ? null : Arrays.asList(s.definition.configFileId, s.definition.configId)); os.put(s.key, v);
            v.add(s.shape); v.add(s.name);
        }
        for (InteractPlugin p : interactions.values()) {
            List<Object> v = new ArrayList<>();
            for (Option o : p.options) v.add(o == null ? null : Arrays.asList(o.key, o.name, o.index));
            is.put(p.key, Arrays.asList(p.generation, v));
        }
        result.put("objects", os); result.put("interactions", is); result.put("events", new ArrayList<>(events));
        result.put("location", coord(player.location)); result.put("lookup", lookup); result.put("varbit", varbit);
        result.put("listeners", listeners); result.put("plugin", plugin); result.put("error", error);
        result.put("result", value); result.put("registered", new TreeMap<>(registered));
        result.put("permitted", permitted); result.put("properties", properties);
        result.put("teleports", Arrays.asList(coord(props[0].teleport), coord(props[1].teleport)));
        return result;
    }
    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.ROOT);
        try (BufferedReader r = new BufferedReader(new InputStreamReader(System.in, java.nio.charset.StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                input = (Map) new JSONParser().parse(line); Map initial = new TreeMap(input);
                objects = new TreeMap<>(); interactions = new TreeMap<>(); events = new ArrayList<>(); calls = new HashMap<>();
                created = 0; lookup = "base"; varbit = 7; listeners = plugin = false;
                permitted = true; properties = 0; props = new Properties[]{new Properties(0), new Properties(1)};
                destination = location(Arrays.asList(3208L, 3217L, 0L)); registered = new TreeMap<>(); doorHandler = new DoorManagingPlugin();
                player = new Player(); player.location = location(Arrays.asList(3207L, 3216L, 0L));
                add("base", 12348, location(Arrays.asList(3207L, 3217L, 0L)));
                add("other", 10327, location(Arrays.asList(3207L, 3217L, 0L)));
                set(input, true);
                List<Object> states = new ArrayList<>();
                for (Object action : (List) input.get("actions")) {
                    events.clear(); String error = null; Object value = null;
                    try { value = act((Map) action); }
                    catch (Failure e) { error = "Binding"; }
                    catch (NullPointerException e) { error = "Null"; }
                    catch (IndexOutOfBoundsException e) { error = "Index"; }
                    states.add(snapshot(value, error));
                }
                initial.put("expected", states); System.out.println(JSONValue.toJSONString(initial));
            }
        }
    }
}
