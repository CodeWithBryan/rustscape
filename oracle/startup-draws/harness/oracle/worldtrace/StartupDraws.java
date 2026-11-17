package oracle.worldtrace;

import core.ServerConstants;
import core.ServerStore;
import core.api.PersistWorld;
import core.api.StartupListener;
import core.auth.Auth;
import core.cache.Cache;
import core.cache.def.impl.NPCDefinition;
import core.cache.def.impl.SceneryDefinition;
import core.game.node.entity.npc.NPC;
import core.game.system.SystemManager;
import core.game.system.SystemState;
import core.game.system.config.*;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import core.game.world.map.RegionChunk;
import core.game.world.repository.Repository;
import core.game.world.update.ChunkUpdateTracker;
import core.plugin.ClassScanner;
import core.tools.Log;
import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

/**
 * Who draws what during the original start-up. Runs the same statements as StartupPhases.startup (calling its own
 * phase and table methods, so its phase and table lines must equal fixtures/startup-loaders/oracle.jsonl line for
 * line), with the three shared generators replaced by counting generators that also record, for every next(bits)
 * call, the call site that drew. After each phase it reads, without creating anything, which NPCs the phase added
 * to Repository.npcs, the region cache size and key digest, the definition table sizes, GroundItemManager's list
 * size and ChunkUpdateTracker's dirty set size. Then it writes the boot record as WorldTrace does; the generator
 * accepts the run only if that record's digest equals the world-trace fixture's.
 *
 * Nothing here is game logic. The thread-local generator cannot be intercepted; its seed per phase is in the phase
 * lines.
 *
 * Usage (working directory = the world directory): StartupDraws OUT_JSONL CLOCK_START THREAD_LOCAL_SEED
 */
public final class StartupDraws {
    static String current = null;
    static final Map<String, Map<String, long[]>> draws = new LinkedHashMap<>();

    /**
     * Installed where the original reads its generator; every next(bits) is recorded and then made by the
     * Pin.Counting generator it wraps, which keeps the count and the state the phase lines report. Its own seed
     * is never used (seeded constructor: the JDK's seed uniquifier is not touched).
     */
    static final class Attributing extends java.util.Random {
        final String name;
        final Pin.Counting target;
        Attributing(String name, Pin.Counting target) { super(0L); this.name = name; this.target = target; }
        @Override protected int next(int bits) {
            if (current != null) attribute(name);
            return target.next(bits);
        }
    }

    static boolean library(String c) {
        return c.startsWith("java.") || c.startsWith("jdk.") || c.startsWith("sun.") || c.startsWith("kotlin.")
            || c.startsWith("oracle.worldtrace.");
    }

    static String frame(StackWalker.StackFrame f) {
        return f.getClassName() + "." + f.getMethodName() + ":" + f.getLineNumber();
    }

    static void attribute(String generator) {
        List<StackWalker.StackFrame> frames = StackWalker.getInstance().walk(s -> s.collect(Collectors.toList()));
        String api = null, via = null, site = null, inner = null, caller = null;
        List<String> game = new ArrayList<>();
        for (StackWalker.StackFrame f : frames) {
            String c = f.getClassName();
            if (api == null && (c.equals("java.util.Random") || c.equals("java.lang.Math") || c.equals("java.util.Collections"))
                && !f.getMethodName().equals("next")) api = frame(f);
            if (library(c)) continue;
            if (c.equals("core.tools.RandomFunction")) { if (via == null) via = frame(f); continue; }
            if (site == null) site = frame(f);
            if (inner == null && c.startsWith("content.")) inner = frame(f);
            // The first frame outside the shared utilities (core.tools, core.api.utils): who asked for the draw.
            if (caller == null && !c.startsWith("core.api.utils.")) caller = frame(f);
            game.add(frame(f));
        }
        // The outermost game frame is the phase's own entry point; the one inside it is what the phase called.
        String entry = game.isEmpty() ? null : game.get(game.size() - 1);
        String called = game.size() < 2 ? null : game.get(game.size() - 2);
        String key = generator + "|" + api + "|" + via + "|" + site + "|" + inner + "|" + entry + "|" + called + "|" + caller;
        draws.computeIfAbsent(current, k -> new LinkedHashMap<>()).computeIfAbsent(key, k -> new long[1])[0]++;
        // Repository.npcs.size() when the draw is made (a read of NodeList's own field).
        if (generator.equals("shared")) bySize.merge(Repository.getNpcs().size(), 1L, Long::sum);
    }

