package oracle;

import java.io.*;
import java.lang.reflect.*;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import content.global.skill.summoning.familiar.Familiar;
import core.cache.def.impl.SceneryDefinition;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.music.MusicZone;
import core.game.node.item.GroundItem;
import core.game.node.scenery.*;
import core.game.system.task.Pulse;
import core.game.world.map.*;
import core.game.world.map.build.*;
import core.game.world.map.zone.*;
import core.game.system.config.XteaParser;
import core.net.packet.PacketRepository;
import core.net.packet.context.*;
import core.net.packet.out.*;

/**
 * Drives the unchanged DynamicRegion, Region, RegionChunk, RegionManager,
 * ChunkFlags, Landscape/MapscapeParser, Scenery/SceneryBuilder and scene
 * packet classes over real cache regions and records what they do.
 */
public class Support {
    static Map cache;
    static Map<String, Region> regions;
    static Map<String, ZoneBorders> bordersByName;
    static Map<String, Entity> entities;
    static Map<Pulse, Region> pulseOwners;
    static List<Pulse> queue;
    static List<Object> events;
    static List<String> frames;
    static Map<String, int[]> failures;
    static int copies;
    static final TreeSet<String> missing = new TreeSet<>();
    static final boolean DISCOVER = Boolean.getBoolean("oracle.discover");
    public static core.net.IoSession session;

