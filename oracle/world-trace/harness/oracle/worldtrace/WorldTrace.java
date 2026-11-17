package oracle.worldtrace;

import core.Server;
import core.cache.def.impl.ItemDefinition;
import core.cache.def.impl.NPCDefinition;
import core.cache.def.impl.SceneryDefinition;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.login.LoginConfiguration;
import core.game.system.SystemManager;
import core.game.system.SystemState;
import core.game.system.config.ServerConfigParser;
import core.game.system.task.Pulse;
import core.game.system.timer.TimerRegistry;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import core.game.world.repository.Repository;
import core.game.world.update.UpdateSequence;
import core.plugin.ClassScanner;
import java.io.File;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/**
 * Boots the original server in this JVM without sockets and without its
 * real-time loop, runs one scripted scenario and writes a trace.
 *
 * Usage (working directory = the scenario's world directory, which holds
 * worldprops/ and data/): WorldTrace SCENARIO_JSON TRACE_OUT
 */
public final class WorldTrace {
    static final Map<String, TraceSession> sessions = new LinkedHashMap<>();
    static final List<String> lines = new ArrayList<>();
    static long errLines;
    static Path world;
    static Map<String, String> files;
    static int sequence;

    /** Counts lines the original prints to standard error (its caught stack traces). */
    static final class CountingErr extends OutputStream {
        final OutputStream next;
        CountingErr(OutputStream next) { this.next = next; }
        @Override public void write(int b) throws java.io.IOException { if (b == '\n') errLines++; next.write(b); }
        @Override public void flush() throws java.io.IOException { next.flush(); }
    }

    /** Path to "size:modified" of every file in the world directory; a changed entry is then read and hashed. */
    static Map<String, String> snapshot() throws java.io.IOException {
        Map<String, String> out = new TreeMap<>();
        try (Stream<Path> walk = Files.walk(world)) {
            for (Path file : walk.filter(Files::isRegularFile).collect(Collectors.toList())) {
                out.put(world.relativize(file).toString().replace('\\', '/'), Files.size(file) + ":" + Files.getLastModifiedTime(file).to(java.util.concurrent.TimeUnit.NANOSECONDS));
            }
        }
        return out;
    }

    /** Files created, changed or removed in the world directory since the previous call. */
    static List<Object> changedFiles() throws java.io.IOException {
        Map<String, String> now = snapshot();
        List<Object> out = new ArrayList<>();
        TreeSet<String> names = new TreeSet<>(now.keySet());
        names.addAll(files.keySet());
        for (String name : names) {
            String before = files.get(name), after = now.get(name);
            if (after != null && after.equals(before)) continue;
            Map<String, Object> entry = new LinkedHashMap<>();
            entry.put("path", name);
            if (after == null) {
                entry.put("removed", true);
            } else {
                byte[] data = Files.readAllBytes(world.resolve(name));
                entry.put("size", data.length);
                entry.put("sha256", Json.sha256(data));
                // Character saves are small and are the point of the record; other files are identified by digest.
                if (name.startsWith("data/players/")) entry.put("hex", Json.hex(data));
            }
            out.add(entry);
        }
        files = now;
        return out;
    }

    static List<Object> names(Collection<?> objects) {
        List<Object> out = new ArrayList<>();
        for (Object object : objects) out.add(object.getClass().getName());
        return out;
    }

    static Object staticField(Class<?> owner, String name) throws ReflectiveOperationException {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(null);
    }

    static Map<String, Object> generators() {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("clock", Pin.now);
        out.put("clockReads", Pin.clockReads.get());
        out.put("math", counting(Pin.math));
        out.put("seeded", Pin.seeds.get());
        out.put("shared", counting(Pin.shared));
        out.put("shuffle", counting(Pin.shuffle));
        out.put("threadLocal", Pin.threadLocalState());
        return out;
    }

    static List<Object> counting(Pin.Counting generator) {
        List<Object> out = new ArrayList<>();
        out.add(generator.draws());
        out.add(generator.state());
        return out;
    }

