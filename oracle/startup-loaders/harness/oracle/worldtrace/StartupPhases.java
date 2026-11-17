package oracle.worldtrace;

import core.Server;
import core.ServerConstants;
import core.ServerStore;
import core.api.PersistWorld;
import core.api.StartupListener;
import core.api.utils.WeightBasedTable;
import core.api.utils.WeightedItem;
import core.auth.Auth;
import core.cache.Cache;
import core.cache.def.impl.SceneryDefinition;
import core.game.node.entity.combat.equipment.Ammunition;
import core.game.node.entity.combat.equipment.RangeWeapon;
import core.game.node.entity.impl.Projectile;
import core.game.node.entity.player.link.music.MusicEntry;
import core.game.node.entity.player.link.music.MusicZone;
import core.game.node.scenery.Scenery;
import core.game.system.SystemManager;
import core.game.system.SystemState;
import core.game.system.config.*;
import core.game.world.GameWorld;
import core.game.world.map.Region;
import core.game.world.map.RegionChunk;
import core.game.world.map.zone.ZoneBorders;
import core.game.world.update.ChunkUpdateTracker;
import core.plugin.ClassScanner;
import core.tools.Log;
import core.game.world.update.flag.context.Graphics;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.parser.JSONParser;

/**
 * Runs the body of the original GameWorld.prompt(false) one call at a time, in its order, and records the
 * generator positions after every call, the tables the small configuration loaders leave behind, and then the
 * boot record exactly as WorldTrace would write it. The generator compares that record with the world-trace
 * fixture: if they are identical, running the calls one at a time did not change the start-up.
 *
 * After the boot record it runs selected loaders again over authored inputs (CONFIG_PATH pointed at a case
 * directory) and records what each left behind or which exception left it.
 *
 * Nothing here is game logic: every call below is an original method, called with the arguments prompt uses.
 *
 * Usage (working directory = the world directory): StartupPhases CASES_JSON OUT_JSONL
 */
public final class StartupPhases {
    static final List<String> out = new ArrayList<>();
    /** Standard output, kept per phase: the original's own log lines ("Parsed N ..."). */
    static final ByteArrayOutputStream stdout = new ByteArrayOutputStream();

    static final class Tee extends OutputStream {
        final OutputStream next;
        Tee(OutputStream next) { this.next = next; }
        @Override public void write(int b) throws java.io.IOException { stdout.write(b); next.write(b); }
        @Override public void flush() throws java.io.IOException { next.flush(); }
    }

    interface Step { void run() throws Throwable; }

    /** The original's log lines of one phase, without colour codes and the clock prefix. */
    static List<Object> logLines() {
        String text = new String(stdout.toByteArray(), StandardCharsets.UTF_8);
        stdout.reset();
        List<Object> lines = new ArrayList<>();
        for (String line : text.split("\n")) {
            String plain = line.replaceAll("\u001b\\[[0-9;]*m", "").replace("\r", "");
            int bracket = plain.indexOf("]: ");
            if (plain.startsWith("[") && bracket > 0) plain = plain.substring(bracket + 3);
            if (!plain.isEmpty()) lines.add(plain);
        }
        return lines;
    }

    static void phase(String name, Step step) throws Throwable {
        stdout.reset();
        long errBefore = WorldTrace.errLines;
        step.run();
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("t", "phase");
        row.put("name", name);
        row.put("generators", WorldTrace.generators());
        row.put("stderrLines", WorldTrace.errLines - errBefore);
        row.put("log", logLines());
        out.add(Json.write(row));
    }

    static void table(String loader, Map<String, Object> state) {
        Map<String, Object> row = new LinkedHashMap<>();
        row.put("t", "table");
        row.put("loader", loader);
        row.putAll(state);
        out.add(Json.write(row));
    }

    @SuppressWarnings("unchecked")
    static Map<Integer, Region> regionCache() throws ReflectiveOperationException {
        return (Map<Integer, Region>) WorldTrace.staticField(core.game.world.map.RegionManager.class, "REGION_CACHE");
    }

    static List<Object> list(Object... values) {
        List<Object> l = new ArrayList<>();
        for (Object v : values) l.add(v);
        return l;
    }

    // ---- what each loader leaves behind (read only: nothing below creates, draws or reads the clock) ----

