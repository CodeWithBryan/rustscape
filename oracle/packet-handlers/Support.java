package oracle;

import content.global.handlers.item.equipment.brawling_gloves.BrawlingGlovesManager;
import core.cache.def.impl.ItemDefinition;
import core.cache.def.impl.NPCDefinition;
import core.cache.def.impl.SceneryDefinition;
import core.game.container.Container;
import core.game.container.impl.BankContainer;
import core.game.container.impl.EquipmentContainer;
import core.game.dialogue.DialogueAction;
import core.game.global.action.DropListener;
import core.game.global.action.EquipHandler;
import core.game.interaction.InteractPlugin;
import core.game.interaction.Option;
import core.game.interaction.OptionHandler;
import core.game.interaction.UseWithHandler;
import core.game.interaction.NodeUsageEvent;
import core.game.node.Node;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.GroundItem;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import core.net.packet.PacketProcessor;
import core.net.packet.in.Packet;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.*;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the original bodies and prints what they did. */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static List<Object> events;
    static Map<String, Integer> calls;
    static int opCalls, failAt;

    public static Map<String, Node> nodes;
    public static Map<String, InteractPlugin> interactions;
    public static Player player;
    public static Map<String, Object> flags, values, answers, attributes;
    static Map<String, Integer> answerCalls;
    public static Map<String, Container> containers;
    public static Map<Integer, String> repoPlayers, repoNpcs;
    public static int playerLength, npcLength;
    public static String object, objectAt, ground;
    public static Map<Integer, ItemDefinition> itemDefs;
    public static Map<Integer, SceneryDefinition> sceneryDefs;
    public static Map<Integer, NPCDefinition> npcDefs;
    public static Set<String> missingDefs;
    public static List<Pulse> pulses;
    public static List<Function1> scripts;
    public static List<DialogueAction> actions;
    public static List<Function2> listeners;
    public static Map<String, UseRec> useHandlers;

    static int n(Object o) { return ((Number) o).intValue(); }

    public static void emit(String name, Object... args) {
        List<Object> row = new ArrayList<>();
        row.add(name);
        for (Object a : args) row.add(a);
        events.add(row);
        opCalls++;
        int count = calls.merge(name, 1, Integer::sum);
        for (Object h : (List) input.getOrDefault("hooks", Collections.emptyList())) {
            Map hook = (Map) h;
            if (name.equals(hook.get("event")) && count == n(hook.getOrDefault("at", 1L)))
                for (Object action : (List) hook.get("actions")) {
                    Map a = (Map) action;
                    if ("fail".equals(a.get("op"))) throw new Failure();
                    set(a);
                }
        }
        if (opCalls == failAt) throw new Failure();
    }

    // Keys and values as the events show them.
    public static Object key(Object node) {
        if (node == null) return null;
        if (node instanceof Node) return ((Node) node).key;
        if (node instanceof Option) return ((Option) node).key;
        if (node instanceof InteractPlugin) return ((InteractPlugin) node).key;
        throw new IllegalStateException("key of " + node.getClass());
    }
    public static List coord(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    static Location location(Object v) {
        if (v == null) return null;
        List a = (List) v;
        return Location.create(n(a.get(0)), n(a.get(1)), n(a.get(2)));
    }

    public static boolean flag(String k) { return Boolean.TRUE.equals(flags.get(k)); }
    public static int intValue(String k, int d) { Object v = values.get(k); return v == null ? d : n(v); }
    public static String text(String k, String d) { return values.containsKey(k) ? (String) values.get(k) : d; }
    public static int ticks() { return intValue("ticks", 0); }

    /** A service's answer: a value, or a list taken in order (the last repeats). */
    public static Object answer(String name, Object fallback) {
        if (!answers.containsKey(name)) return fallback;
        Object v = answers.get(name);
        if (v instanceof List) {
            List l = (List) v;
            int i = answerCalls.merge(name, 1, Integer::sum) - 1;
            v = l.get(Math.min(i, l.size() - 1));
        }
        return v instanceof Long ? (Object) ((Long) v).intValue() : v;
    }
    public static boolean answerBool(String name, boolean fallback) { return Boolean.TRUE.equals(answer(name, fallback)); }
    public static int answerInt(String name, int fallback) { return n(answer(name, fallback)); }

    public static Node node(Object k) { return k == null ? null : nodes.get((String) k); }
    public static ItemDefinition itemDef(int id) { return itemDefs.computeIfAbsent(id, i -> new ItemDefinition()); }
    public static SceneryDefinition sceneryDef(int id) { return sceneryDefs.computeIfAbsent(id, i -> new SceneryDefinition()); }
    public static NPCDefinition npcDef(int id) { return npcDefs.computeIfAbsent(id, i -> new NPCDefinition()); }

    static Object handlerValue(Object v) {
        if (v instanceof Long) return ((Long) v).intValue();
        if (v instanceof Map) return new RecPlugin((String) ((Map) v).get("plugin"));
        return v;
    }

    /** An equipment plugin: fireEvent answers from the case. */
    public static class RecPlugin implements core.plugin.Plugin<Object> {
        public final String label;
        RecPlugin(String label) { this.label = label; }
        public core.plugin.Plugin<Object> newInstance(Object arg) { return this; }
        public Object fireEvent(String identifier, Object... args) {
            emit("fireEvent", label, identifier, key(args[1]));
            return answer("fireEvent:" + label, null);
        }
    }

    /** Option handlers: their class's simple name is part of the debug text. */
    public static abstract class RecHandler extends OptionHandler {
        public final int id;
        RecHandler(int id) { this.id = id; }
        public boolean handle(Player p, Node node, String option) {
            emit("handlerHandle", id, key(node), option);
            return answerBool("handler:" + id, false);
        }
    }
    public static class FirstHandler extends RecHandler { FirstHandler() { super(1); } }
    public static class SecondHandler extends RecHandler { SecondHandler() { super(2); } }

    /** Registered use-with handlers. */
    public static abstract class UseRec extends UseWithHandler {
        public final String ref;
        public final boolean dynamic;
        UseRec(String ref, int[] allowed, boolean dynamic) { super(allowed); this.ref = ref; this.dynamic = dynamic; }
        public core.plugin.Plugin<Object> newInstance(Object arg) { return this; }
        public boolean handle(NodeUsageEvent event) {
            emit("useHandle", ref, key(event.getUsed()), key(event.getUsedWith()));
            return answerBool("useHandle:" + ref, false);
        }
        public boolean isDynamic() { return dynamic; }
    }
    public static class FirstUse extends UseRec { FirstUse(String r, int[] a, boolean d) { super(r, a, d); } }
    public static class SecondUse extends UseRec { SecondUse(String r, int[] a, boolean d) { super(r, a, d); } }

    static Option option(String interaction, int slot, Object spec) {
        if (spec == null) return null;
        Map m = (Map) spec;
        Option o = new Option((String) m.get("name"), m.get("index") == null ? slot : n(m.get("index")));
        o.key = m.get("key") == null ? interaction + "." + slot : (String) m.get("key");
        Object h = m.get("handler");
        if (h != null) o.setHandler(n(h) == 1 ? new FirstHandler() : new SecondHandler());
        return o;
    }

    static void node(String k, Map v) {
        Node node = nodes.get(k);
        if (node == null) {
            String kind = (String) v.get("kind");
            switch (kind) {
                case "item": node = new Item(); break;
                case "ground": node = new GroundItem(); break;
                case "npc": node = new NPC(); break;
                case "player": node = new Player(); break;
                case "scenery": node = new Scenery(); break;
                default: throw new IllegalArgumentException(kind);
            }
            node.key = k;
            node.text = kind + ":" + k;
            nodes.put(k, node);
            node.definitionId = v.containsKey("definition") ? n(v.get("definition")) : v.containsKey("id") ? n(v.get("id")) : 0;
        }
        if (v.containsKey("id")) node.nodeId = n(v.get("id"));
        if (v.containsKey("index")) node.nodeIndex = n(v.get("index"));
        if (v.containsKey("name")) node.nodeName = (String) v.get("name");
        if (v.containsKey("active")) node.nodeActive = Boolean.TRUE.equals(v.get("active"));
        if (v.containsKey("location")) node.nodeLocation = location(v.get("location"));
        if (v.containsKey("interaction")) node.interactionKey = (String) v.get("interaction");
        if (v.containsKey("text")) node.text = (String) v.get("text");
        if (v.containsKey("shown")) node.shown = (String) v.get("shown");
        if (v.containsKey("child")) node.child = (String) v.get("child");
        if (v.containsKey("spawn")) node.spawn = location(v.get("spawn"));
        if (v.containsKey("configFile")) node.configFile = n(v.get("configFile"));
        if (v.containsKey("amount")) ((Item) node).oracleAmount(n(v.get("amount")));
        if (v.containsKey("definition")) node.definitionId = n(v.get("definition"));
        if (v.containsKey("drop")) node.drop = (String) v.get("drop");
    }

    static void definitions(Map v) {
        for (Object k : ((Map) v.getOrDefault("itemDefs", Collections.emptyMap())).keySet()) {
            Map d = (Map) ((Map) v.get("itemDefs")).get(k);
            ItemDefinition def = itemDefs.computeIfAbsent(Integer.parseInt((String) k), i -> new ItemDefinition());
            if (d.containsKey("name")) def.setName((String) d.get("name"));
            if (d.containsKey("examine")) def.setExamine((String) d.get("examine"));
            if (d.containsKey("value")) def.setValue(n(d.get("value")));
            if (d.containsKey("model")) def.setInterfaceModelId(n(d.get("model")));
            if (d.containsKey("unnoted")) def.setUnnoted(Boolean.TRUE.equals(d.get("unnoted")));
            Map h = (Map) d.getOrDefault("handlers", Collections.emptyMap());
            for (Object hk : h.keySet()) def.getHandlers().put((String) hk, handlerValue(h.get(hk)));
        }
        for (Object k : ((Map) v.getOrDefault("sceneryDefs", Collections.emptyMap())).keySet()) {
            Map d = (Map) ((Map) v.get("sceneryDefs")).get(k);
            SceneryDefinition def = sceneryDefs.computeIfAbsent(Integer.parseInt((String) k), i -> new SceneryDefinition());
            if (d.containsKey("name")) def.setName((String) d.get("name"));
            if (d.containsKey("examine")) def.setExamine((String) d.get("examine"));
            if (d.containsKey("configFile")) def.setConfigFileId(n(d.get("configFile")));
            Map h = (Map) d.getOrDefault("handlers", Collections.emptyMap());
            for (Object hk : h.keySet()) def.getHandlers().put((String) hk, handlerValue(h.get(hk)));
        }
        for (Object k : ((Map) v.getOrDefault("npcDefs", Collections.emptyMap())).keySet()) {
            Map d = (Map) ((Map) v.get("npcDefs")).get(k);
            NPCDefinition def = npcDefs.computeIfAbsent(Integer.parseInt((String) k), i -> new NPCDefinition());
            if (d.containsKey("name")) def.setName((String) d.get("name"));
            if (d.containsKey("examine")) def.setExamine((String) d.get("examine"));
            if (d.containsKey("configFile")) def.setConfigFileId(n(d.get("configFile")));
            Map h = (Map) d.getOrDefault("handlers", Collections.emptyMap());
            for (Object hk : h.keySet()) def.getHandlers().put((String) hk, handlerValue(h.get(hk)));
        }
    }

    /** Applies a state change (the case itself, a "set" operation or a hook action). */
    static void set(Map v) {
        Map ns = (Map) v.getOrDefault("nodes", Collections.emptyMap());
        for (Object k : new TreeSet(ns.keySet())) node((String) k, (Map) ns.get(k));
        Map is = (Map) v.getOrDefault("interactions", Collections.emptyMap());
        for (Object k : new TreeSet(is.keySet())) {
            Map spec = (Map) is.get(k);
            InteractPlugin p = interactions.get(k);
            if (p == null) {
                p = new InteractPlugin(nodes.get((String) spec.get("owner")));
                p.key = (String) k;
                interactions.put((String) k, p);
            }
            List os = (List) spec.get("options");
            Option[] options = new Option[os.size()];
            for (int i = 0; i < options.length; i++) options[i] = option((String) k, i, os.get(i));
            p.setOptions(options);
        }
        Map cs = (Map) v.getOrDefault("containers", Collections.emptyMap());
        for (Object k : new TreeSet(cs.keySet())) {
            Map spec = (Map) cs.get(k);
            Container c = containers.get(k);
            if (spec.containsKey("capacity")) c.resize(n(spec.get("capacity")));
            Map slots = (Map) spec.getOrDefault("slots", Collections.emptyMap());
            for (Object s : slots.keySet()) c.items[Integer.parseInt((String) s)] = (Item) node(slots.get(s));
            if (spec.containsKey("tabStart")) {
                List t = (List) spec.get("tabStart");
                for (int i = 0; i < t.size(); i++) ((BankContainer) c).tabStartSlot[i] = n(t.get(i));
            }
        }
        Map rp = (Map) v.getOrDefault("players", Collections.emptyMap());
        for (Object k : rp.keySet()) repoPlayers.put(Integer.parseInt((String) k), (String) rp.get(k));
        Map rn = (Map) v.getOrDefault("npcs", Collections.emptyMap());
        for (Object k : rn.keySet()) repoNpcs.put(Integer.parseInt((String) k), (String) rn.get(k));
        if (v.containsKey("object")) object = (String) v.get("object");
        if (v.containsKey("objectAt")) objectAt = (String) v.get("objectAt");
        if (v.containsKey("ground")) ground = (String) v.get("ground");
        flags.putAll((Map) v.getOrDefault("flags", Collections.emptyMap()));
        values.putAll((Map) v.getOrDefault("values", Collections.emptyMap()));
        answers.putAll((Map) v.getOrDefault("answers", Collections.emptyMap()));
        Map attrs = (Map) v.getOrDefault("attributes", Collections.emptyMap());
        for (Object k : attrs.keySet()) {
            Object a = attrs.get(k);
            attributes.put((String) k, a instanceof Long ? (Object) ((Long) a).intValue() : a);
        }
        if (v.containsKey("missing")) for (Object m : (List) v.get("missing")) missingDefs.add((String) m);
        definitions(v);
        core.ServerConstants.MAX_PLAYERS = intValue("maxPlayers", 2000);
        core.ServerConstants.MAX_NPCS = intValue("maxNpcs", 32000);
    }

    // Callbacks from the adapters.
    public static void submit(Pulse p) { emit("submit", p.number); }
    public static void queue(String strength, Function1 script) { scripts.add(script); emit("queueScript", strength, scripts.size() - 1); }
    public static void addAction(DialogueAction a) { actions.add(a); emit("addDialogueAction", actions.size() - 1); }
    public static void on(String type, String[] options, Function2 handler) {
        listeners.add(handler);
        emit("on", type, Arrays.asList(options), listeners.size() - 1);
    }

    static UseRec useHandler(Map op) {
        String ref = (String) op.get("ref");
        UseRec h = useHandlers.get(ref);
        if (h == null) {
            Object a = op.get("allowed");
            int[] allowed = null;
            if (a != null) {
                List l = (List) a;
                allowed = new int[l.size()];
                for (int i = 0; i < allowed.length; i++) allowed[i] = n(l.get(i));
            }
            boolean dynamic = Boolean.TRUE.equals(op.get("dynamic"));
            h = "SecondUse".equals(op.get("class")) ? new SecondUse(ref, allowed, dynamic) : new FirstUse(ref, allowed, dynamic);
            useHandlers.put(ref, h);
        }
        return h;
    }

    static int i(Map op, String k) { return n(op.get(k)); }

    static Object run(Map op) {
        switch ((String) op.get("op")) {
            case "set": set(op); return null;
            case "item_action": {
                Packet.ItemAction p = new Packet.ItemAction();
                p.player = player; p.optIndex = i(op, "opt"); p.itemId = i(op, "item"); p.slot = i(op, "slot"); p.iface = i(op, "iface"); p.child = i(op, "child");
                PacketProcessor.item(p); return null;
            }
            case "npc_action": {
                Packet.NpcAction p = new Packet.NpcAction();
                p.player = player; p.optIndex = i(op, "opt"); p.npcIndex = i(op, "npc");
                PacketProcessor.npc(p); return null;
            }
            case "player_action": {
                Packet.PlayerAction p = new Packet.PlayerAction();
                p.player = player; p.optIndex = i(op, "opt"); p.otherIndex = i(op, "other");
                PacketProcessor.player(p); return null;
            }
            case "use_npc": {
                Packet.UseWithNpc p = new Packet.UseWithNpc();
                p.player = player; p.itemId = i(op, "item"); p.npcIndex = i(op, "npc"); p.iface = i(op, "iface"); p.slot = i(op, "slot");
                PacketProcessor.useWith(p); return null;
            }
            case "use_player": {
                Packet.UseWithPlayer p = new Packet.UseWithPlayer();
                p.player = player; p.itemId = i(op, "item"); p.otherIndex = i(op, "other"); p.iface = i(op, "iface"); p.slot = i(op, "slot");
                PacketProcessor.useWith(p); return null;
            }
            case "use_item": {
                Packet.UseWithItem p = new Packet.UseWithItem();
                p.player = player; p.usedId = i(op, "used"); p.usedWithId = i(op, "with"); p.usedSlot = i(op, "usedSlot"); p.usedWithSlot = i(op, "withSlot");
                p.usedIface = i(op, "usedIface"); p.usedWithIface = i(op, "withIface"); p.usedChild = 0; p.usedWithChild = 0;
                PacketProcessor.useWith(p); return null;
            }
            case "use_scenery": {
                Packet.UseWithScenery p = new Packet.UseWithScenery();
                p.player = player; p.itemId = i(op, "item"); p.slot = i(op, "slot"); p.sceneryId = i(op, "scenery"); p.x = i(op, "x"); p.y = i(op, "y");
                PacketProcessor.useWith(p); return null;
            }
            case "use_ground": {
                Packet.UseWithGroundItem p = new Packet.UseWithGroundItem();
                p.player = player; p.usedId = i(op, "used"); p.withId = i(op, "with"); p.iface = i(op, "iface"); p.child = 0; p.slot = i(op, "slot"); p.x = i(op, "x"); p.y = i(op, "y");
                PacketProcessor.useWith(p); return null;
            }
            case "comp_ground": {
                Packet.ComponentGroundItemAction p = new Packet.ComponentGroundItemAction();
                p.player = player; p.iface = i(op, "iface"); p.child = i(op, "child"); p.slot = 0; p.itemId = i(op, "item"); p.x = i(op, "x"); p.y = i(op, "y");
                PacketProcessor.component(p); return null;
            }
            case "comp_player": {
                Packet.ComponentPlayerAction p = new Packet.ComponentPlayerAction();
                p.player = player; p.otherIndex = i(op, "other"); p.iface = i(op, "iface"); p.child = i(op, "child");
                PacketProcessor.component(p); return null;
            }
            case "comp_scenery": {
                Packet.ComponentSceneryAction p = new Packet.ComponentSceneryAction();
                p.player = player; p.iface = i(op, "iface"); p.child = i(op, "child"); p.sceneryId = i(op, "scenery"); p.x = i(op, "x"); p.y = i(op, "y");
                PacketProcessor.component(p); return null;
            }
            case "comp_npc": {
                Packet.ComponentNpcAction p = new Packet.ComponentNpcAction();
                p.player = player; p.iface = i(op, "iface"); p.child = i(op, "child"); p.npcIndex = i(op, "npc");
                PacketProcessor.component(p); return null;
            }
            case "comp_item": {
                Packet.ComponentItemAction p = new Packet.ComponentItemAction();
                p.player = player; p.iface = i(op, "iface"); p.child = i(op, "child"); p.itemId = i(op, "item"); p.slot = i(op, "slot");
                PacketProcessor.component(p); return null;
            }
            case "switch_multi": {
                Packet.SlotSwitchMultiComponent p = new Packet.SlotSwitchMultiComponent();
                p.player = player; p.sourceIface = i(op, "srcIface"); p.sourceChild = 0; p.sourceSlot = i(op, "src");
                p.destIface = i(op, "dstIface"); p.destChild = i(op, "dstChild"); p.destSlot = i(op, "dst");
                PacketProcessor.slotSwitch(p); return null;
            }
            case "switch_single": {
                Packet.SlotSwitchSingleComponent p = new Packet.SlotSwitchSingleComponent();
                p.player = player; p.iface = i(op, "iface"); p.child = 0; p.sourceSlot = i(op, "src"); p.destSlot = i(op, "dst");
                p.isInsert = Boolean.TRUE.equals(op.get("insert"));
                PacketProcessor.slotSwitch(p); return null;
            }
            case "examine_item": { Packet.ItemExamine p = new Packet.ItemExamine(); p.player = player; p.id = i(op, "id"); PacketProcessor.process(p); return null; }
            case "examine_scenery": { Packet.SceneryExamine p = new Packet.SceneryExamine(); p.player = player; p.id = i(op, "id"); PacketProcessor.process(p); return null; }
            case "examine_npc": { Packet.NpcExamine p = new Packet.NpcExamine(); p.player = player; p.id = i(op, "id"); PacketProcessor.process(p); return null; }
            case "track": { Packet.TrackFinished p = new Packet.TrackFinished(); p.player = player; p.trackId = i(op, "track"); PacketProcessor.process(p); return null; }
            case "equip": new EquipHandler().handleEquip(player, node(op.get("node"))); return null;
            case "unequip": EquipHandler.unequip(player, i(op, "slot"), i(op, "item")); return null;
            case "equip_define": new EquipHandler().defineListeners(); return null;
            case "drop_define": new DropListener().defineListeners(); return null;
            case "drop": return DropListener.drop(player, (Item) node(op.get("node")));
            case "invoke": return i(op, "n") < listeners.size() ? listeners.get(i(op, "n")).invoke(player, node(op.get("node"))) : "none";
            case "use_run": UseWithHandler.run(new NodeUsageEvent(player, 0, node(op.get("used")), node(op.get("with")))); return null;
            case "use_register": UseWithHandler.addHandler(i(op, "id"), i(op, "type"), useHandler(op)); return null;
            case "pulse": return i(op, "n") < pulses.size() ? (Object) pulses.get(i(op, "n")).pulse() : "none";
            case "script": return i(op, "n") < scripts.size() ? scripts.get(i(op, "n")).invoke(0) : "none";
            case "dialogue": if (i(op, "n") >= actions.size()) return "none"; actions.get(i(op, "n")).handle(player, i(op, "button")); return null;
            default: throw new IllegalArgumentException((String) op.get("op"));
        }
    }

    static Map<String, Object> snapshot(Object result, String error) {
        Map<String, Object> out = new TreeMap<>();
        out.put("error", error);
        out.put("result", result);
        out.put("events", new ArrayList<>(events));
        Map<String, Object> cs = new TreeMap<>();
        for (Map.Entry<String, Container> e : containers.entrySet()) {
            List<Object> slots = new ArrayList<>();
            for (Item item : e.getValue().items) slots.add(key(item));
            cs.put(e.getKey(), slots);
        }
        out.put("containers", cs);
        Map<String, Object> indices = new TreeMap<>();
        for (Node node : nodes.values()) indices.put(node.key, node.nodeIndex);
        out.put("indices", indices);
        Map<String, Object> examines = new TreeMap<>();
        for (Map.Entry<Integer, ItemDefinition> e : itemDefs.entrySet()) examines.put("item:" + e.getKey(), e.getValue().examineField());
        for (Map.Entry<Integer, SceneryDefinition> e : sceneryDefs.entrySet()) examines.put("scenery:" + e.getKey(), e.getValue().examineField());
        out.put("examines", examines);
        Map<String, Object> allowed = new TreeMap<>();
        for (Map.Entry<String, UseRec> e : useHandlers.entrySet()) {
            int[] a;
            try { java.lang.reflect.Field f = UseWithHandler.class.getDeclaredField("allowedNodes"); f.setAccessible(true); a = (int[]) f.get(e.getValue()); }
            catch (ReflectiveOperationException x) { throw new IllegalStateException(x); }
            List<Object> l = null;
            if (a != null) { l = new ArrayList<>(); for (int x : a) l.add(x); }
            allowed.put(e.getKey(), l);
        }
        out.put("allowed", allowed);
        out.put("chatbox", flag("chatbox"));
        out.put("autocast", flag("autocast"));
        return out;
    }

    static void reset() {
        nodes = new TreeMap<>(); interactions = new TreeMap<>();
        flags = new TreeMap<>(); values = new TreeMap<>(); answers = new TreeMap<>(); attributes = new TreeMap<>();
        answerCalls = new HashMap<>(); calls = new HashMap<>(); events = new ArrayList<>();
        containers = new TreeMap<>();
        containers.put("inventory", new Container("inventory", 28));
        containers.put("bank", new BankContainer("bank", 496));
        containers.put("equipment", new EquipmentContainer("equipment", 14));
        repoPlayers = new HashMap<>(); repoNpcs = new HashMap<>();
        object = objectAt = ground = null;
        itemDefs = new TreeMap<>(); sceneryDefs = new TreeMap<>(); npcDefs = new TreeMap<>(); missingDefs = new TreeSet<>();
        pulses = new ArrayList<>(); scripts = new ArrayList<>(); actions = new ArrayList<>(); listeners = new ArrayList<>();
        useHandlers = new TreeMap<>();
        try { java.lang.reflect.Field f = UseWithHandler.class.getDeclaredField("HANDLERS"); f.setAccessible(true); ((Map) f.get(null)).clear(); }
        catch (ReflectiveOperationException e) { throw new IllegalStateException(e); }
    }

    public static void main(String[] args) throws Exception {
        Locale.setDefault(Locale.US);
        try (BufferedReader r = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = r.readLine()) != null) {
                input = (Map) new JSONParser().parse(line);
                Map initial = new TreeMap(input);
                reset();
                Map p = new TreeMap();
                p.put("kind", "player");
                Map ns = new TreeMap();
                ns.put("p", p);
                Map base = new TreeMap();
                base.put("nodes", ns);
                set(base);
                player = (Player) nodes.get("p");
                player.nodeLocation = Location.create(3222, 3218, 0);
                player.nodeName = "tracer";
                set(input);
                // Repository list sizes are fixed when the case starts.
                playerLength = core.ServerConstants.MAX_PLAYERS;
                npcLength = core.ServerConstants.MAX_NPCS;
                List<Object> states = new ArrayList<>();
                for (Object o : (List) input.get("ops")) {
                    Map op = (Map) o;
                    events.clear();
                    opCalls = 0;
                    failAt = op.containsKey("failAt") ? n(op.get("failAt")) : -1;
                    String error = null;
                    Object value = null;
                    try { value = run(op); }
                    catch (Failure e) { error = "Binding"; }
                    catch (NullPointerException e) { error = "Null"; }
                    catch (IndexOutOfBoundsException e) { error = "Index"; }
                    catch (ClassCastException e) { error = "Cast"; }
                    catch (RuntimeException e) { error = "Other: " + e; }
                    states.add(snapshot(value, error));
                }
                initial.put("expected", states);
                System.out.println(JSONValue.toJSONString(initial));
            }
        }
    }
}
