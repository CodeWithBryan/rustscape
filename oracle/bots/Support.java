package oracle;

import content.data.Quests;
import content.global.bots.Adventurer;
import content.global.bots.Idler;
import core.game.bots.*;
import core.game.ge.GrandExchangeOffer;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.link.prayer.PrayerType;
import core.game.node.item.GroundItem;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.system.task.Pulse;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import core.game.world.repository.NodeList;
import core.game.world.repository.Repository;
import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.*;
import kotlin.Unit;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Runs one case (Boot gives every case a class loader of its own, so the statics of AIPlayer, AIRepository,
 * GeneralBotCreator and PvMBotsBuilder start fresh) against the unmodified bot engine and returns what it did: every
 * recorded call of every operation, its result or failure class, and a snapshot of the player list, the statics and
 * every bot and script afterwards.
 */
public class Support {
    static Map input;
    static Map<String, Script> scripts = new TreeMap<>();
    static IdentityHashMap<Script, String> scriptLabels = new IdentityHashMap<>();
    static GeneralBotCreator creator;
    static Map<String, Object> pests = new LinkedHashMap<>();
    static List<Object> transitions = new ArrayList<>();
    static PrintStream realErr;

    static int n(Object o) { return ((Number) o).intValue(); }
    static Location loc(Object v) { return R.location(v); }

    // ------------------------------------------------------------ case entry
    public static String runCase(String line) throws Exception {
        input = (Map) new JSONParser().parse(line);
        R.input = input;
        R.answers = (Map) input.getOrDefault("answers", new HashMap());
        realErr = System.err;
        // Throwable.printStackTrace prints the throwable itself first: that line is the observation.
        System.setErr(new PrintStream(realErr, false, "UTF-8") {
            @Override public void println(Object x) {
                if (x instanceof Throwable) R.ev("@printStackTrace", x.getClass().getSimpleName());
            }
            @Override public void println(String x) {}
        });
        List<Object> expected = new ArrayList<>();
        try {
            setup();
            for (Object o : (List) input.get("ops")) expected.add(step((Map) o));
        } finally {
            System.setErr(realErr);
        }
        Map out = new LinkedHashMap(input);
        out.put("expected", expected);
        return JSONValue.toJSONString(out);
    }

    static void setup() throws Exception {
        Map config = (Map) input.getOrDefault("config", Collections.emptyMap());
        // A directory of its own for every case: the original never closes its Scanner, so the files of an
        // earlier case can still be open.
        int number = Integer.getInteger("oracle.case", 0) + 1;
        System.setProperty("oracle.case", Integer.toString(number));
        File data = new File(System.getProperty("oracle.data"), Integer.toString(number));
        data.mkdirs();
        for (Object e : ((Map) input.getOrDefault("files", Collections.emptyMap())).entrySet()) {
            Map.Entry entry = (Map.Entry) e;
            Files.write(new File(data, (String) entry.getKey()).toPath(), ((String) entry.getValue()).getBytes(StandardCharsets.UTF_8));
        }
        if (!Boolean.FALSE.equals(config.get("dataPath"))) core.ServerConstants.BOT_DATA_PATH = data.getPath().replace('\\', '/') + "/";
        if (config.containsKey("maxPathfind")) core.ServerConstants.MAX_PATHFIND_DISTANCE = n(config.get("maxPathfind"));
        core.ServerConstants.SERVER_GE_NAME = (String) config.getOrDefault("geName", "Bot Exchange");
        core.Server.setRunning(!Boolean.FALSE.equals(config.get("serverRunning")));
        if (!config.containsKey("geLimit")) core.game.world.GameWorld.setSettings(new core.game.world.GameSettings(500));
        else if (config.get("geLimit") != null) core.game.world.GameWorld.setSettings(new core.game.world.GameSettings(n(config.get("geLimit"))));
        if (config.containsKey("capacity")) Repository.players = new NodeList<>(n(config.get("capacity")));
        R.quiet = true;
        Map nodes = (Map) input.getOrDefault("nodes", Collections.emptyMap());
        // Players first: ground items name their dropper.
        for (Object key : new TreeSet<Object>(nodes.keySet())) if ("player".equals(((Map) nodes.get(key)).get("kind"))) make((String) key, (Map) nodes.get(key));
        for (Object key : new TreeSet<Object>(nodes.keySet())) if (!"player".equals(((Map) nodes.get(key)).get("kind"))) make((String) key, (Map) nodes.get(key));
        R.quiet = false;
        Map specs = (Map) input.getOrDefault("scripts", Collections.emptyMap());
        for (Object key : new TreeSet<Object>(specs.keySet())) {
            Map spec = (Map) specs.get(key);
            Script s;
            switch ((String) spec.getOrDefault("class", "CaseScript")) {
                case "Idler": s = new Idler(); break;
                case "Adventurer": s = new Adventurer(); if (spec.containsKey("state")) ((Adventurer) s).setState(Adventurer.State.valueOf((String) spec.get("state"))); break;
                default: s = new CaseScript();
            }
            for (Object i : (List) spec.getOrDefault("inventory", Collections.emptyList())) s.inventory.add(R.itemOf(i));
            for (Object i : (List) spec.getOrDefault("equipment", Collections.emptyList())) s.equipment.add(R.itemOf(i));
            for (Object p : (List) spec.getOrDefault("skills", Collections.emptyList())) s.skills.put(n(((List) p).get(0)), n(((List) p).get(1)));
            for (Object q : (List) spec.getOrDefault("quests", Collections.emptyList())) s.quests.add(Quests.valueOf((String) q));
            if (spec.containsKey("running")) s.running = (Boolean) spec.get("running");
            if (spec.containsKey("endDialogue")) s.endDialogue = (Boolean) spec.get("endDialogue");
            if (spec.get("bot") != null) s.bot = (Player) R.node(spec.get("bot"));
            scripts.put((String) key, s);
            scriptLabels.put(s, (String) key);
        }
    }