    static Map<String, Object> xteas() {
        Map<String, Object> s = new LinkedHashMap<>();
        TreeMap<Integer, int[]> sorted = new TreeMap<>(XteaParser.Companion.getREGION_XTEA());
        List<Object> rows = new ArrayList<>();
        for (Map.Entry<Integer, int[]> e : sorted.entrySet()) {
            int[] k = e.getValue();
            rows.add(list(e.getKey(), k[0], k[1], k[2], k[3]));
        }
        s.put("size", XteaParser.Companion.getREGION_XTEA().size());
        s.put("keys", rows);
        return s;
    }

    /** Scenery definitions named by ids, as they stand: [id, present, examine handler, handler keys, animationId]. */
    static Map<String, Object> sceneryConfigs(List<Integer> ids) {
        Map<String, Object> s = new LinkedHashMap<>();
        List<Object> rows = new ArrayList<>();
        for (int id : new java.util.TreeSet<>(ids)) {
            SceneryDefinition def = SceneryDefinition.getDefinitions().get(id);
            if (def == null) { rows.add(list(id, false, null, null, null)); continue; }
            Object examine = def.getHandlers().get("examine");
            rows.add(list(id, true, examine == null ? null : String.valueOf(examine),
                new ArrayList<Object>(new java.util.TreeSet<>(def.getHandlers().keySet())), def.animationId));
        }
        s.put("definitions", SceneryDefinition.getDefinitions().size());
        s.put("rows", rows);
        return s;
    }

    static Object scenery(Scenery o) {
        return list(o.getId(), o.getLocation().getX(), o.getLocation().getY(), o.getLocation().getZ(), o.getType(), o.getRotation());
    }

    static Map<String, Object> borders(ZoneBorders b) {
        Map<String, Object> m = new LinkedHashMap<>();
        m.put("box", list(b.getSouthWestX(), b.getSouthWestY(), b.getNorthEastX(), b.getNorthEastY(), b.getPlane()));
        List<Object> ex = null;
        if (b.getExceptions() != null) {
            ex = new ArrayList<>();
            for (ZoneBorders e : b.getExceptions()) ex.add(borders(e));
        }
        m.put("exceptions", ex);
        return m;
    }

    /** Every cached region with overrides or music: [id, add list, remove list, music, zones]. */
    static Map<String, Object> regions() throws ReflectiveOperationException {
        Map<String, Object> s = new LinkedHashMap<>();
        Map<Integer, Region> cache = regionCache();
        List<Object> rows = new ArrayList<>();
        // Zones are shared between regions; each distinct zone object is numbered once, in first-seen order.
        java.util.IdentityHashMap<MusicZone, Integer> zoneIds = new java.util.IdentityHashMap<>();
        List<Object> zones = new ArrayList<>();
        for (int id : new java.util.TreeSet<>(cache.keySet())) {
            Region r = cache.get(id);
            if (r.getAddSceneries().isEmpty() && r.getRemoveSceneries().isEmpty() && r.getMusic() == -1 && r.getMusicZones().isEmpty()) continue;
            List<Object> add = new ArrayList<>(), remove = new ArrayList<>(), z = new ArrayList<>();
            for (Scenery o : r.getAddSceneries()) add.add(scenery(o));
            for (Scenery o : r.getRemoveSceneries()) remove.add(scenery(o));
            for (MusicZone zone : r.getMusicZones()) {
                Integer n = zoneIds.get(zone);
                if (n == null) {
                    n = zones.size();
                    zoneIds.put(zone, n);
                    Map<String, Object> m = new LinkedHashMap<>();
                    m.put("music", zone.getMusicId());
                    m.put("borders", borders(zone.getBorders()));
                    zones.add(m);
                }
                z.add(n);
            }
            rows.add(list(id, add, remove, r.getMusic(), z));
        }
        s.put("regionCache", cache.size());
        s.put("regionKeysSha256", Json.sha256(String.join("\n", cache.keySet().stream().map(String::valueOf).sorted().collect(java.util.stream.Collectors.toList()))));
        s.put("regions", rows);
        s.put("zones", zones);
        return s;
    }

    static Map<String, Object> songs() {
        Map<String, Object> s = new LinkedHashMap<>();
        List<Object> rows = new ArrayList<>();
        for (Map.Entry<Integer, MusicEntry> e : new TreeMap<>(MusicEntry.getSongs()).entrySet()) {
            MusicEntry m = e.getValue();
            rows.add(list(e.getKey(), m.getId(), m.getName(), m.getIndex()));
        }
        s.put("songs", rows);
        return s;
    }

