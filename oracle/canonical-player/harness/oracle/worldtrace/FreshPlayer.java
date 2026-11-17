package oracle.worldtrace;

import core.ServerConstants;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.info.login.PlayerSaveParser;
import core.game.node.entity.player.info.login.PlayerSaver;
import core.game.node.entity.player.link.music.MusicEntry;
import core.game.node.entity.player.link.quest.Quest;
import core.game.node.entity.player.link.quest.QuestRepository;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import core.game.world.GameWorld;
import java.io.File;
import java.io.PrintStream;
import java.lang.reflect.Array;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
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
import org.json.simple.JSONObject;

/**
 * Boots the original world exactly as WorldTrace does (its boot record must be
 * byte-identical to the world-trace fixture's), then constructs players with
 * the original constructor and records, through reflection over the original
 * classes' own fields, what a freshly constructed Player holds; what
 * PlayerSaver.populate writes for it (each content hook's own output is
 * recorded separately); and the same after PlayerSaveParser.parse and
 * runContentHooks load each given character file into a new Player.
 *
 * Usage (working directory = a world directory): FreshPlayer CLOCK SEED NAME OUT [SAVE_NAME SAVE_HEX]...
 */
public final class FreshPlayer {
    static final List<String> out = new ArrayList<>();

    /** One value as the record holds it. Doubles and floats are their raw bits so that -0.0 and NaN survive. */
    static Object value(Object v) {
        if (v == null) return null;
        if (v instanceof Boolean || v instanceof Integer || v instanceof Long || v instanceof String) return v;
        if (v instanceof Short || v instanceof Byte) return ((Number) v).intValue();
        if (v instanceof Character) return (int) (Character) v;
        if (v instanceof Double) return String.format("d:%016x", Double.doubleToRawLongBits((Double) v));
        if (v instanceof Float) return String.format("f:%08x", Float.floatToRawIntBits((Float) v));
        if (v instanceof Enum) return "enum:" + ((Enum<?>) v).name();
        if (v instanceof Location) {
            Location l = (Location) v;
            return "Location(" + l.getX() + "," + l.getY() + "," + l.getZ() + ")";
        }
        if (v instanceof Animation) {
            Animation a = (Animation) v;
            return "Animation(" + a.getId() + "," + a.getDelay() + "," + (a.getPriority() == null ? "null" : a.getPriority().name()) + ")";
        }
        if (v instanceof Graphics) {
            Graphics g = (Graphics) v;
            return "Graphics(" + g.getId() + "," + g.getHeight() + "," + g.getDelay() + ")";
        }
        if (v.getClass().isArray()) {
            int length = Array.getLength(v);
            Class<?> component = v.getClass().getComponentType();
            if (component.isPrimitive()) {
                List<Object> list = new ArrayList<>();
                for (int i = 0; i < length; i++) list.add(value(Array.get(v, i)));
                return list;
            }
            return "array:" + component.getName() + "[" + length + "]";
        }
        if (v instanceof Collection) return "collection:" + v.getClass().getName() + "[" + ((Collection<?>) v).size() + "]";
        if (v instanceof Map) return "map:" + v.getClass().getName() + "[" + ((Map<?, ?>) v).size() + "]";
        return "object:" + v.getClass().getName();
    }

    /** Every instance field the class itself declares, by name. */
    static Map<String, Object> fields(Object owner, Class<?> type) throws IllegalAccessException {
        Map<String, Object> outFields = new TreeMap<>();
        for (Field field : type.getDeclaredFields()) {
            if (Modifier.isStatic(field.getModifiers())) continue;
            field.setAccessible(true);
            outFields.put(field.getName(), value(field.get(owner)));
        }
        return outFields;
    }