    static void make(String key, Map spec) {
        Location l = loc(spec.get("loc"));
        String name = (String) spec.get("name");
        int id = spec.containsKey("id") ? n(spec.get("id")) : 0;
        Node node;
        switch ((String) spec.get("kind")) {
            case "player": {
                PlayerDetails d = new PlayerDetails(name);
                Player p = new Player(d);
                d.owner = p;
                p.oracleLocation(l);
                node = p;
                break;
            }
            case "npc": node = new NPC(id, name, l); break;
            case "scenery": node = new Scenery(id, name, l); break;
            case "ground": node = new GroundItem(id, spec.containsKey("amount") ? n(spec.get("amount")) : 1, l, (Player) R.node(spec.get("dropper"))); break;
            case "item": node = new Item(id, spec.containsKey("amount") ? n(spec.get("amount")) : 1); break;
            default: throw new IllegalStateException(key);
        }
        node.key = key;
        if (Boolean.FALSE.equals(spec.get("active"))) node.oracleActive(false);
        if (spec.get("direction") != null) node.oracleDirection(Direction.valueOf((String) spec.get("direction")));
        R.nodes.put(key, node);
    }

    public static String scriptLabel(Script s) { String l = scriptLabels.get(s); if (l == null) throw new IllegalStateException("script"); return l; }
    public static Script newInstance(Script s) {
        Object a = R.ev("Script.newInstance", scriptLabel(s));
        return a == null ? null : scripts.get((String) a);
    }