    static Object graphics(Graphics g) {
        return g == null ? null : list(g.getId(), g.getHeight(), g.getDelay());
    }

    static Map<String, Object> ranged() {
        Map<String, Object> s = new LinkedHashMap<>();
        List<Object> ammo = new ArrayList<>();
        for (Map.Entry<Integer, Ammunition> e : new TreeMap<>(Ammunition.getAmmunition()).entrySet()) {
            Ammunition a = e.getValue();
            Projectile p = a.getProjectile();
            ammo.add(list(e.getKey(), a.getItemId(), graphics(a.getStartGraphics()), graphics(a.getDarkBowGraphics()),
                list(p.getProjectileId(), p.getStartHeight(), p.getEndHeight(), p.getStartDelay(), p.getSpeed(), p.getAngle(), p.getDistance()),
                a.getPoisonDamage(), a.getEffect() == null ? null : a.getEffect().name()));
        }
        List<Object> weapons = new ArrayList<>();
        for (Map.Entry<Integer, RangeWeapon> e : new TreeMap<>(RangeWeapon.getRangeWeapons()).entrySet()) {
            RangeWeapon w = e.getValue();
            weapons.add(list(e.getKey(), w.getItemId(), w.getAnimation().getId(), w.getAttackSpeed(), w.getAmmunitionSlot(), w.getType(),
                w.isDropAmmo(), new ArrayList<Object>(w.getAmmunition())));
        }
        s.put("ammunition", ammo);
        s.put("weapons", weapons);
        return s;
    }

    static Object weighted(WeightBasedTable t) {
        Map<String, Object> m = new LinkedHashMap<>();
        List<Object> items = new ArrayList<>(), guaranteed = new ArrayList<>();
        for (WeightedItem i : t) items.add(list(i.getId(), i.getMinAmt(), i.getMaxAmt(), Double.toString(i.getWeight()), i.getGuaranteed()));
        for (WeightedItem i : t.getGuaranteedItems()) guaranteed.add(list(i.getId(), i.getMinAmt(), i.getMaxAmt(), Double.toString(i.getWeight()), i.getGuaranteed()));
        m.put("items", items);
        m.put("totalWeight", Double.toString(t.getTotalWeight()));
        m.put("guaranteed", guaranteed);
        return m;
    }

    static Map<String, Object> clues() {
        Map<String, Object> s = new LinkedHashMap<>();
        s.put("easy", weighted(ClueRewardParser.getEasyTable()));
        s.put("medium", weighted(ClueRewardParser.getMedTable()));
        s.put("hard", weighted(ClueRewardParser.getHardTable()));
        s.put("rare", weighted(ClueRewardParser.getRareTable()));
        return s;
    }

    @SuppressWarnings("unchecked")
    static List<Integer> objectConfigIds(String path) throws Exception {
        List<Integer> ids = new ArrayList<>();
        try {
            JSONArray rows = (JSONArray) new JSONParser().parse(new String(Files.readAllBytes(Paths.get(path)), StandardCharsets.UTF_8));
            for (Object row : rows) {
                if (!(row instanceof JSONObject)) continue;
                for (String part : String.valueOf(((JSONObject) row).get("ids")).split(",")) {
                    try { ids.add(Integer.parseInt(part)); } catch (NumberFormatException e) { /* not an id */ }
                }
            }
        } catch (Exception e) {
            // An authored malformed file: the ids its case names explicitly are dumped instead.
        }
        return ids;
    }

    // ---- the start-up ----

