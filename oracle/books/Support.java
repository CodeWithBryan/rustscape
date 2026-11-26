package oracle;

import java.io.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function3;
import kotlin.jvm.functions.Function6;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.ServerConstants;
import core.game.component.Component;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.world.GameSettings;
import core.game.world.GameWorld;
import content.global.handlers.iface.BookInterface;
import content.global.handlers.item.book.GeneralRuleBook;

/**
 * Runs one case (oracle.boot.Boot gives every case a class loader of its own, so each book class initialises
 * with the case's names) against the unchanged BookInterface and the five global books, and returns what they did.
 */
public class Support {
    public static class Failure extends RuntimeException {}

    static Map input;
    static List<Object> events;
    static int opCalls, failAt;
    static Map<String, Integer> answerCalls;
    static Player player;
    static Map<Integer, Function2> items;
    static Map<Integer, Function6> interfaces;

    static int n(Object v) { return ((Number) v).intValue(); }
    static Map<String, Object> map(Object... v) { Map<String, Object> m = new TreeMap<>(); for (int i = 0; i < v.length; i += 2) m.put((String) v[i], v[i + 1]); return m; }

    public static void emit(String name, Object... args) {
        List<Object> e = new ArrayList<>();
        e.add(name);
        Collections.addAll(e, args);
        events.add(e);
        opCalls++;
        if (opCalls == failAt) throw new Failure();
    }
    public static <T> T answer(String name, T fallback) {
        Map answers = (Map) input.get("answers");
        if (answers == null || !answers.containsKey(name)) return fallback;
        Object v = answers.get(name);
        if (v instanceof List) {
            int i = answerCalls.merge(name, 1, Integer::sum) - 1;
            List l = (List) v;
            v = i < l.size() ? l.get(i) : l.get(l.size() - 1);
        }
        return (T) v;
    }
    public static Object value(Object v) {
        if (v == null || v instanceof String || v instanceof Boolean || v instanceof Integer) return v;
        if (v instanceof kotlin.Function) return map("function", v.getClass().getName());
        if (v instanceof Long) return map("long", v);
        return map("class", v.getClass().getName());
    }

    // ------------------------------------------------------------------ the listener registries
    public static void on(int id, String type, String[] options, Function2 handler) {
        emit("on", id, type, Arrays.asList(options), handler.getClass().getName());
        items.put(id, handler);
    }
    public static void onInterface(int id, Function6 handler) {
        emit("onInterface", id, handler.getClass().getName());
        interfaces.put(id, handler);
    }

    static Object define(String name) throws Exception {
        Object listener = Class.forName(name).getDeclaredConstructor().newInstance();
        if (listener instanceof core.game.interaction.InterfaceListener) ((core.game.interaction.InterfaceListener) listener).defineInterfaceListeners();
        if (listener instanceof core.game.interaction.InteractionListener) ((core.game.interaction.InteractionListener) listener).defineListeners();
        return null;
    }

    static Object act(Map a) throws Throwable {
        String op = (String) a.get("op");
        switch (op) {
            case "define": return define((String) a.get("class"));
            case "read": {
                Function2 f = items.get(n(a.get("item")));
                if (f == null) throw new AssertionError("no listener for " + a.get("item"));
                return f.invoke(player, new Node("item" + a.get("item")));
            }
            case "button": {
                Function6 f = interfaces.get(n(a.get("component")));
                if (f == null) throw new AssertionError("no listener for " + a.get("component"));
                return f.invoke(player, new Component(n(a.get("component"))), n(a.getOrDefault("opcode", 155L)), n(a.get("button")), n(a.getOrDefault("slot", -1L)), n(a.getOrDefault("item", -1L)));
            }
            case "rules": GeneralRuleBook.Companion.openBook(player); return null;
            case "model":
                BookInterface.Companion.setModelOnPage(player, n(a.get("pageSet")), n(a.get("model")), n(a.get("component")), n(a.get("enable")), n(a.get("draw")), n(a.get("zoom")), n(a.get("pitch")), n(a.get("yaw")));
                return null;
            case "itemOnPage":
                BookInterface.Companion.setItemOnPage(player, n(a.get("pageSet")), n(a.get("itemId")), n(a.get("component")), n(a.get("enable")), n(a.get("draw")), n(a.get("zoom")), n(a.get("pitch")), n(a.get("yaw")));
                return null;
            case "clearModels": BookInterface.Companion.clearModelsOnPage(player, n(a.get("component"))); return null;
            case "isLastPage": return BookInterface.Companion.isLastPage(n(a.get("page")), n(a.get("total")));
            case "attr": {
                Object v = a.get("value");
                if (v instanceof Long) v = ((Long) v).intValue();
                if (v == null) player.attributeMap.remove(a.get("key")); else player.attributeMap.put((String) a.get("key"), v);
                return null;
            }
            default: throw new AssertionError(a);
        }
    }

    static String error(Throwable t) {
        if (t instanceof InvocationTargetException) return error(t.getCause());
        if (t instanceof Failure) return "Failure";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof ClassCastException) return "ClassCast";
        if (t instanceof ArrayIndexOutOfBoundsException) return "Index";
        throw new AssertionError("unexpected original exception", t);
    }

    public static String runCase(String line) throws Exception {
        JSONObject c = (JSONObject) new JSONParser().parse(line);
        input = c;
        events = new ArrayList<>(); answerCalls = new HashMap<>(); items = new HashMap<>(); interfaces = new HashMap<>();
        Map world = (Map) c.getOrDefault("world", Collections.emptyMap());
        GameWorld.setSettings(new GameSettings((String) world.getOrDefault("settingsName", "2009scape")));
        ServerConstants.SERVER_NAME = (String) world.getOrDefault("serverName", "2009scape");
        player = new Player("player");
        List<Object> out = new ArrayList<>();
        for (Object raw : (List) c.get("ops")) {
            Map a = (Map) raw;
            events.clear();
            opCalls = 0;
            failAt = a.containsKey("failAt") ? n(a.get("failAt")) : -1;
            Map<String, Object> result = new TreeMap<>();
            try {
                result.put("value", act(a));
            } catch (Throwable t) {
                result.put("error", error(t));
            }
            failAt = -1;
            Map<String, Object> attributes = new TreeMap<>();
            for (Map.Entry<String, Object> e : player.attributeMap.entrySet()) attributes.put(e.getKey(), value(e.getValue()));
            out.add(map("result", result, "events", new ArrayList<>(events), "attributes", attributes));
        }
        c.put("expected", out);
        StringWriter w = new StringWriter();
        JSONValue.writeJSONString(c, w);
        return w.toString();
    }
}