    /**
     * Repository.npcs by slot. A slot and the index its NPC reports are recorded separately: the original
     * can hold one NPC in two slots, and it then reports the later one.
     */
    static Map<String, Object> npcs(boolean full) {
        StringBuilder index = new StringBuilder(), state = new StringBuilder();
        List<Object> list = new ArrayList<>();
        int count = 0;
        for (int slot = 1; slot < Repository.getNpcs().length(); slot++) {
            NPC npc = Repository.getNpcs().get(slot);
            if (npc == null) continue;
            Location l = npc.getLocation();
            count++;
            index.append(slot).append(':').append(npc.getIndex()).append(':').append(npc.getId()).append(';');
            state.append(slot).append(':').append(npc.getIndex()).append(':').append(npc.getId()).append(':');
            if (l == null) state.append("null;"); else state.append(l.getX()).append(',').append(l.getY()).append(',').append(l.getZ()).append(';');
            if (full) {
                List<Object> row = new ArrayList<>();
                row.add(slot);
                row.add(npc.getIndex());
                row.add(npc.getId());
                row.add(l == null ? null : l.getX());
                row.add(l == null ? null : l.getY());
                row.add(l == null ? null : l.getZ());
                list.add(row);
            }
        }
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("count", count);
        out.put("indexSha256", Json.sha256(index.toString()));
        out.put("renderable", Repository.getRenderableNpcs().size());
        out.put("stateSha256", Json.sha256(state.toString()));
        if (full) out.put("list", list);
        return out;
    }

    static List<Object> players() {
        List<Object> out = new ArrayList<>();
        for (int slot = 1; slot < Repository.getPlayers().length(); slot++) {
            Player player = Repository.getPlayers().get(slot);
            if (player == null) continue;
            Location l = player.getLocation();
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("slot", slot);
            row.put("active", player.isActive());
            row.put("artificial", player.isArtificial());
            row.put("index", player.getIndex());
            row.put("location", l == null ? null : java.util.Arrays.<Object>asList(l.getX(), l.getY(), l.getZ()));
            row.put("name", player.getName());
            row.put("playing", player.isPlaying());
            row.put("queuedForDisconnect", Repository.getDisconnectionQueue().contains(player.getName()));
            row.put("renderable", UpdateSequence.getRenderablePlayers().contains(player));
            out.add(row);
        }
        return out;
    }

    /** One record: what the step did to the world and what every session was sent. */
    static void record(String type, Map<String, Object> extra) throws Exception {
        long before = errLines;
        long[] pumped = Handoffs.pump();
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("t", type);
        record.put("i", sequence++);
        record.putAll(extra);
        record.put("tick", GameWorld.getTicks());
        Map<String, Object> handoffs = new LinkedHashMap<>();
        handoffs.put("blocks", pumped[0]);
        handoffs.put("events", Handoffs.drainPublished());
        handoffs.put("foreign", Handoffs.drainForeign());
        record.put("handoffs", handoffs);
        Map<String, Object> out = new LinkedHashMap<>();
        for (TraceSession session : sessions.values()) {
            if (session.out.isEmpty()) continue;
            out.put(session.id, new ArrayList<>(session.out));
            session.out.clear();
        }
        record.put("out", out);
        Map<String, Object> state = new LinkedHashMap<>();
        for (TraceSession session : sessions.values()) {
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("active", session.isActive());
            row.put("producer", session.getProducer().getClass().getSimpleName());
            state.put(session.id, row);
        }
        record.put("sessions", state);
        record.put("players", players());
        record.put("lobby", Repository.getLobbyPlayers().stream().map(Player::getName).collect(Collectors.toList()));
        record.put("npcs", npcs(type.equals("boot")));
        record.put("pulses", GameWorld.getPulser().getCurrentPulses().length);
        record.put("generators", generators());
        record.put("files", changedFiles());
        record.put("stderrLines", errLines - before + (extra.containsKey("_err") ? (Long) extra.get("_err") : 0L));
        record.remove("_err");
        record.put("refused", new ArrayList<>(Guard.refused));
        record.put("unpinned", Pin.unpinned.get());
        lines.add(Json.write(record));
    }

    /** Classes whose static maps and collections are the original's loaded tables and registries. */
    static final String[] REGISTRIES = {
        "core.cache.def.impl.ItemDefinition", "core.cache.def.impl.NPCDefinition", "core.cache.def.impl.SceneryDefinition",
        "core.cache.def.impl.VarbitDefinition", "core.game.activity.ActivityManager", "core.game.component.ComponentDefinition",
        "core.game.dialogue.DialogueInterpreter", "core.game.interaction.InteractionListeners", "core.game.interaction.InterfaceListeners",
        "core.game.interaction.PluginInteractionManager", "core.game.node.entity.combat.equipment.RangeWeapon",
        "core.game.node.entity.npc.AbstractNPC", "core.game.node.entity.npc.NPCBehavior", "core.game.node.entity.player.link.music.MusicEntry",
        "core.game.node.entity.player.link.quest.QuestRepository", "core.game.node.item.GroundItemManager", "core.game.shops.Shops",
        "core.game.system.command.CommandMapping", "core.game.system.timer.TimerRegistry", "core.game.world.map.RegionManager",
        "core.game.worldevents.WorldEvents",
    };