    static void startup() throws Throwable {
        phase("ServerConfigParser.parse", () -> {
            core.game.system.config.ServerConfigParser.INSTANCE.parse("worldprops" + File.separator + "default.conf");
            Server.startTime = Pin.currentTimeMillis();
        });
        // GameWorld.prompt(run = false, directory = "server.properties"), statement by statement.
        phase("prompt.log", () -> core.api.ContentAPIKt.log(GameWorld.class, Log.FINE,
            "Prompting " + (GameWorld.getSettings() == null ? null : GameWorld.getSettings().getName()) + " Game World..."));
        phase("Cache.init", () -> Cache.init(ServerConstants.CACHE_PATH));
        phase("RegionChunk.dirtyListener", () -> RegionChunk.dirtyListener = ChunkUpdateTracker.INSTANCE);
        phase("Auth.configure", () -> Auth.INSTANCE.configure());
        // ConfigParser().prePlugin()
        ConfigParser[] pre = new ConfigParser[1];
        phase("ConfigParser()", () -> pre[0] = new ConfigParser());
        phase("NPCConfigParser", () -> new NPCConfigParser().load());
        phase("ItemConfigParser", () -> new ItemConfigParser().load());
        phase("ObjectConfigParser", () -> new ObjectConfigParser().load());
        table("ObjectConfigParser", sceneryConfigs(objectConfigIds(ServerConstants.Companion.getCONFIG_PATH() + "object_configs.json")));
        phase("XteaParser", () -> new XteaParser().load());
        table("XteaParser", xteas());
        phase("ObjectOverrideParser", () -> new ObjectOverrideParser().load());
        table("ObjectOverrideParser", regions());
        phase("InterfaceConfigParser", () -> new InterfaceConfigParser().load());
        phase("ClassScanner.scanClasspath", ClassScanner::scanClasspath);
        phase("ClassScanner.loadPureInterfaces", ClassScanner::loadPureInterfaces);
        phase("ClassScanner.loadTimers", () -> ClassScanner.INSTANCE.loadTimers());
        phase("ServerStore.parse", () -> {
            ServerStore s = null;
            for (PersistWorld p : GameWorld.getWorldPersists()) if (p instanceof ServerStore) { s = (ServerStore) p; break; }
            if (s == null) throw new java.util.NoSuchElementException("List contains no element matching the predicate.");
            s.parse();
        });
        phase("worldPersists.parse", () -> {
            List<PersistWorld> others = new ArrayList<>();
            for (PersistWorld p : GameWorld.getWorldPersists()) if (!(p instanceof ServerStore)) others.add(p);
            for (PersistWorld p : others) p.parse();
        });
        phase("ClassScanner.loadSideEffectfulPlugins", ClassScanner::loadSideEffectfulPlugins);
        // configParser.postPlugin(): GameWorld's own instance; ConfigParser holds no state.
        phase("ShopParser", () -> new ShopParser().load());
        phase("DropTableParser", () -> new DropTableParser().load());
        phase("NPCSpawner", () -> new NPCSpawner().load());
        phase("DoorConfigLoader", () -> new DoorConfigLoader().load());
        phase("GroundSpawnLoader", () -> new GroundSpawnLoader().load());
        phase("MusicConfigLoader", () -> new MusicConfigLoader().load());
        Map<String, Object> music = regions();
        music.putAll(songs());
        table("MusicConfigLoader", music);
        phase("RangedConfigLoader", () -> new RangedConfigLoader().load());
        table("RangedConfigLoader", ranged());
        phase("CustomVarbitParser", () -> new CustomVarbitParser().load());
        phase("ClueRewardParser", () -> new ClueRewardParser().load());
        table("ClueRewardParser", clues());
        for (StartupListener listener : GameWorld.getStartupListeners()) {
            phase("startup:" + listener.getClass().getName(), listener::startup);
        }
        phase("SceneryDefinition.examine", () -> {
            for (SceneryDefinition d : SceneryDefinition.getDefinitions().values()) d.getExamine();
        });
        if (ServerConstants.PRELOAD_MAP) throw new IllegalStateException("preload_map is on; the harness does not decompose it");
        phase("System.gc", System::gc);
    }

    // ---- authored inputs ----

    static void reset(String loader) throws ReflectiveOperationException {
        switch (loader) {
            case "XteaParser": XteaParser.Companion.getREGION_XTEA().clear(); break;
            case "RangedConfigLoader": Ammunition.getAmmunition().clear(); RangeWeapon.getRangeWeapons().clear(); break;
            case "ObjectOverrideParser": case "MusicConfigLoader":
                for (Region r : regionCache().values()) {
                    r.getAddSceneries().clear(); r.getRemoveSceneries().clear(); r.setMusic(-1); r.getMusicZones().clear();
                }
                break;
            default: break;
        }
    }