    static Map<String, Object> dump(Player player) throws Exception {
        Map<String, Object> d = new TreeMap<>();
        d.put("Node", fields(player, Node.class));
        d.put("Entity", fields(player, Entity.class));
        d.put("Player", fields(player, Player.class));
        d.put("Properties", fields(player.getProperties(), player.getProperties().getClass()));
        d.put("Skills", fields(player.getSkills(), player.getSkills().getClass()));
        d.put("PlayerDetails", fields(player.getDetails(), PlayerDetails.class));
        d.put("UserAccountInfo", fields(player.getDetails().accountInfo, player.getDetails().accountInfo.getClass()));
        d.put("UIDInfo", fields(player.getDetails().getInfo(), player.getDetails().getInfo().getClass()));
        d.put("CommunicationInfo", fields(player.getDetails().getCommunication(), player.getDetails().getCommunication().getClass()));
        d.put("SpellBookManager", fields(player.getSpellBookManager(), player.getSpellBookManager().getClass()));
        d.put("FamiliarManager", fields(player.getFamiliarManager(), player.getFamiliarManager().getClass()));
        d.put("Settings", fields(player.getSettings(), player.getSettings().getClass()));
        Map<String, Object> derived = new TreeMap<>();
        derived.put("totalLevel", player.getSkills().getTotalLevel());
        derived.put("rights", player.getDetails().getRights().name());
        derived.put("ipAddress", player.getDetails().getIpAddress());
        derived.put("varpMap", new ArrayList<Object>(player.varpMap.entrySet()).toString());
        derived.put("saveVarp", new ArrayList<Object>(player.saveVarp.entrySet()).toString());
        derived.put("attributes", new TreeMap<>(player.getAttributes()).toString());
        derived.put("bankTabs", value(player.getBankPrimary().getTabStartSlot()));
        derived.put("bankSecondaryTabs", value(player.getBankSecondary().getTabStartSlot()));
        derived.put("capacities", java.util.Arrays.<Object>asList(player.getInventory().capacity(), player.getEquipment().capacity(),
            player.getBankPrimary().capacity(), player.getBankSecondary().capacity(), player.getDropLog().capacity()));
        d.put("derived", derived);
        return d;
    }

    /** Wraps each registered PersistPlayer so that what each one writes into the save is recorded. */
    static final class Recording implements core.api.PersistPlayer {
        final core.api.PersistPlayer inner;
        final List<Object> sink;
        Recording(core.api.PersistPlayer inner, List<Object> sink) { this.inner = inner; this.sink = sink; }
        @Override public void savePlayer(Player player, JSONObject root) {
            Map<Object, Object> before = new LinkedHashMap<>(root);
            inner.savePlayer(player, root);
            List<Object> keys = new ArrayList<>();
            for (Object key : root.keySet()) {
                if (!before.containsKey(key) || before.get(key) != root.get(key)) {
                    List<Object> row = new ArrayList<>();
                    row.add(String.valueOf(key));
                    JSONObject one = new JSONObject();
                    one.put(key, root.get(key));
                    row.add(one.toJSONString());
                    keys.add(row);
                }
            }
            Map<String, Object> entry = new TreeMap<>();
            entry.put("class", inner.getClass().getName());
            entry.put("keys", keys);
            sink.add(entry);
        }
        @Override public void parsePlayer(Player player, JSONObject root) { inner.parsePlayer(player, root); }
    }

    static Map<String, Object> generators() {
        Map<String, Object> g = new TreeMap<>();
        g.put("clockReads", Pin.clockReads.get());
        g.put("seeded", Pin.seeds.get());
        g.put("shared", WorldTrace.counting(Pin.shared));
        g.put("math", WorldTrace.counting(Pin.math));
        g.put("shuffle", WorldTrace.counting(Pin.shuffle));
        g.put("threadLocal", Pin.threadLocalState());
        return g;
    }

    @SuppressWarnings("unchecked")
    static Map<String, Object> populate(Player player) {
        List<core.api.PersistPlayer> hooks =
            (List<core.api.PersistPlayer>) (List<?>) PlayerSaver.Companion.getContentHooks();
        List<Object> sink = new ArrayList<>();
        List<core.api.PersistPlayer> original = new ArrayList<>(hooks);
        for (int i = 0; i < hooks.size(); i++) hooks.set(i, new Recording(original.get(i), sink));
        Map<String, Object> record = new TreeMap<>();
        try {
            record.put("text", new PlayerSaver(player).populate().toJSONString());
        } finally {
            for (int i = 0; i < hooks.size(); i++) hooks.set(i, original.get(i));
        }
        record.put("hooks", sink);
        return record;
    }