    static final List<String> out = new ArrayList<>();
    static int seenSlots = 0;
    static int sizeAtStart = 0;
    static java.util.Set<Integer> lastRegions = new java.util.HashSet<>();
    static java.util.Set<Integer> cacheScenery = new java.util.HashSet<>(), lastScenery = new java.util.HashSet<>(), lastNpcDefinitions = new java.util.HashSet<>();
    static java.util.Set<String> lastDirty = new java.util.TreeSet<>();
    static final java.util.TreeMap<Integer, Long> bySize = new java.util.TreeMap<>();

    static Object staticValue(Class<?> owner, Object instance, String name) throws ReflectiveOperationException {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(instance);
    }

    /** Read-only state after a phase. */
    @SuppressWarnings("unchecked")
    static void snapshot(String name) throws Throwable {
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("t", "phase");
        row.put("name", name);
        List<Object> rows = new ArrayList<>();
        Map<String, long[]> drawn = draws.getOrDefault(name, new LinkedHashMap<>());
        for (Map.Entry<String, long[]> e : drawn.entrySet()) {
            String[] k = e.getKey().split("\\|", -1);
            Map<String, Object> d = new LinkedHashMap<>();
            d.put("generator", k[0]);
            d.put("count", e.getValue()[0]);
            d.put("api", "null".equals(k[1]) ? null : k[1]);
            d.put("via", "null".equals(k[2]) ? null : k[2]);
            d.put("site", "null".equals(k[3]) ? null : k[3]);
            d.put("content", "null".equals(k[4]) ? null : k[4]);
            d.put("entry", "null".equals(k[5]) ? null : k[5]);
            d.put("called", "null".equals(k[6]) ? null : k[6]);
            d.put("caller", "null".equals(k[7]) ? null : k[7]);
            rows.add(d);
        }
        row.put("draws", rows);
        // NPCs this phase added: slot ranges, the NPCs whose class is not the base NPC, and runs of
        // [shared draws made before the NPC was added, number of consecutive NPCs with that many].
        List<Object> added = new ArrayList<>(), subclasses = new ArrayList<>(), runs = new ArrayList<>();
        java.util.TreeMap<String, java.util.TreeSet<Integer>> byClass = new java.util.TreeMap<>();
        java.util.TreeMap<String, Integer> classCounts = new java.util.TreeMap<>();
        int length = Repository.getNpcs().length();
        for (int slot = 1; slot < length; slot++) {
            NPC npc = Repository.getNpcs().get(slot);
            if (npc == null || slot <= seenSlots) continue;
            added.add(slot);
            if (npc.getClass() != NPC.class) {
                java.util.TreeSet<Integer> ids = byClass.computeIfAbsent(npc.getClass().getName(), k -> new java.util.TreeSet<>());
                ids.add(npc.getId());
                classCounts.merge(npc.getClass().getName(), 1, Integer::sum);
            }
        }
        if (!added.isEmpty()) seenSlots = (Integer) added.get(added.size() - 1);
        row.put("npcSlots", ranges(added));
        // NPCs whose runtime class is not the base NPC: class, number of NPCs, their distinct ids.
        for (Map.Entry<String, java.util.TreeSet<Integer>> e : byClass.entrySet()) subclasses.add(List.of(e.getKey(), classCounts.get(e.getKey()), new ArrayList<>(e.getValue())));
        row.put("npcSubclasses", subclasses);
        // Shared draws by Repository.npcs.size() at the time of the draw, for every size this phase passed
        // through, as runs [draws, consecutive sizes]: NPC.init draws (auto timers) before Repository.add, so
        // the draws at size s are those made before the NPC that takes the size to s + 1 was added.
        int sizeAtEnd = Repository.getNpcs().size();
        for (int s = sizeAtStart; s <= sizeAtEnd; s++) {
            long n = bySize.getOrDefault(s, 0L);
            if (!runs.isEmpty()) {
                @SuppressWarnings("unchecked") List<Object> last = (List<Object>) runs.get(runs.size() - 1);
                if ((Long) last.get(0) == n) { last.set(1, (Integer) last.get(1) + 1); continue; }
            }
            List<Object> run = new ArrayList<>();
            run.add(n);
            run.add(1);
            runs.add(run);
        }
        row.put("npcSizes", List.of(sizeAtStart, sizeAtEnd));
        row.put("sharedDrawsBySize", runs);
        bySize.clear();
        sizeAtStart = sizeAtEnd;
        Map<Integer, ?> regions = (Map<Integer, ?>) WorldTrace.staticField(core.game.world.map.RegionManager.class, "REGION_CACHE");
        row.put("regionCache", regions.size());
        row.put("regionKeysSha256", Json.sha256(String.join("\n", regions.keySet().stream().map(String::valueOf).sorted().collect(Collectors.toList()))));
        row.put("npcDefinitions", NPCDefinition.getDefinitions().size());
        row.put("npcDefinitionKeysSha256", Json.sha256(String.join("\n", NPCDefinition.getDefinitions().keySet().stream().map(String::valueOf).sorted().collect(Collectors.toList()))));
        row.put("sceneryDefinitions", SceneryDefinition.getDefinitions().size());
        row.put("groundItems", ((Collection<?>) WorldTrace.staticField(core.game.node.item.GroundItemManager.class, "GROUND_ITEMS")).size());
        Collection<RegionChunk> dirty = (Collection<RegionChunk>) staticValue(ChunkUpdateTracker.class, ChunkUpdateTracker.INSTANCE, "dirty");
        row.put("dirtyChunks", dirty.size());
        // The region ids and dirty chunk bases this phase added (sorted), when it added any.
        List<Integer> regionKeys = new ArrayList<>(new java.util.TreeSet<>(regions.keySet()));
        List<Integer> newRegions = new ArrayList<>(regionKeys);
        newRegions.removeAll(lastRegions);
        row.put("regionsAdded", newRegions);
        lastRegions = new java.util.HashSet<>(regionKeys);
        java.util.TreeSet<String> dirtyKeys = new java.util.TreeSet<>();
        for (RegionChunk chunk : dirty) {
            Location b = chunk.getCurrentBase();
            dirtyKeys.add(b.getX() + "," + b.getY() + "," + b.getZ());
        }
        List<String> newDirty = new ArrayList<>(dirtyKeys);
        newDirty.removeAll(lastDirty);
        row.put("dirtyChunksAdded", newDirty);
        lastDirty = dirtyKeys;
        row.put("renderableNpcs", Repository.getRenderableNpcs().size());
        row.put("dirtyListener", RegionChunk.dirtyListener.getClass().getName().replaceAll("\\$\\$Lambda\\$.*", "\\$\\$Lambda"));
        // Auth's two lateinit fields, read as fields (null before configure).
        Object authenticator = staticValue(Auth.class, Auth.INSTANCE, "authenticator");
        Object storage = staticValue(Auth.class, Auth.INSTANCE, "storageProvider");
        row.put("auth", List.of(authenticator == null ? "null" : authenticator.getClass().getName(), storage == null ? "null" : storage.getClass().getName()));
        // ServerStore's file map and counter, GraveController's active graves.
        Map<String, ?> store = (Map<String, ?>) staticValue(ServerStore.class, null, "fileMap");
        row.put("serverStore", List.of(staticValue(ServerStore.class, null, "counter"), new ArrayList<>(new java.util.TreeSet<>(store.keySet()))));
        Class<?> graves = Class.forName("core.game.node.entity.combat.graves.GraveController");
        row.put("activeGraves", ((Map<?, ?>) staticValue(graves, null, "activeGraves")).size());
        // The examine text every scenery definition has cached (the field, not getExamine, which would cache).
        Field examine = core.cache.def.Definition.class.getDeclaredField("examine");
        examine.setAccessible(true);
        // Also over the definitions Cache.init created only (the ones a start-up without content has).
        if (name.equals("Cache.init")) cacheScenery = new java.util.HashSet<>(SceneryDefinition.getDefinitions().keySet());
        StringBuilder text = new StringBuilder(), cacheText = new StringBuilder();
        int cached = 0, cacheCached = 0;
        for (Map.Entry<Integer, SceneryDefinition> e : new java.util.TreeMap<>(SceneryDefinition.getDefinitions()).entrySet()) {
            Object value = examine.get(e.getValue());
            if (value == null) continue;
            cached++;
            text.append(e.getKey()).append('\t').append(value).append('\n');
            if (cacheScenery.contains(e.getKey())) {
                cacheCached++;
                cacheText.append(e.getKey()).append('\t').append(value).append('\n');
            }
        }
        row.put("sceneryExamines", List.of(cached, Json.sha256(text.toString()), cacheCached, Json.sha256(cacheText.toString())));
        List<Integer> sceneryKeys = new ArrayList<>(new java.util.TreeSet<>(SceneryDefinition.getDefinitions().keySet()));
        List<Integer> newScenery = new ArrayList<>(sceneryKeys);
        newScenery.removeAll(lastScenery);
        row.put("sceneryDefinitionsAdded", name.equals("Cache.init") ? List.of() : newScenery);
        lastScenery = new java.util.HashSet<>(sceneryKeys);
        List<Integer> npcKeys = new ArrayList<>(new java.util.TreeSet<>(NPCDefinition.getDefinitions().keySet()));
        List<Integer> newNpcs = new ArrayList<>(npcKeys);
        newNpcs.removeAll(lastNpcDefinitions);
        row.put("npcDefinitionsAdded", name.equals("NPCConfigParser") ? List.of() : newNpcs);
        lastNpcDefinitions = new java.util.HashSet<>(npcKeys);
        out.add(Json.write(row));
    }