    /**
     * Size of every static map and collection of the classes above and, where all keys are strings or
     * integers, a digest of the sorted keys. A class the start-up never initialised is reported, not initialised.
     */
    static Map<String, Object> registries() throws ReflectiveOperationException {
        Map<String, Object> out = new TreeMap<>();
        for (String name : REGISTRIES) {
            Class<?> owner = Class.forName(name, false, WorldTrace.class.getClassLoader());
            if (Pin.U.shouldBeInitialized(owner)) {
                out.put(name, "not initialised");
                continue;
            }
            Map<String, Object> fields = new TreeMap<>();
            for (Field field : owner.getDeclaredFields()) {
                if (!java.lang.reflect.Modifier.isStatic(field.getModifiers())) continue;
                field.setAccessible(true);
                Object value = field.get(null);
                if (value instanceof Collection) {
                    fields.put(field.getName(), java.util.Collections.singletonMap("size", ((Collection<?>) value).size()));
                } else if (value instanceof Map) {
                    Map<?, ?> map = (Map<?, ?>) value;
                    Map<String, Object> row = new LinkedHashMap<>();
                    row.put("size", map.size());
                    List<Object> keys;
                    try {
                        keys = new ArrayList<>(map.keySet());
                    } catch (RuntimeException e) {
                        keys = null;
                    }
                    if (keys != null && keys.stream().allMatch(k -> k instanceof String || k instanceof Integer || k instanceof Long)) {
                        List<String> sorted = keys.stream().map(String::valueOf).sorted().collect(Collectors.toList());
                        row.put("keysSha256", Json.sha256(String.join("\n", sorted)));
                    }
                    fields.put(field.getName(), row);
                }
            }
            out.put(name, fields);
        }
        return out;
    }

    static Map<String, Object> boot() throws Throwable {
        long before = errLines;
        // Server.main, without its networking, console reader and watchdog.
        ServerConfigParser.INSTANCE.parse("worldprops" + File.separator + "default.conf");
        Server.startTime = Pin.currentTimeMillis();
        // prompt(true) would also start the 600 ms worker thread; the state it sets is applied below instead.
        GameWorld.prompt(false);
        Field state = SystemManager.class.getDeclaredField("state");
        state.setAccessible(true);
        state.set(null, GameWorld.getSettings().isDevMode() ? SystemState.PRIVATE : SystemState.ACTIVE);

        Map<String, Object> out = new LinkedHashMap<>();
        Map<String, Object> listeners = new LinkedHashMap<>();
        listeners.put("login", names(GameWorld.getLoginListeners()));
        listeners.put("loginPlugins", names(LoginConfiguration.getLoginPlugins()));
        listeners.put("logout", names(GameWorld.getLogoutListeners()));
        listeners.put("persistPlayer", names((Collection<?>) staticField(Class.forName("core.game.node.entity.player.info.login.PlayerSaver"), "contentHooks")));
        listeners.put("persistWorld", names(GameWorld.getWorldPersists()));
        listeners.put("shutdown", names(GameWorld.getShutdownListeners()));
        listeners.put("startup", names(GameWorld.getStartupListeners()));
        listeners.put("tick", names(GameWorld.getTickListeners()));
        listeners.put("timers", names(TimerRegistry.INSTANCE.getAutoTimers()));
        out.put("listeners", listeners);
        // The class scan's own answers, asked again of the retained result: the order the original instantiated in.
        Map<String, Object> scan = new LinkedHashMap<>();
        scan.put("contentInterfaces", new ArrayList<Object>(ClassScanner.scanResults.getClassesImplementing("core.api.ContentInterface").getNames()));
        scan.put("initializable", new ArrayList<Object>(ClassScanner.scanResults.getClassesWithAnnotation("core.plugin.Initializable").getNames()));
        scan.put("timers", new ArrayList<Object>(ClassScanner.scanResults.getSubclasses("core.game.system.timer.RSTimer").getNames()));
        out.put("scan", scan);
        Map<String, Object> tables = new LinkedHashMap<>();
        tables.put("contentInterfaces", ClassScanner.INSTANCE.getAmountLoaded());
        tables.put("itemDefinitions", ItemDefinition.getDefinitions().size());
        tables.put("legacyPlugins", ClassScanner.INSTANCE.getNumPlugins());
        tables.put("npcDefinitions", NPCDefinition.getDefinitions().size());
        tables.put("sceneryDefinitions", SceneryDefinition.getDefinitions().size());
        tables.put("timerTypes", new ArrayList<Object>(new TreeSet<>(TimerRegistry.INSTANCE.getTimerMap().keySet())));
        List<Object> pulses = new ArrayList<>();
        for (Pulse pulse : GameWorld.getPulser().getCurrentPulses()) pulses.add(pulse.getClass().getName());
        tables.put("pulseClasses", pulses);
        out.put("tables", tables);
        out.put("systemState", String.valueOf(SystemManager.state()));
        List<Object> threads = new ArrayList<>();
        for (Thread thread : new TreeMap<>(Thread.getAllStackTraces().keySet().stream()
                .filter(t -> t != Thread.currentThread() && t.getThreadGroup() != null && !t.getThreadGroup().getName().equals("system")
                    && !t.getThreadGroup().getName().equals("InnocuousThreadGroup"))
                .collect(Collectors.toMap(t -> t.getName() + "#" + t.getId(), t -> t))).values()) {
            threads.add(thread.getName().replaceAll("\\d+$", "N") + (thread.isDaemon() ? " (daemon)" : ""));
        }
        java.util.Collections.sort((List) threads);
        out.put("threads", threads);
        out.put("registries", registries());
        out.put("_err", errLines - before);
        return out;
    }