    static int n(Object v) { return ((Number) v).intValue(); }
    static boolean b(Map m, String k) { return Boolean.TRUE.equals(m.get(k)); }
    static Location loc(Object v) { List l = (List) v; return new Location(n(l.get(0)), n(l.get(1)), n(l.get(2))); }
    public static Object coords(Location l) { return l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()); }
    static byte[] bytes(String s) { byte[] b = new byte[s.length() / 2]; for (int i = 0; i < b.length; i++) b[i] = (byte) Integer.parseInt(s.substring(i * 2, i * 2 + 2), 16); return b; }
    static String hex(byte[] v) { StringBuilder s = new StringBuilder(); for (byte x : v) s.append(String.format("%02x", x & 255)); return s.toString(); }
    static Map<String, Object> map(Object... v) { Map<String, Object> m = new TreeMap<>(); for (int i = 0; i < v.length; i += 2) m.put((String) v[i], v[i + 1]); return m; }

    public static void emit(String name, Object... args) { List<Object> e = new ArrayList<>(); e.add(name); Collections.addAll(e, args); events.add(e); }
    public static void frame(byte[] bytes) { frames.add(hex(bytes)); }
    public static void submit(Pulse p) { queue.add(p); }
    public static String copyName() { return "copy" + (++copies); }
    /** An injected failure: the named service throws on its counted call. */
    public static void fail(String service) {
        int[] f = failures.get(service);
        if (f != null && --f[0] == 0) throw new RuntimeException("injected " + service);
    }

    static Map cacheRegion(int id) { return (Map) ((Map) cache.get("regions")).get(Integer.toString(id)); }
    public static int archive(String s) {
        int split = s.indexOf('_');
        int x = Integer.parseInt(s.substring(1, split)), y = Integer.parseInt(s.substring(split + 1));
        Map r = cacheRegion(x << 8 | y);
        // Region.load swallows exceptions, so an unsupplied region is recorded and made fatal after the action.
        if (r == null) { missing.add(s); return -1; }
        Object g = r.get(s.charAt(0) == 'm' ? "mapGroup" : "landGroup");
        return n(g);
    }
    static byte[] cacheBytes(int group, String kind) {
        for (Object v : ((Map) cache.get("regions")).values()) {
            Map r = (Map) v;
            if (n(r.get(kind + "Group")) == group) { String s = (String) r.get(kind); return s == null ? null : bytes(s); }
        }
        throw new AssertionError("Unsupplied cache group " + group);
    }
    public static byte[] mapBytes(int group) { return cacheBytes(group, "map"); }
    public static byte[] landBytes(int group) { return cacheBytes(group, "land"); }

    static Region region(Object ref) {
        if (ref instanceof Number) return RegionManager.forId(n(ref));
        Region r = regions.get((String) ref);
        if (r == null) throw new AssertionError("unknown region " + ref);
        return r;
    }
    static ZoneBorders borders(Object ref) {
        if (ref instanceof String) return bordersByName.get(ref);
        List l = (List) ref;
        return new ZoneBorders(n(l.get(0)), n(l.get(1)), n(l.get(2)), n(l.get(3)));
    }
    static Object bordersValue(ZoneBorders b) { return b == null ? null : Arrays.asList(b.getSouthWestX(), b.getSouthWestY(), b.getNorthEastX(), b.getNorthEastY()); }
    static RegionChunk chunk(Map a) { List c = (List) a.get("chunk"); return region(a.get("region")).getChunks()[n(c.get(0))][n(c.get(1))][n(c.get(2))]; }

    static String scenery(Scenery o) {
        Location l = o.getLocation();
        return o.getId() + "," + o.getType() + "," + o.getRotation() + "," + l.getX() + "," + l.getY() + "," + l.getZ() + "," + o.isActive() + "," + o.isRenderable() + "," + (o instanceof Constructed) + "," + o.getCharge();
    }
    static String landscape(ChunkFlags f) {
        boolean[][] a = f.getLandscape();
        if (a == null) return "null";
        StringBuilder s = new StringBuilder();
        for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) s.append(a[x][y] ? '1' : '0');
        return s.toString();
    }
    static String names(List<? extends Entity> list) {
        StringBuilder s = new StringBuilder("[");
        for (Entity e : list) { if (s.length() > 1) s.append(','); s.append(e == null ? "null" : e.getName()); }
        return s.append(']').toString();
    }
    static Object field(Object o, Class<?> c, String name) throws Exception { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(o); }

    /** The canonical text of every stored object and chunk; hashed in the snapshot. */
    static void describe(StringBuilder storage, StringBuilder chunks, int id, Region r) throws Exception {
        for (int cx = 0; cx < 8; cx++) for (int cy = 0; cy < 8; cy++) for (int z = 0; z < 4; z++) {
            RegionChunk c = r.getChunks()[cx][cy][z];
            if (c == null) { chunks.append("C ").append(id).append(' ').append(cx).append(' ').append(cy).append(' ').append(z).append(" null\n"); continue; }
            Location base = c.getBase(), current = c.getCurrentBase();
            List items = (List) field(c, RegionChunk.class, "items");
            List updates = (List) field(c, RegionChunk.class, "updateFlags");
            chunks.append("C ").append(id).append(' ').append(cx).append(' ').append(cy).append(' ').append(z)
                .append(' ').append(base.getX()).append(' ').append(base.getY()).append(' ').append(base.getZ())
                .append(' ').append(current.getX()).append(' ').append(current.getY()).append(' ').append(current.getZ())
                .append(' ').append(c.getRotation()).append(' ').append(names(c.getPlayers())).append(' ').append(names(c.getNpcs()))
                .append(' ').append(items == null ? "null" : Integer.toString(items.size())).append(' ').append(updates.size())
                .append(' ').append(landscape(c.getFlags())).append(' ').append(landscape(c.getProjectileFlags())).append('\n');
            for (int stat = 0; stat < 2; stat++) {
                Scenery[][][] a = stat == 0 ? c.getStatObjects() : c.getObjects();
                for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) for (int s = 0; s < 4; s++) {
                    Scenery o = a[x][y][s];
                    if (o != null) storage.append("O ").append(id).append(' ').append(cx).append(' ').append(cy).append(' ').append(z).append(' ')
                        .append(stat).append(' ').append(x).append(' ').append(y).append(' ').append(s).append(' ').append(scenery(o)).append('\n');
                }
            }
        }
    }
    static String sha(String text) throws Exception { return hex(MessageDigest.getInstance("SHA-256").digest(text.getBytes(StandardCharsets.UTF_8))); }

    static Object snapshot(String label) throws Exception {
        TreeMap<Integer, Region> all = new TreeMap<>(RegionManager.getRegionCache());
        List<Object> rs = new ArrayList<>(), instances = new ArrayList<>();
        StringBuilder storage = new StringBuilder(), chunks = new StringBuilder();
        for (int id : all.keySet()) {
            Region r = all.get(id);
            boolean dyn = r instanceof DynamicRegion;
            rs.add(Arrays.asList(id, dyn, r.getRegionId(), r.getX(), r.getY(), r.isLoaded(), r.isBuild(), r.isHasFlags(), r.getObjectCount(), r.isActive(), new ArrayList<>(r.getViewingPlayers())));
            describe(storage, chunks, id, r);
            if (dyn) {
                DynamicRegion d = (DynamicRegion) r;
                List<Object> linked = new ArrayList<>();
                DynamicRegion[] l = (DynamicRegion[]) field(d, DynamicRegion.class, "linked");
                if (l != null) for (DynamicRegion x : l) linked.add(x.getId());
                List<Object> zones = new ArrayList<>(), music = new ArrayList<>(), npcs = new ArrayList<>();
                for (RegionZone z : r.getRegionZones()) zones.add(Arrays.asList(z.getZone().getName(), bordersValue(z.getBorders())));
                for (MusicZone m : r.getMusicZones()) music.add(Arrays.asList(m.getMusicId(), bordersValue(m.getBorders())));
                for (NPC x : d.NPCs) npcs.add(x.getName());
                instances.add(Arrays.asList(id, bordersValue(d.getBorders()), d.isMulticombat(), d.isPermanent(), d.getParent() == null ? null : d.getParent().getId(),
                    l == null ? null : linked, zones, music, npcs));
            }
        }
        MessageDigest hash = MessageDigest.getInstance("SHA-256");
        TreeSet<Integer> keys = new TreeSet<>();
        keys.addAll(RegionManager.getCLIPPING_FLAGS().keySet());
        keys.addAll(RegionManager.getPROJECTILE_FLAGS().keySet());
        for (int k : keys) for (int p = 0; p < 2; p++) {
            Integer[] a = (p == 1 ? RegionManager.getPROJECTILE_FLAGS() : RegionManager.getCLIPPING_FLAGS()).get(k);
            if (a == null) continue;
            hash.update(ByteBuffer.allocate(5).putInt(k).put((byte) p).array());
            for (int i : a) hash.update(ByteBuffer.allocate(4).putInt(i).array());
        }
        List<Object> reserved = new ArrayList<>();
        for (ZoneBorders z : (List<ZoneBorders>) field(null, DynamicRegion.class, "RESERVED_AREAS")) reserved.add(bordersValue(z));
        List<Object> pulses = new ArrayList<>();
        Field passed = Pulse.class.getDeclaredField("ticksPassed");
        passed.setAccessible(true);
        for (Pulse p : queue) pulses.add(Arrays.asList(pulseOwners.containsKey(p) ? pulseOwners.get(p).getId() : null, p.getDelay(), passed.get(p), p.isRunning()));
        List<Object> players = new ArrayList<>();
        for (String name : new TreeSet<>(entities.keySet())) {
            Entity e = entities.get(name);
            if (e instanceof Player) players.add(Arrays.asList(name, coords(e.getLocation()), coords(((Player) e).lastScene)));
        }
        String dump = System.getenv("DYNAMIC_REGIONS_DUMP");
        if (dump != null) {
            try (Writer w = new OutputStreamWriter(new FileOutputStream(new File(dump, label + ".txt")), StandardCharsets.UTF_8)) { w.write(chunks.toString()); w.write(storage.toString()); }
        }
        return map("regions", rs, "instances", instances, "storage", sha(storage.toString()), "objects", storage.length() == 0 ? 0 : storage.toString().split("\n").length,
            "chunks", sha(chunks.toString()), "flags", hex(hash.digest()), "flagKeys", RegionManager.getCLIPPING_FLAGS().size() + RegionManager.getPROJECTILE_FLAGS().size(),
            "reserved", reserved, "queue", pulses, "players", players, "events", new ArrayList<>(events), "frames", new ArrayList<>(frames));
    }

    static void tick() {
        int count = queue.size();
        for (int i = 0; i < count; i++) {
            Pulse p = queue.remove(0);
            if (!p.update() && p.isRunning()) queue.add(p);
        }
    }

    static Object act(Map a) throws Exception {
        String op = (String) a.get("op");
        switch (op) {
            case "create": { DynamicRegion d = DynamicRegion.create(n(a.get("template"))); regions.put((String) a.get("as"), d); return d.getId(); }
            case "createBetween": { DynamicRegion d = DynamicRegion.create(n(a.get("one")), n(a.get("two"))); regions.put((String) a.get("as"), d); return d.getId(); }
            case "createArea": {
                DynamicRegion[] all = DynamicRegion.create(borders(a.get("borders")));
                List names = (List) a.get("as");
                List<Object> ids = new ArrayList<>();
                for (int i = 0; i < all.length; i++) { if (i < names.size()) regions.put((String) names.get(i), all[i]); ids.add(all[i].getId()); }
                return ids;
            }
            case "copy": { DynamicRegion d = DynamicRegion.copy(n(a.get("template")), loc(a.get("to"))); regions.put((String) a.get("as"), d); return d.getId(); }
            case "reserve": { List s = (List) a.get("size"); ZoneBorders z = DynamicRegion.reserveArea(n(s.get(0)), n(s.get(1))); if (a.get("as") != null) bordersByName.put((String) a.get("as"), z); return bordersValue(z); }
            case "find": { List s = (List) a.get("size"); return bordersValue(DynamicRegion.findZoneBorders(n(s.get(0)), n(s.get(1)))); }
            case "withBorders": { DynamicRegion d = new DynamicRegion(borders(a.get("borders"))); regions.put((String) a.get("as"), d); return d.getId(); }
            case "emptyInstance": {
                // HouseManager.getPreparedRegion.
                ZoneBorders z = DynamicRegion.reserveArea(Region.CHUNKS_SIZE, Region.CHUNKS_SIZE);
                DynamicRegion d = new DynamicRegion(-1, z.getSouthWestX() >> 6, z.getSouthWestY() >> 6);
                d.setBorders(z);
                d.setBuild(b(a, "build"));
                RegionManager.addRegion(d.getId(), d);
                regions.put((String) a.get("as"), d);
                return d.getId();
            }
            case "load": Region.load(region(a.get("region")), b(a, "build")); return null;
            case "replace": {
                Region from = region(a.get("from"));
                List c = (List) a.get("source");
                RegionChunk copy = from.getChunks()[n(c.get(0))][n(c.get(1))][n(c.get(2))].copy();
                ((DynamicRegion) region(a.get("region"))).replaceChunk(n(a.get("z")), n(a.get("x")), n(a.get("y")), copy, from);
                return null;
            }
            case "rotate": chunk(a).rotate(Direction.valueOf((String) a.get("direction"))); return null;
            case "link": {
                List l = (List) a.get("linked");
                DynamicRegion[] linked = new DynamicRegion[l.size()];
                for (int i = 0; i < linked.length; i++) linked[i] = (DynamicRegion) region(l.get(i));
                ((DynamicRegion) region(a.get("region"))).link(linked);
                return null;
            }
            case "multicombat": ((DynamicRegion) region(a.get("region"))).toggleMulticombat(); return null;
            case "music": ((DynamicRegion) region(a.get("region"))).setMusicId(n(a.get("music"))); return null;
            case "permanent": ((DynamicRegion) region(a.get("region"))).setPermanent(b(a, "value")); return null;
            case "npc": {
                Location l = loc(a.get("location"));
                String name = (String) a.get("name");
                NPC npc = a.get("owner") != null
                    ? new Familiar(name, n(a.get("id")), l, (Player) entities.get(a.get("owner")))
                    : new NPC(name, n(a.get("id")), l, a.get("direction") == null ? null : Direction.valueOf((String) a.get("direction")));
                if (b(a, "aggressive")) npc.setAggressive(true);
                if (b(a, "respawn")) npc.setRespawn(true);
                if (b(a, "walks")) npc.setWalks(true);
                if (a.containsKey("active")) npc.setActive(b(a, "active"));
                entities.put(name, npc);
                RegionManager.move(npc, null, l);
                return null;
            }
            case "player": {
                Location l = loc(a.get("location"));
                Player p = new Player((String) a.get("name"), l);
                entities.put(p.getName(), p);
                RegionManager.move(p, null, l);
                return null;
            }
            case "move": {
                Entity e = entities.get(a.get("name"));
                Location to = loc(a.get("to"));
                RegionManager.move(e, e.getLocation(), to);
                Field f = core.game.node.Node.class.getDeclaredField("location");
                f.setAccessible(true);
                f.set(e, to);
                return null;
            }
            case "leave": { Player p = (Player) entities.get(a.get("name")); p.getLocation().getChunk().removePlayer(p); return null; }
            case "view": {
                Region r = region(a.get("region"));
                return b(a, "add") ? r.incrementViewAmount((String) a.get("name")) : r.decrementViewAmount((String) a.get("name"));
            }
            case "checkInactive": return region(a.get("region")).checkInactive();
            case "isInactive": return region(a.get("region")).isInactive(b(a, "run"));
            case "flagActive": region(a.get("region")).flagActive(); return null;
            case "flagInactive": return a.containsKey("force") ? region(a.get("region")).flagInactive(b(a, "force")) : region(a.get("region")).flagInactive();
            case "unload": return region(a.get("region")).unload(b(a, "force"));
            case "timeout": region(a.get("region")).setRegionTimeOut(n(a.get("ticks"))); return null;
            case "tick": for (int i = 0; i < n(a.get("count")); i++) tick(); return queue.size();
            case "removeRegion": { Region r = region(a.get("region")); RegionManager.removeRegion(r.getId()); return null; }
            case "scene": ((Player) entities.get(a.get("name"))).updateSceneGraph(b(a, "login")); return null;
            case "minimap": PacketRepository.send(MinimapState.class, new MinimapStateContext((Player) entities.get(a.get("name")), n(a.get("state")))); return null;
            case "clearFlag": PacketRepository.send(ClearMinimapFlag.class, new PlayerContext((Player) entities.get(a.get("name")))); return null;
            case "track": ((DynamicRegion) region(a.get("region"))).NPCs.add((NPC) entities.get(a.get("name"))); return null;
            case "clear": ((DynamicRegion) region(a.get("region"))).clear(); return null;
            case "item": chunk(a).getItems().add(new GroundItem((String) a.get("label"))); return null;
            case "nullNpc": chunk(a).add((NPC) null); return null;
            case "object": { Location l = loc(a.get("location")); Scenery o = RegionManager.getObject(l.getX(), l.getY(), l.getZ()); return o == null ? null : scenery(o); }
            case "clip": { Location l = loc(a.get("location")); return Arrays.asList(RegionManager.getClippingFlag(l.getZ(), l.getX(), l.getY()), RegionManager.getProjectileFlag(l.getZ(), l.getX(), l.getY()), RegionManager.isLandscape(l)); }
            case "fail": failures.put((String) a.get("service"), new int[] { n(a.get("at")) }); return null;
            default: throw new AssertionError(a);
        }
    }

    static String error(Throwable t) {
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof ClassCastException) return "ClassCast";
        if (t instanceof ConcurrentModificationException) return "ConcurrentModification";
        if (t instanceof ArrayIndexOutOfBoundsException) return "Index";
        if (t instanceof IllegalStateException) return "IllegalState";
        if (t instanceof RuntimeException && t.getMessage() != null && t.getMessage().startsWith("injected ")) return "Failure";
        throw new AssertionError("unexpected original exception", t);
    }

    static void setup() throws Exception {
        regions = new TreeMap<>(); bordersByName = new TreeMap<>(); entities = new TreeMap<>();
        pulseOwners = new IdentityHashMap<>(); queue = new ArrayList<>(); events = new ArrayList<>(); frames = new ArrayList<>();
        failures = new HashMap<>(); copies = 0; session = new core.net.IoSession();
        ((Map) RegionManager.getRegionCache()).clear();
        RegionManager.getCLIPPING_FLAGS().clear();
        RegionManager.getPROJECTILE_FLAGS().clear();
        ((List) field(null, DynamicRegion.class, "RESERVED_AREAS")).clear();
        SceneryDefinition.getDefinitions().clear();
        Map defs = (Map) cache.get("definitions");
        for (Object id : new TreeSet<Object>(defs.keySet())) {
            int k = Integer.parseInt((String) id);
            SceneryDefinition.getDefinitions().put(k, SceneryDefinition.parseDefinition(k, ByteBuffer.wrap(bytes((String) defs.get(id)))));
        }
        XteaParser.Companion.getREGION_XTEA().clear();
        Map keys = (Map) cache.get("keys");
        for (Object id : keys.keySet()) {
            List v = (List) keys.get(id);
            XteaParser.Companion.getREGION_XTEA().put(Integer.parseInt((String) id), new int[] { n(v.get(0)), n(v.get(1)), n(v.get(2)), n(v.get(3)) });
        }
    }

    public static void main(String[] args) throws Throwable {
        cache = (Map) new JSONParser().parse(new InputStreamReader(new FileInputStream(args[0]), StandardCharsets.UTF_8));
        PrintStream err = System.err;
        System.setErr(new PrintStream(new ByteArrayOutputStream(), true) {
            @Override public void println(Object x) {
                String s = String.valueOf(x);
                if (events != null && !s.startsWith("\t") && !s.startsWith("Caused by")) emit("stackTrace");
            }
        });
        try (BufferedReader r = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
             BufferedWriter w = new BufferedWriter(new OutputStreamWriter(System.out, StandardCharsets.UTF_8))) {
            String line;
            int index = 0;
            while ((line = r.readLine()) != null) {
                JSONObject c = (JSONObject) new JSONParser().parse(line);
                setup();
                List<Object> out = new ArrayList<>();
                int step = 0;
                for (Object raw : (List) c.get("actions")) {
                    Map a = (Map) raw;
                    events.clear(); frames.clear();
                    Map<String, Object> result = new TreeMap<>();
                    try {
                        result.put("value", act(a));
                    } catch (Throwable t) {
                        result.put("error", error(t));
                    }
                    if (!missing.isEmpty() && !DISCOVER) throw new Error("Unsupplied cache regions " + missing);
                    for (Pulse p : queue) if (!pulseOwners.containsKey(p)) for (Region x : RegionManager.getRegionCache().values()) if (field(x, Region.class, "activityPulse") == p) pulseOwners.put(p, x);
                    Map<String, Object> s = (Map<String, Object>) snapshot(index + "-" + step);
                    s.put("result", result);
                    out.add(s);
                    step++;
                }
                Map<String, Object> expected = new TreeMap<>();
                expected.put("steps", out);
                c.put("expected", expected);
                if (!DISCOVER) { JSONValue.writeJSONString(c, w); w.newLine(); }
                w.flush();
                index++;
            }
            if (DISCOVER) { JSONValue.writeJSONString(new ArrayList<>(missing), w); w.newLine(); w.flush(); }
        } finally {
            System.setErr(err);
        }
    }
}