    static List<Object> ranges(List<Object> slots) {
        List<Object> r = new ArrayList<>();
        int start = -1, end = -1;
        for (Object o : slots) {
            int s = (Integer) o;
            if (start >= 0 && s == end + 1) { end = s; continue; }
            if (start >= 0) r.add(List.of(start, end));
            start = end = s;
        }
        if (start >= 0) r.add(List.of(start, end));
        return r;
    }

    static void step(String name, StartupPhases.Step body) throws Throwable {
        current = name;
        StartupPhases.phase(name, body);
        current = null;
        snapshot(name);
    }


    static void startup() throws Throwable {
        // The statements of StartupPhases.startup, in its order, with the same phase names and table dumps.
        step("ServerConfigParser.parse", () -> {
            core.game.system.config.ServerConfigParser.INSTANCE.parse("worldprops" + File.separator + "default.conf");
            core.Server.startTime = Pin.currentTimeMillis();
        });
        step("prompt.log", () -> core.api.ContentAPIKt.log(GameWorld.class, Log.FINE,
            "Prompting " + (GameWorld.getSettings() == null ? null : GameWorld.getSettings().getName()) + " Game World..."));
        step("Cache.init", () -> Cache.init(ServerConstants.CACHE_PATH));
        step("RegionChunk.dirtyListener", () -> RegionChunk.dirtyListener = ChunkUpdateTracker.INSTANCE);
        step("Auth.configure", () -> Auth.INSTANCE.configure());
        ConfigParser[] pre = new ConfigParser[1];
        step("ConfigParser()", () -> pre[0] = new ConfigParser());
        step("NPCConfigParser", () -> new NPCConfigParser().load());
        step("ItemConfigParser", () -> new ItemConfigParser().load());
        step("ObjectConfigParser", () -> new ObjectConfigParser().load());
        StartupPhases.table("ObjectConfigParser", StartupPhases.sceneryConfigs(StartupPhases.objectConfigIds(ServerConstants.Companion.getCONFIG_PATH() + "object_configs.json")));
        step("XteaParser", () -> new XteaParser().load());
        StartupPhases.table("XteaParser", StartupPhases.xteas());
        step("ObjectOverrideParser", () -> new ObjectOverrideParser().load());
        StartupPhases.table("ObjectOverrideParser", StartupPhases.regions());
        step("InterfaceConfigParser", () -> new InterfaceConfigParser().load());
        step("ClassScanner.scanClasspath", ClassScanner::scanClasspath);
        step("ClassScanner.loadPureInterfaces", ClassScanner::loadPureInterfaces);
        step("ClassScanner.loadTimers", () -> ClassScanner.INSTANCE.loadTimers());
        step("ServerStore.parse", () -> {
            ServerStore s = null;
            for (PersistWorld p : GameWorld.getWorldPersists()) if (p instanceof ServerStore) { s = (ServerStore) p; break; }
            if (s == null) throw new java.util.NoSuchElementException("List contains no element matching the predicate.");
            s.parse();
        });
        step("worldPersists.parse", () -> {
            List<PersistWorld> others = new ArrayList<>();
            for (PersistWorld p : GameWorld.getWorldPersists()) if (!(p instanceof ServerStore)) others.add(p);
            for (PersistWorld p : others) p.parse();
        });
        step("ClassScanner.loadSideEffectfulPlugins", ClassScanner::loadSideEffectfulPlugins);
        step("ShopParser", () -> new ShopParser().load());
        step("DropTableParser", () -> new DropTableParser().load());
        step("NPCSpawner", () -> new NPCSpawner().load());
        step("DoorConfigLoader", () -> new DoorConfigLoader().load());
        step("GroundSpawnLoader", () -> new GroundSpawnLoader().load());
        step("MusicConfigLoader", () -> new MusicConfigLoader().load());
        Map<String, Object> music = StartupPhases.regions();
        music.putAll(StartupPhases.songs());
        StartupPhases.table("MusicConfigLoader", music);
        step("RangedConfigLoader", () -> new RangedConfigLoader().load());
        StartupPhases.table("RangedConfigLoader", StartupPhases.ranged());
        step("CustomVarbitParser", () -> new CustomVarbitParser().load());
        step("ClueRewardParser", () -> new ClueRewardParser().load());
        StartupPhases.table("ClueRewardParser", StartupPhases.clues());
        for (StartupListener listener : GameWorld.getStartupListeners()) {
            step("startup:" + listener.getClass().getName(), listener::startup);
        }
        step("SceneryDefinition.examine", () -> {
            for (SceneryDefinition d : SceneryDefinition.getDefinitions().values()) d.getExamine();
        });
        if (ServerConstants.PRELOAD_MAP) throw new IllegalStateException("preload_map is on; the harness does not decompose it");
        step("System.gc", System::gc);
    }