    public static void main(String[] args) throws Throwable {
        world = Paths.get("").toAbsolutePath();
        JSONObject scenario = (JSONObject) new JSONParser().parse(new String(Files.readAllBytes(Paths.get(args[0])), StandardCharsets.UTF_8));
        System.setErr(new PrintStream(new CountingErr(System.err), true));
        System.setSecurityManager(new Guard(world.getParent().toFile()));
        Pin.installGenerators((Long) scenario.get("clockStart"), (Long) scenario.get("threadLocalSeed"));
        Handoffs.install();
        files = snapshot();

        Map<String, Object> header = new LinkedHashMap<>();
        header.put("t", "scenario");
        header.put("name", scenario.get("name"));
        header.put("clockStart", scenario.get("clockStart"));
        header.put("threadLocalSeed", scenario.get("threadLocalSeed"));
        header.put("tickMillis", Pin.TICK_MILLIS);
        lines.add(Json.write(header));

        record("boot", boot());
        for (Object item : (JSONArray) scenario.get("steps")) {
            JSONObject step = (JSONObject) item;
            String op = (String) step.get("op");
            Map<String, Object> extra = new LinkedHashMap<>();
            extra.put("op", op);
            if (op.equals("tick")) {
                // The original's worker sleeps out the rest of 600 ms after each tick.
                Pin.now += Pin.TICK_MILLIS;
                GameWorld.getMajorUpdateWorker().handleTickActions(false);
                record("tick", extra);
                continue;
            }
            String id = (String) step.get("session");
            extra.put("session", id);
            if (op.equals("connect")) {
                if (sessions.put(id, new TraceSession(id)) != null) throw new IllegalArgumentException("session reused: " + id);
            } else if (op.equals("recv")) {
                // IoEventHandler.read: the session's current producer reads what arrived.
                TraceSession session = sessions.get(id);
                extra.put("hex", step.get("hex"));
                session.getProducer().produceReader(session, ByteBuffer.wrap(Json.unhex((String) step.get("hex")))).run();
            } else if (op.equals("close")) {
                // IoEventHandler.read at end of stream.
                sessions.get(id).disconnect();
            } else {
                throw new IllegalArgumentException(op);
            }
            record("io", extra);
        }
        Map<String, Object> end = new LinkedHashMap<>();
        record("end", end);
        Files.write(Paths.get(args[1]), (String.join("\n", lines) + "\n").getBytes(StandardCharsets.UTF_8));
        System.out.flush();
        System.err.flush();
        // The original keeps non-daemon threads (GE worker); shutdown hooks are outside this trace.
        Runtime.getRuntime().halt(0);
    }
}