    static void load(String loader) throws Throwable {
        switch (loader) {
            case "XteaParser": new XteaParser().load(); break;
            case "ObjectConfigParser": new ObjectConfigParser().load(); break;
            case "ObjectOverrideParser": new ObjectOverrideParser().load(); break;
            case "ShopParser": new ShopParser().load(); break;
            case "MusicConfigLoader": new MusicConfigLoader().load(); break;
            case "RangedConfigLoader": new RangedConfigLoader().load(); break;
            case "ClueRewardParser": new ClueRewardParser().load(); break;
            default: throw new IllegalArgumentException(loader);
        }
    }

    @SuppressWarnings("unchecked")
    static void cases(Path file, Path base) throws Throwable {
        JSONArray cases = (JSONArray) new JSONParser().parse(new String(Files.readAllBytes(file), StandardCharsets.UTF_8));
        String original = ServerConstants.Companion.getCONFIG_PATH();
        for (Object item : cases) {
            JSONObject c = (JSONObject) item;
            String name = (String) c.get("name"), loader = (String) c.get("loader");
            Path dir = base.resolve(name);
            ServerConstants.Companion.setCONFIG_PATH(dir.toString() + File.separator);
            reset(loader);
            stdout.reset();
            long shared = Pin.shared.draws(), shuffle = Pin.shuffle.draws(), math = Pin.math.draws();
            String error = null;
            try {
                load(loader);
            } catch (Throwable t) {
                error = t.getClass().getName();
            }
            Map<String, Object> row = new LinkedHashMap<>();
            row.put("t", "case");
            row.put("name", name);
            row.put("loader", loader);
            row.put("error", error);
            row.put("draws", list(Pin.shared.draws() - shared, Pin.math.draws() - math, Pin.shuffle.draws() - shuffle));
            row.put("log", logLines());
            switch (loader) {
                case "XteaParser": row.put("state", xteas()); break;
                case "ObjectConfigParser": {
                    List<Integer> ids = new ArrayList<>();
                    for (Object id : (JSONArray) c.get("dump")) ids.add(((Long) id).intValue());
                    row.put("state", sceneryConfigs(ids));
                    break;
                }
                case "ObjectOverrideParser": row.put("state", regions()); break;
                case "MusicConfigLoader": row.put("state", regions()); break;
                case "RangedConfigLoader": row.put("state", ranged()); break;
                case "ClueRewardParser": row.put("state", clues()); break;
                default: row.put("state", null); break;
            }
            out.add(Json.write(row));
        }
        ServerConstants.Companion.setCONFIG_PATH(original);
    }

    public static void main(String[] args) throws Throwable {
        WorldTrace.world = Paths.get("").toAbsolutePath();
        System.setErr(new PrintStream(new WorldTrace.CountingErr(System.err), true));
        System.setOut(new PrintStream(new Tee(System.out), true, "UTF-8"));
        System.setSecurityManager(new Guard(WorldTrace.world.getParent().toFile()));
        // The boot scenario's inputs (fixtures/world-trace/trace.jsonl, first line).
        Pin.installGenerators(Long.parseLong(args[2]), Long.parseLong(args[3]));
        Handoffs.install();
        WorldTrace.files = WorldTrace.snapshot();
        Map<String, Object> header = new LinkedHashMap<>();
        header.put("t", "scenario");
        header.put("name", "boot");
        header.put("clockStart", Long.parseLong(args[2]));
        header.put("threadLocalSeed", Long.parseLong(args[3]));
        header.put("tickMillis", Pin.TICK_MILLIS);
        WorldTrace.lines.add(Json.write(header));

        long before = WorldTrace.errLines;
        startup();
        // WorldTrace.boot after its GameWorld.prompt(false) call, unchanged in substance.
        Field state = SystemManager.class.getDeclaredField("state");
        state.setAccessible(true);
        state.set(null, GameWorld.getSettings().isDevMode() ? SystemState.PRIVATE : SystemState.ACTIVE);
        Map<String, Object> boot = BootRecord.after(before);
        WorldTrace.record("boot", boot);
        out.add(0, WorldTrace.lines.get(1));

        cases(Paths.get(args[0]), Paths.get(args[0]).getParent().resolve("cases"));
        Files.write(Paths.get(args[1]), (String.join("\n", out) + "\n").getBytes(StandardCharsets.UTF_8));
        System.out.flush();
        System.err.flush();
        Runtime.getRuntime().halt(0);
    }
}