    static String errorName(Throwable t) {
        if (t instanceof InvocationTargetException) return errorName(t.getCause());
        if (t instanceof R.Failure) return "Binding";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof ClassCastException) return "Cast";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        if (t instanceof NumberFormatException) return "Number";
        if (t instanceof IllegalArgumentException) return "Argument";
        if (t instanceof NoSuchElementException) return "NoSuchElement";
        java.io.StringWriter w = new java.io.StringWriter();
        t.printStackTrace(new java.io.PrintWriter(w));
        return t.getClass().getName() + ": " + w;
    }

    static Map step(Map op) {
        R.resetOp();
        R.failAt = op.containsKey("failAt") ? n(op.get("failAt")) : 0;
        Map out = new LinkedHashMap();
        Object result = null;
        String error = null;
        try { result = op(op); } catch (Throwable t) { error = errorName(t); }
        R.failAt = 0;
        out.put("events", R.events);
        out.put("result", R.value(result));
        out.put("error", error);
        try { out.put("state", state()); } catch (Exception e) { throw new RuntimeException(e); }
        return out;
    }

    /** A hook's action: an operation run in the middle of a recorded call. Its failure is the call's failure. */
    public static void hook(Map action) {
        try { op(action); }
        catch (RuntimeException | Error e) { throw e; }
        catch (InvocationTargetException e) { if (e.getCause() instanceof RuntimeException) throw (RuntimeException) e.getCause(); throw new RuntimeException(e.getCause()); }
        catch (Throwable t) { throw new RuntimeException(t); }
    }

    // ------------------------------------------------------------ snapshot
    static Object key(Object node) { return node == null ? null : R.label(node); }
    static Object coords(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    static Object field(Class<?> c, Object o, String name) throws Exception { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(o); }

    static boolean initialised(String name) throws Exception {
        Class<?> c = Class.forName(name, false, Support.class.getClassLoader());
        Class<?> unsafe = Class.forName("sun.misc.Unsafe");
        Field f = unsafe.getDeclaredField("theUnsafe");
        f.setAccessible(true);
        return !(Boolean) unsafe.getMethod("shouldBeInitialized", Class.class).invoke(f.get(null), c);
    }

    static Map state() throws Exception {
        Map s = new LinkedHashMap();
        List slots = new ArrayList();
        NodeList<Player> players = Repository.getPlayers();
        for (int i = 0; i < players.length(); i++) {
            Player p;
            try { p = players.get(i); } catch (IndexOutOfBoundsException e) { continue; }
            if (p != null) slots.add(Arrays.asList(i, key(p)));
        }
        Map list = new LinkedHashMap();
        list.put("size", players.size());
        list.put("slots", slots);
        s.put("players", list);
        boolean init = initialised("core.game.bots.AIPlayer");
        s.put("init", init);
        if (init) {
            s.put("uid", field(AIPlayer.class, null, "currentUID"));
            s.put("names", new ArrayList((List) field(AIPlayer.class, null, "botNames")));
            List mapping = new ArrayList();
            for (Map.Entry<Integer, AIPlayer> e : new TreeMap<Integer, AIPlayer>((Map<Integer, AIPlayer>) field(AIPlayer.class, null, "botMapping")).entrySet()) mapping.add(Arrays.asList(e.getKey(), key(e.getValue())));
            s.put("mapping", mapping);
            s.put("line", field(AIPlayer.class, null, "OSRScopyLine"));
        } else {
            s.put("uid", 1); s.put("names", new ArrayList()); s.put("mapping", new ArrayList()); s.put("line", null);
        }
        s.put("session", key(ArtificialSession.getSingleton().getObject()));
        List<List> ground = new ArrayList<>();
        for (Map.Entry<Player, ArrayList<GroundItem>> e : AIRepository.Companion.getGroundItems().entrySet()) {
            List items = new ArrayList();
            for (GroundItem g : e.getValue()) items.add(key(g));
            ground.add(Arrays.asList(key(e.getKey()), items));
        }
        ground.sort(Comparator.comparing(l -> l.get(0) == null ? "" : (String) l.get(0)));
        s.put("ground", ground);
        List<List> offers = new ArrayList<>();
        for (Map.Entry<Player, GrandExchangeOffer> e : AIRepository.Companion.getGEOffers().entrySet()) {
            GrandExchangeOffer o = e.getValue();
            offers.add(Arrays.asList(key(e.getKey()), o.getItemID(), o.getSell(), o.getOfferedValue(), o.getAmount(), o.getCompletedAmount(), key(o.getPlayer())));
        }
        offers.sort(Comparator.comparing(l -> (String) l.get(0)));
        s.put("offers", offers);
        List pulses = new ArrayList();
        for (Map.Entry<String, GeneralBotCreator.BotScriptPulse> e : AIRepository.getPulseRepository().entrySet()) pulses.add(Arrays.asList(e.getKey(), pulseState(e.getValue())));
        s.put("pulses", pulses);
        s.put("triggered", GeneralBotCreator.Companion.getBotPulsesTriggeredThisTick());
        s.put("spawned", PvMBotsBuilder.Companion.getBotsSpawned());
        List bots = new ArrayList();
        for (Object o : R.bots) {
            Player p = (Player) o;
            if (!(p instanceof AIPlayer)) throw new IllegalStateException("a recorded player that is not an AIPlayer");
            AIPlayer a = (AIPlayer) p;
            String kind = a instanceof CombatBot ? "Combat" : a instanceof PvMBots ? "Pvm" : "Plain";
            Object tick = a instanceof CombatBot ? (Object) ((CombatBot) a).getTick() : a instanceof PvMBots ? field(PvMBots.class, a, "tick") : (Object) 0;
            bots.add(Arrays.asList(key(a), kind, a.getUid(), a.getUsername(), coords(a.getStartLocation()), a.isHighPriority(), key(a.getControler()), a.isArtificial(),
                a.getIndex(), coords(a.getLocation()), a.isPlaying(), tick));
        }
        s.put("bots", bots);
        List scriptStates = new ArrayList();
        for (Map.Entry<String, Script> e : scripts.entrySet()) {
            Script x = e.getValue();
            scriptStates.add(Arrays.asList(e.getKey(), key(x.bot), x.running, x.endDialogue, x.scriptAPI != null));
        }
        s.put("scripts", scriptStates);
        List queue = new ArrayList();
        for (Map.Entry<String, Object[]> e : Repository.getDisconnectionQueue().queue.entrySet()) queue.add(Arrays.asList(e.getKey(), key(e.getValue()[0]), e.getValue()[1]));
        s.put("queue", queue);
        List identifiers = new ArrayList();
        for (Map.Entry<String, PlayerScripts.PlayerScript> e : PlayerScripts.INSTANCE.getIdentifierMap().entrySet()) {
            PlayerScripts.PlayerScript p = e.getValue();
            identifiers.add(Arrays.asList(e.getKey(), p.getIdentifier(), Arrays.asList(p.getDescription()), p.getName(), p.getClazz().getSimpleName()));
        }
        s.put("identifiers", identifiers);
        return s;
    }

    static Object pulseState(GeneralBotCreator.BotScriptPulse p) {
        return Arrays.asList(scriptLabel(p.getBotScript()), p.isPlayer(), p.getTicks(), p.getRandomDelay(), coords(p.getLastBotLocation()), p.getLastBotMoveTicks(), p.running);
    }

    // ------------------------------------------------------------ operations
    static Player player(Map op, String k) { return (Player) R.node(op.get(k)); }
    static AIPlayer bot(Map op) { return (AIPlayer) R.node(op.get("bot")); }
    static Node node(Map op, String k) { return (Node) R.node(op.get(k)); }
    static ScriptAPI api(Map op) { return new ScriptAPI(player(op, "api")); }
    static Script script(Map op) { return op.get("script") == null ? null : scripts.get((String) op.get("script")); }
    static int i(Map op, String k) { return n(op.get(k)); }
    static boolean flag(Map op, String k) { return Boolean.TRUE.equals(op.get(k)); }
    static Boolean nullable(Map op, String k) { return (Boolean) op.get(k); }
    static List<Integer> ints(Object v) { List<Integer> out = new ArrayList<>(); for (Object o : (List) v) out.add(n(o)); return out; }
    static List<String> strings(Object v) { List<String> out = new ArrayList<>(); for (Object o : (List) v) out.add((String) o); return out; }
    static Object invoke(Object target, String name) throws Throwable {
        Method m = AIPlayer.class.getDeclaredMethod(name);
        m.setAccessible(true);
        try { return m.invoke(target); } catch (InvocationTargetException e) { throw e.getCause(); }
    }
    static Pulse pulse(Map op) {
        String key = (String) op.get("pulse");
        return (Pulse) R.node("last".equals(key) ? R.lastPulse : key);
    }
    static CombatBotAssembler.Tier tier(Map op) { return CombatBotAssembler.Tier.valueOf((String) op.get("tier")); }

    static Object op(Map op) throws Throwable {
        String name = (String) op.get("op");
        switch (name) {
            // ---- setup, not calls
            case "setLocation": node(op, "node").oracleLocation(loc(op.get("loc"))); return null;
            case "setActive": node(op, "node").oracleActive(flag(op, "value")); return null;
            case "setOffer": AIRepository.getOffer(player(op, "player")).setCompletedAmount(i(op, "completed")); return null;
            case "setScript": {
                Script s = script(op);
                if (op.containsKey("bot")) s.bot = op.get("bot") == null ? null : player(op, "bot");
                if (op.containsKey("running")) s.running = flag(op, "running");
                if (op.containsKey("endDialogue")) s.endDialogue = flag(op, "endDialogue");
                return null;
            }
            case "setTriggered": GeneralBotCreator.Companion.setBotPulsesTriggeredThisTick(i(op, "value")); return null;
            case "setPulseDelay": ((GeneralBotCreator.BotScriptPulse) pulse(op)).setRandomDelay(i(op, "value")); return null;
            // ---- AIPlayer
            case "newAIPlayer": return new AIPlayer(loc(op.get("loc")));
            case "newAIPlayerFile": return new AIPlayer((String) op.get("file"), loc(op.get("loc")));
            case "combatBot": return new CombatBot(loc(op.get("loc")));
            case "pvmBots": return new PvMBots(loc(op.get("loc")));
            case "pvmBotsFile": return new PvMBots((String) op.get("file"), loc(op.get("loc")));
            case "loadNames": AIPlayer.loadNames((String) op.get("file")); return null;
            case "getRandomName": return AIPlayer.getRandomName();
            case "updateRandomOSRScopyLine": AIPlayer.updateRandomOSRScopyLine((String) op.get("file")); return null;
            case "get": return AIPlayer.get(i(op, "uid"));
            case "deregister": AIPlayer.deregister(i(op, "uid")); return null;
            case "tick": bot(op).tick(); return null;
            case "update": bot(op).update(); return null;
            case "clear": bot(op).clear(); return null;
            case "reset": bot(op).reset(); return null;
            case "init": bot(op).init(); return null;
            case "updateRandomValues": bot(op).updateRandomValues(); return null;
            case "finalizeDeath": bot(op).finalizeDeath(op.get("killer") == null ? null : (Entity) node(op, "killer")); return null;
            case "follow": bot(op).follow((Entity) node(op, "entity")); return null;
            case "randomWalkAroundPoint": bot(op).randomWalkAroundPoint(loc(op.get("point")), i(op, "radius")); return null;
            case "randomWalk": bot(op).randomWalk(i(op, "x"), i(op, "y")); return null;
            case "walkToPosSmartXY": bot(op).walkToPosSmart(i(op, "x"), i(op, "y")); return null;
            case "walkToPosSmart": bot(op).walkToPosSmart(loc(op.get("loc"))); return null;
            case "walkPos": bot(op).walkPos(i(op, "x"), i(op, "y")); return null;
            case "checkVictimIsPlayer": return bot(op).checkVictimIsPlayer();
            case "getItemById": return bot(op).getItemById(i(op, "id"));
            case "getClosestNodeWithEntry": return bot(op).getClosestNodeWithEntry(i(op, "range"), i(op, "entry"));
            case "getClosestNodeWithEntries": return bot(op).getClosestNodeWithEntry(i(op, "range"), ints(op.get("entries")));
            case "getClosestNodeWithEntryAndDirection": return bot(op).getClosestNodeWithEntryAndDirection(i(op, "range"), i(op, "entry"), Direction.valueOf((String) op.get("direction")));
            case "getClosesCreature": return bot(op).getClosesCreature(i(op, "radius"));
            case "getClosesCreatureEntry": return bot(op).getClosesCreature(i(op, "radius"), i(op, "entry"));
            case "getClosesCreatureEntries": return bot(op).getClosesCreature(i(op, "radius"), new ArrayList<Integer>(ints(op.get("entries"))));
            case "setHighPriority": bot(op).setHighPriority(flag(op, "value")); return null;
            case "setControler": bot(op).setControler(op.get("player") == null ? null : player(op, "player")); return null;
            case "interactNoop": bot(op).interact(node(op, "node")); bot(op).handleIncomingChat(null); return null;
            case "setLevels": return invoke(bot(op), "setLevels");
            case "giveArmor": return invoke(bot(op), "giveArmor");
            case "session": {
                ArtificialSession s = ArtificialSession.getSingleton();
                s.write(new Object(), true);
                s.write(new Object());
                s.queue(null);
                s.write();
                s.disconnect();
                return s.getRemoteAddress();
            }
            // ---- AIRepository
            case "addItem": AIRepository.addItem((GroundItem) node(op, "item")); return null;
            case "getItems": return AIRepository.getItems(player(op, "player"));
            case "addOffer": {
                GrandExchangeOffer o = new GrandExchangeOffer();
                o.setItemID(i(op, "id")); o.setAmount(i(op, "amount")); o.setPlayer(player(op, "player"));
                AIRepository.addOffer(player(op, "player"), o);
                return null;
            }
            case "getOffer": { GrandExchangeOffer o = AIRepository.getOffer(player(op, "player")); return o == null ? null : Arrays.asList(o.getItemID(), o.getAmount()); }
            case "clearAllBots": AIRepository.clearAllBots(); return null;
            case "sendBotInfo": AIRepository.sendBotInfo(player(op, "player"), bot(op)); return null;
            // ---- Script
            case "scriptInit": script(op).init(flag(op, "isPlayer")); return null;
            case "scriptToString": return script(op).toString();
            case "scriptSetLevel": script(op).setLevel(i(op, "skill"), i(op, "level")); return null;
            // ---- GeneralBotCreator
            case "gbcLoc": creator = new GeneralBotCreator(script(op), loc(op.get("loc"))); return null;
            case "gbcBot": creator = new GeneralBotCreator(script(op), op.get("bot") == null ? null : bot(op)); return null;
            case "gbcPlayer": creator = new GeneralBotCreator(script(op), player(op, "player"), flag(op, "isPlayer")); return null;
            case "gbcTick": new GBCTick().tick(); return null;
            case "transitionNew": {
                GeneralBotCreator.TransitionPulse t = creator.new TransitionPulse(script(op));
                transitions.add(t);
                return Arrays.asList(R.label(t), t.getDelay());
            }
            case "runPulse": return pulse(op).pulse();
            case "stopPulse": pulse(op).stop(); return null;
            case "runLaunched": R.launched.get(i(op, "task") - 1).run(); return null;
            // ---- ScriptAPI
            case "apiDistance": return api(op).distance(node(op, "a"), node(op, "b"));
            case "interact": api(op).interact(player(op, "player"), op.get("node") == null ? null : node(op, "node"), (String) op.get("option")); return null;
            case "useWith": api(op).useWith(player(op, "player"), i(op, "item"), op.get("node") == null ? null : node(op, "node")); return null;
            case "sendChat": api(op).sendChat((String) op.get("message")); return null;
            case "getNearestNodeFromList": return api(op).getNearestNodeFromList(strings(op.get("names")), flag(op, "isObject"));
            case "getNearestNodeId": return api(op).getNearestNode(i(op, "id"), flag(op, "isObject"));
            case "getNearestNodeName": return api(op).getNearestNode((String) op.get("name"));
            case "getNearestNodeNameObj": return api(op).getNearestNode((String) op.get("name"), flag(op, "isObject"));
            case "getNearestObjectByPredicate": return api(op).getNearestObjectByPredicate(x -> R.b("Predicate.invoke", x));
            case "evaluateViability":
                return api(op).evaluateViability(op.get("node") == null ? null : node(op, "node"), ((Number) op.get("min")).doubleValue(), ((Number) op.get("max")).doubleValue(),
                    op.get("names") == null ? null : strings(op.get("names")), op.containsKey("id") ? i(op, "id") : -1, flag(op, "predicate") ? x -> R.b("Predicate.invoke", x) : null);
            case "takeNearestGroundItem": return api(op).takeNearestGroundItem(i(op, "id"));
            case "getNearestGameObject": return api(op).getNearestGameObject(loc(op.get("loc")), i(op, "id"));
            case "attackNpcsInRadius": return api(op).attackNpcsInRadius(player(op, "player"), i(op, "radius"));
            case "attackNpcInRadius": return api(op).attackNpcInRadius(player(op, "player"), (String) op.get("name"), i(op, "radius"));
            case "walkTo": api(op).walkTo(loc(op.get("loc"))); return null;
            case "walkArray": {
                List l = (List) op.get("steps");
                Location[] steps = new Location[l.size()];
                for (int k = 0; k < steps.length; k++) steps[k] = loc(l.get(k));
                api(op).walkArray(steps);
                return null;
            }
            case "randomWalkTo": api(op).randomWalkTo(loc(op.get("loc")), i(op, "radius")); return null;
            case "randomizeLocationInRanges": {
                // kotlin.random.Random cannot be seeded or replaced: the two draws are what the result shows.
                Location from = loc(op.get("loc"));
                try {
                    Location to = api(op).randomizeLocationInRanges(from, i(op, "xMin"), i(op, "xMax"), i(op, "yMin"), i(op, "yMax"), i(op, "z"));
                    int dx = to.getX() - from.getX(), dy = to.getY() - from.getY();
                    if (dx < i(op, "xMin") || dx >= i(op, "xMax") || dy < i(op, "yMin") || dy >= i(op, "yMax")) throw new IllegalStateException("draw out of range");
                    R.events.add(Arrays.asList("@kotlin.Random.nextInt", i(op, "xMin"), i(op, "xMax"), dx));
                    R.events.add(Arrays.asList("@kotlin.Random.nextInt", i(op, "yMin"), i(op, "yMax"), dy));
                    return Arrays.asList(dx, dy, to.getZ());
                } catch (IllegalArgumentException e) {
                    // The first draw fails for an empty x range; otherwise the first was made and the second failed.
                    if (i(op, "xMax") > i(op, "xMin")) R.events.add(Arrays.asList("@kotlin.Random.nextInt", i(op, "xMin"), i(op, "xMax"), null));
                    throw e;
                }
            }
            case "teleportToGE": return api(op).teleportToGE();
            case "teleport": return api(op).teleport(loc(op.get("loc")));
            case "sellOnGE": api(op).sellOnGE(i(op, "id")); return null;
            case "sellAllOnGe": api(op).sellAllOnGe(); return null;
            case "sellAllOnGeAdv": api(op).sellAllOnGeAdv(); return null;
            case "depositAtBank": api(op).depositAtBank(); return null;
            case "saleIsBigNews": return api(op).saleIsBigNews(i(op, "id"), i(op, "amount"));
            case "bankItem": api(op).bankItem(i(op, "item")); return null;
            case "bankAll": api(op).bankAll(flag(op, "onComplete") ? () -> { R.v("OnComplete.invoke"); return Unit.INSTANCE; } : null); return null;
            case "eat": api(op).eat(i(op, "food")); return null;
            case "forceEat": api(op).forceEat(i(op, "food")); return null;
            case "buyFromGE": api(op).buyFromGE(player(op, "player"), i(op, "id"), i(op, "amount")); return null;
            case "withdraw": api(op).withdraw(i(op, "id"), i(op, "amount")); return null;
            case "equipAndSetStatsList": {
                List<Item> items = null;
                if (op.get("items") != null) { items = new ArrayList<>(); for (Object o : (List) op.get("items")) items.add(R.itemOf(o)); }
                api(op).equipAndSetStats(items);
                return null;
            }
            case "equipAndSetStats": api(op).equipAndSetStats(R.itemOf(op.get("item"))); return null;
            case "loadAppearanceAndEquipment": api(op).loadAppearanceAndEquipment(op.get("json") == null ? null : (JSONObject) new JSONParser().parse((String) op.get("json"))); return null;
            case "overlay": {
                ScriptAPI.BottingOverlay o = api(op).getOverlay();
                o.init();
                o.setTitle((String) op.get("title"));
                o.setTaskLabel((String) op.get("label"));
                o.setAmount(i(op, "amount"));
                return null;
            }
            case "checkPriceOverrides": return api(op).checkPriceOverrides(i(op, "id"));
            // ---- AIPBuilder, CombatBot, PvMBots, PvMBotsBuilder
            case "aipCreate": return AIPBuilder.create(loc(op.get("loc")));
            case "aipCopy": return AIPBuilder.copy(player(op, "player"));
            case "aipCopyAt": return AIPBuilder.copy(player(op, "player"), loc(op.get("loc")));
            case "randomAfkPlayer": AIPBuilder.RandomAfkPlayer(loc(op.get("loc"))); return null;
            case "immersiveSpawns": AIPBuilder.immersiveSpawns(); return null;
            case "checkPrayer": {
                List l = (List) op.get("types");
                PrayerType[] types = new PrayerType[l.size()];
                for (int k = 0; k < types.length; k++) types[k] = l.get(k) == null ? null : PrayerType.valueOf((String) l.get(k));
                if (bot(op) instanceof CombatBot) ((CombatBot) bot(op)).CheckPrayer(types); else ((PvMBots) bot(op)).CheckPrayer(types);
                return null;
            }
            case "eatBot": if (bot(op) instanceof CombatBot) ((CombatBot) bot(op)).eat(i(op, "food")); else ((PvMBots) bot(op)).eat(i(op, "food")); return null;
            case "findTargets": return ((PvMBots) bot(op)).FindTargets((Entity) node(op, "entity"), i(op, "radius"));
            case "checkValidTargets": return ((PvMBots) bot(op)).checkValidTargets((NPC) node(op, "target"));
            case "attackNpcsInRadiusPvm":
                return op.get("player") == null ? ((PvMBots) bot(op)).AttackNpcsInRadius(i(op, "radius")) : ((PvMBots) bot(op)).AttackNpcsInRadius(player(op, "player"), i(op, "radius"));
            case "pvmCreate": return PvMBotsBuilder.Companion.create(loc(op.get("loc")));
            case "pvmCreatePc": {
                Object o = flag(op, "second") ? (Object) PvMBotsBuilder.createPestControlTestBot2(loc(op.get("loc"))) : PvMBotsBuilder.createPestControlTestBot(loc(op.get("loc")));
                String label = "pest#" + (pests.size() + 1);
                pests.put(label, o);
                return label;
            }
            // ---- CombatBotAssembler, SkillingBotAssembler
            case "produce": return new CombatBotAssembler().produce(CombatBotAssembler.Type.valueOf((String) op.get("type")), tier(op), loc(op.get("loc")));
            case "assembleRangedBot": return new CombatBotAssembler().assembleRangedBot(tier(op), loc(op.get("loc")), nullable(op, "crossbow"));
            case "assembleMeleeBot": return new CombatBotAssembler().assembleMeleeBot(tier(op), loc(op.get("loc")));
            case "meleeAdventurer": return new CombatBotAssembler().MeleeAdventurer(tier(op), loc(op.get("loc")));
            case "rangeAdventurer": return new CombatBotAssembler().RangeAdventurer(tier(op), loc(op.get("loc")));
            case "assembleMeleeDragonBot": return new CombatBotAssembler().assembleMeleeDragonBot(tier(op), loc(op.get("loc")));
            case "assembleRangeDragonBot": return new CombatBotAssembler().assembleRangeDragonBot(tier(op), loc(op.get("loc")));
            case "gearRangedBot": new CombatBotAssembler().gearRangedBot(bot(op), nullable(op, "crossbow")); return null;
            case "gearMeleeBot": new CombatBotAssembler().gearMeleeBot(bot(op)); return null;
            case "gearPCnRangedBot": new CombatBotAssembler().gearPCnRangedBot(bot(op), nullable(op, "crossbow")); return null;
            case "gearPCnMeleeBot": new CombatBotAssembler().gearPCnMeleeBot(bot(op)); return null;
            case "gearPCiRangedBot": new CombatBotAssembler().gearPCiRangedBot(bot(op), nullable(op, "crossbow")); return null;
            case "gearPCiMeleeBot": new CombatBotAssembler().gearPCiMeleeBot(bot(op)); return null;
            case "generateStats": {
                List<Integer> l = ints(op.get("skills"));
                int[] skills = new int[l.size()];
                for (int k = 0; k < skills.length; k++) skills[k] = l.get(k);
                new CombatBotAssembler().generateStats(bot(op), tier(op), skills);
                return null;
            }
            case "lvlAvg": return new CombatBotAssembler().lvlAvg(R.itemOf(op.get("item")));
            case "skillingProduce": return new SkillingBotAssembler().produce(SkillingBotAssembler.Wealth.valueOf((String) op.get("wealth")), loc(op.get("loc")));
            case "assembleBot": return new SkillingBotAssembler().assembleBot(bot(op), SkillingBotAssembler.Wealth.valueOf((String) op.get("wealth")));
            case "equipSet": return new SkillingBotAssembler().equipSet(bot(op), ints(op.get("set")));
            // ---- PlayerScripts
            case "playerScript": {
                List<String> d = strings(op.get("description"));
                PlayerScripts.INSTANCE.getIdentifierMap().put((String) op.get("identifier"),
                    new PlayerScripts.PlayerScript((String) op.get("identifier"), d.toArray(new String[0]), (String) op.get("name"), script(op).getClass()));
                return null;
            }
            default: throw new IllegalStateException("unknown operation " + name);
        }
    }
}