    public static void main(String[] args) throws Throwable {
        WorldTrace.world = Paths.get("").toAbsolutePath();
        System.setErr(new PrintStream(new WorldTrace.CountingErr(System.err), true));
        System.setOut(new PrintStream(new StartupPhases.Tee(System.out), true, "UTF-8"));
        System.setSecurityManager(new Guard(WorldTrace.world.getParent().toFile()));
        Pin.installGenerators(Long.parseLong(args[1]), Long.parseLong(args[2]));
        // Nothing has drawn yet: replace the three counting generators by recording ones with the same seed.
        Pin.install(Class.forName("core.tools.RandomFunction").getDeclaredField("RANDOM"), new Attributing("shared", Pin.shared));
        Pin.install(Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator"), new Attributing("math", Pin.math));
        Pin.install(java.util.Collections.class.getDeclaredField("r"), new Attributing("shuffle", Pin.shuffle));
        Handoffs.install();
        WorldTrace.files = WorldTrace.snapshot();
        Map<String, Object> header = new LinkedHashMap<>();
        header.put("t", "scenario");
        header.put("name", "boot");
        header.put("clockStart", Long.parseLong(args[1]));
        header.put("threadLocalSeed", Long.parseLong(args[2]));
        header.put("tickMillis", Pin.TICK_MILLIS);
        WorldTrace.lines.add(Json.write(header));

        long before = WorldTrace.errLines;
        startup();
        Field state = SystemManager.class.getDeclaredField("state");
        state.setAccessible(true);
        state.set(null, GameWorld.getSettings().isDevMode() ? SystemState.PRIVATE : SystemState.ACTIVE);
        Map<String, Object> boot = BootRecord.after(before);
        WorldTrace.record("boot", boot);
        List<String> lines = new ArrayList<>();
        lines.add(WorldTrace.lines.get(1));
        lines.add("PHASES");
        lines.addAll(StartupPhases.out);
        lines.add("DRAWS");
        lines.addAll(out);
        Files.write(Paths.get(args[0]), (String.join("\n", lines) + "\n").getBytes(StandardCharsets.UTF_8));
        System.out.flush();
        System.err.flush();
        Runtime.getRuntime().halt(0);
    }
}