    static Map<String, Object> world() {
        Map<String, Object> w = new TreeMap<>();
        List<Object> quests = new ArrayList<>();
        for (Map.Entry<?, Quest> entry : QuestRepository.getQuests().entrySet()) {
            Quest q = entry.getValue();
            List<Object> row = new ArrayList<>();
            row.add(((Enum<?>) entry.getKey()).ordinal());
            row.add(String.valueOf(entry.getKey()));
            row.add(q.getIndex());
            row.add(q.getButtonId());
            row.add(q.getQuestPoints());
            quests.add(row);
        }
        w.put("quests", quests);
        List<Object> songs = new ArrayList<>();
        for (MusicEntry song : new TreeMap<>(MusicEntry.getSongs()).values()) {
            List<Object> row = new ArrayList<>();
            row.add(song.getId());
            row.add(song.getName());
            row.add(song.getIndex());
            songs.add(row);
        }
        w.put("songs", songs);
        w.put("startLocation", value(ServerConstants.START_LOCATION));
        w.put("saveVersion", ServerConstants.CURRENT_SAVEFILE_VERSION);
        w.put("playerSavePath", ServerConstants.PLAYER_SAVE_PATH);
        w.put("ticks", GameWorld.getTicks());
        List<Object> hooks = new ArrayList<>();
        for (Object hook : PlayerSaver.Companion.getContentHooks()) hooks.add(hook.getClass().getName());
        w.put("saverHooks", hooks);
        List<Object> parserHooks = new ArrayList<>();
        for (Object hook : PlayerSaveParser.Companion.getContentHooks()) parserHooks.add(hook.getClass().getName());
        w.put("parserHooks", parserHooks);
        return w;
    }

    static Map<String, Object> construct(String name, String saveHex) throws Exception {
        Map<String, Object> record = new TreeMap<>();
        record.put("t", saveHex == null ? "fresh" : "load");
        record.put("name", name);
        Map<String, Object> before = generators();
        PlayerDetails details = PlayerDetails.getDetails(name);
        Player player = new Player(details);
        record.put("generatorsBefore", before);
        record.put("generatorsConstructed", generators());
        record.put("constructed", dump(player));
        if (saveHex != null) {
            Path file = Paths.get(ServerConstants.PLAYER_SAVE_PATH + name + ".json");
            Files.createDirectories(file.getParent());
            Files.write(file, Json.unhex(saveHex));
            record.put("file", saveHex);
            PlayerSaveParser parser = new PlayerSaveParser(player);
            parser.parse();
            parser.runContentHooks();
            record.put("generatorsLoaded", generators());
            record.put("loaded", dump(player));
        }
        record.put("populate", populate(player));
        record.put("generatorsSaved", generators());
        return record;
    }

    public static void main(String[] args) throws Throwable {
        WorldTrace.world = Paths.get("").toAbsolutePath();
        System.setErr(new PrintStream(new WorldTrace.CountingErr(System.err), true));
        System.setSecurityManager(new Guard(WorldTrace.world.getParent().toFile()));
        Pin.installGenerators(Long.parseLong(args[0]), Long.parseLong(args[1]));
        Handoffs.install();
        WorldTrace.files = WorldTrace.snapshot();
        WorldTrace.record("boot", WorldTrace.boot());
        out.add(WorldTrace.lines.get(0));
        Map<String, Object> w = world();
        w.put("t", "world");
        out.add(Json.write(w));
        out.add(Json.write(construct(args[2], null)));
        for (int i = 4; i + 1 < args.length; i += 2) {
            out.add(Json.write(construct(args[i], new String(Files.readAllBytes(Paths.get(args[i + 1])), StandardCharsets.US_ASCII).trim())));
        }
        Map<String, Object> end = new TreeMap<>();
        end.put("t", "end");
        end.put("stderrLines", WorldTrace.errLines);
        end.put("refused", new ArrayList<>(Guard.refused));
        end.put("unpinned", Pin.unpinned.get());
        end.put("foreign", Handoffs.drainForeign());
        out.add(Json.write(end));
        Files.write(Paths.get(args[3]), (String.join("\n", out) + "\n").getBytes(StandardCharsets.UTF_8));
        System.out.flush();
        System.err.flush();
        Runtime.getRuntime().halt(0);
    }
}
