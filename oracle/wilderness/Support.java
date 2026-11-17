package oracle;

import content.data.LightSource;
import content.global.handlers.item.equipment.brawling_gloves.BrawlingGloves;
import content.global.skill.summoning.familiar.Familiar;
import core.game.component.Component;
import core.game.event.UseWithEvent;
import core.game.interaction.Option;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.PulseRunner;
import core.game.world.map.Location;
import core.game.world.map.Region;
import core.game.world.map.RegionManager;
import core.game.world.map.zone.MapZone;
import core.game.world.map.zone.RegionZone;
import core.game.world.map.zone.ZoneBorders;
import core.game.world.map.zone.ZoneBuilder;
import core.game.world.map.zone.ZoneMonitor;
import core.game.world.map.zone.impl.BankZone;
import core.game.world.map.zone.impl.DarkZone;
import core.game.world.map.zone.impl.KaramjaZone;
import core.game.world.map.zone.impl.ModeratorZone;
import core.game.world.map.zone.impl.MultiwayCombatZone;
import core.game.world.map.zone.impl.WildernessZone;
import core.tools.RandomFunction;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.ConcurrentModificationException;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import kotlin.Unit;
import kotlin.jvm.functions.Function0;
import kotlin.jvm.functions.Function1;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives the unchanged WildernessZone, DarkZone, LightSource, BrawlingGloves,
 * ZoneBuilder, ZoneMonitor and MapZone from one JSON case per line. Every
 * adapter records through emit; a case can make an event run further
 * actions or fail.
 */
@SuppressWarnings({"unchecked", "rawtypes"})
public class Support {
	public static Map<String, Object> input;
	public static PulseRunner runner;
	public static List<Function1<Integer, Boolean>> scripts;
	public static List<Function0<Unit>> tasks;
	static TreeMap<Integer, Entity> nodes;
	static Map<String, MapZone> zones;
	static IdentityHashMap<Object, String> zoneIds;
	static IdentityHashMap<Object, Integer> pulseIds;
	static List<Object> events;
	static Map<String, Integer> calls;
	static Set<Integer> fired;

	public static class Failure extends RuntimeException {
	}

	public static int num(Object value) {
		return ((Number) value).intValue();
	}

	public static Object get(Map map, String key, Object fallback) {
		Object value = map.get(key);
		return value == null ? fallback : value;
	}

	public static int num(Map map, String key, int fallback) {
		return num(get(map, key, fallback));
	}

	public static boolean flag(Map map, String key, boolean fallback) {
		return (Boolean) get(map, key, fallback);
	}

	public static Location loc(Object value) {
		if (value == null) {
			return null;
		}
		List list = (List) value;
		return new Location(num(list.get(0)), num(list.get(1)), num(list.get(2)));
	}

	public static List<Integer> coord(Location location) {
		return location == null ? null : Arrays.asList(location.getX(), location.getY(), location.getZ());
	}

	public static Entity entity(Object id) {
		return id == null ? null : nodes.get(num(id));
	}

	static Integer slot(Entity entity) {
		return entity == null ? null : entity.slot();
	}

	public static void emit(String event, Object... args) {
		List<Object> row = new ArrayList<>();
		row.add(event);
		row.addAll(Arrays.asList(args));
		events.add(row);
		int count = calls.merge(event, 1, Integer::sum);
		List hooks = (List) get(input, "hooks", Collections.emptyList());
		for (int i = 0; i < hooks.size(); i++) {
			Map hook = (Map) hooks.get(i);
			if (event.equals(hook.get("event")) && count == num(get(hook, "at", 1)) && fired.add(i)) {
				for (Object action : (List) hook.get("actions")) {
					act((Map) action);
				}
			}
		}
		Map failures = (Map) get(input, "failures", Collections.emptyMap());
		if (failures.containsKey(event) && num(failures.get(event)) == count) {
			throw new Failure();
		}
	}

	/** The scripted answer for this call of an event, or null for the default. */
	public static Boolean answer(String key) {
		List answers = (List) ((Map) get(input, "answers", Collections.emptyMap())).get(key);
		if (answers == null) {
			return null;
		}
		return (Boolean) answers.get(Math.min(calls.get(key) - 1, answers.size() - 1));
	}

	/** An item name of the case: absent ids are "Item id", a JSON null is null. */
	public static String itemName(int id) {
		Map names = (Map) get(input, "names", Collections.emptyMap());
		String key = Integer.toString(id);
		return names.containsKey(key) ? (String) names.get(key) : "Item " + id;
	}

	public static List<NPC> localNpcs(Location location, int distance) {
		emit("localNpcs", coord(location), distance);
		List<NPC> found = new ArrayList<>();
		for (Object id : (List) get(input, "localNpcs", Collections.emptyList())) {
			found.add((NPC) entity(id));
		}
		return found;
	}

	public static List<Player> localPlayers(Location location, int distance) {
		emit("localPlayers", coord(location), distance);
		List<Player> found = new ArrayList<>();
		for (Object id : (List) get(input, "localPlayers", Collections.emptyList())) {
			found.add((Player) entity(id));
		}
		return found;
	}

	public static boolean permitted(Location location) {
		emit("teleportPermitted");
		return true;
	}

	public static Object targetLabel(Node node) {
		if (node == null) {
			return null;
		}
		if (node instanceof Entity) {
			return Arrays.asList("node", ((Entity) node).slot());
		}
		if (node instanceof Item) {
			return Arrays.asList("item", ((Item) node).getId());
		}
		return Arrays.asList("other");
	}

	public static Object zoneLabel(Object zone) {
		if (zone == null) {
			return null;
		}
		String id = zoneIds.get(zone);
		return id != null ? id : zone.getClass().getSimpleName();
	}

	public static Object pulseLabel(Object pulse) {
		if (pulse == null) {
			return null;
		}
		Integer id = pulseIds.get(pulse);
		if (id == null) {
			id = pulseIds.size();
			pulseIds.put(pulse, id);
		}
		return "pulse" + id;
	}

	public static Object componentLabel(Component component) {
		if (component == null) {
			return null;
		}
		return component == DarkZone.DARKNESS_OVERLAY ? "darkness" : (Object) component.getId();
	}

	static Node target(Object value) {
		if (value == null) {
			return null;
		}
		Map map = (Map) value;
		if (map.containsKey("node")) {
			return entity(map.get("node"));
		}
		if (map.containsKey("item")) {
			return new Item(num(map.get("item")));
		}
		return new Node();
	}

	static Option option(Object name) {
		return name == null ? null : new Option((String) name);
	}

	static CombatStyle style(Object name) {
		return name == null ? null : CombatStyle.valueOf((String) name);
	}

	static Component component(Object spec) {
		return "darkness".equals(spec) ? DarkZone.DARKNESS_OVERLAY : new Component(num(spec));
	}

	static MapZone zone(Object id) {
		return id == null ? null : zones.get((String) id);
	}

	static ZoneBorders newBorders(Object value) {
		List v = (List) value;
		int x1 = num(v.get(0)), y1 = num(v.get(1)), x2 = num(v.get(2)), y2 = num(v.get(3));
		return v.size() == 5 ? new ZoneBorders(x1, y1, x2, y2, num(v.get(4))) : new ZoneBorders(x1, y1, x2, y2);
	}

	static List<Object> describe(ZoneBorders b) {
		return Arrays.asList(b.getSouthWestX(), b.getSouthWestY(), b.getNorthEastX(), b.getNorthEastY(), b.getPlane());
	}

	static List<Object> describe(List<RegionZone> list) {
		List<Object> out = new ArrayList<>();
		for (RegionZone z : list) {
			out.add(Arrays.asList(zoneLabel(z.getZone()), describe(z.getBorders())));
		}
		return out;
	}

	static Object extensionInfo(Entity n) {
		Object pulse = n.extension(DarkZone.class);
		return pulse == null ? null : Arrays.asList(pulseLabel(pulse), ((Pulse) pulse).isRunning());
	}

	static Object light(LightSource s) {
		return s == null ? null : s.name();
	}

	static Object hook(MapZone z, Map a) {
		Entity n = entity(a.get("node"));
		switch ((String) a.get("hook")) {
		case "enter": return z.enter(n);
		case "leave": return z.leave(n, flag(a, "logout", false));
		case "death": return z.death(n, entity(a.get("killer")));
		case "interact": return z.interact(n, target(a.get("target")), option(a.get("option")));
		case "teleport": return z.teleport(n, num(get(a, "type", 0)), target(a.get("target")));
		case "attack": return z.continueAttack(n, target(a.get("target")), style(a.get("style")), flag(a, "message", false));
		case "locationUpdate": z.locationUpdate(n, loc(a.get("last"))); return null;
		default: throw new IllegalArgumentException((String) a.get("hook"));
		}
	}

	public static Object act(Map a) {
		String op = (String) a.get("op");
		Entity n = entity(a.get("node"));
		ZoneMonitor m = n == null ? null : n.getZoneMonitor();
		MapZone z = zone(a.get("zone"));
		switch (op) {
		case "set":
			for (Object key : a.keySet()) {
				if (!key.equals("op") && !key.equals("node")) {
					n.state().put((String) key, a.get(key));
				}
			}
			return null;
		case "configure": ZoneBuilder.configure(z); return null;
		case "startup": new ZoneBuilder().startup(); return null;
		case "register": z.register(newBorders(a.get("borders"))); return null;
		case "zoneInfo":
			return Arrays.asList(z.getName(), z.getUid(), z.isOverlappable(), z.isFireRandoms(), z.getRestriction(), z.getZoneType(), z.hashCode());
		case "hook": return hook(z, a);
		case "update": return m.updateLocation(loc(a.get("last")));
		case "teleport": return m.teleport(num(a.get("type")), target(a.get("target")));
		case "continueAttack": return m.continueAttack(target(a.get("target")), style(a.get("style")), flag(a, "message", false));
		case "interact": return m.interact(target(a.get("target")), option(a.get("option")));
		case "death": return m.handleDeath(entity(a.get("killer")));
		case "clear": return m.clear();
		case "wildLevel": return WildernessZone.getWilderness(n);
		case "inZone": return WildernessZone.isInZone(n);
		case "inZoneLocation": return WildernessZone.isInZone(loc(a.get("location")));
		case "checkTeleport": return WildernessZone.checkTeleport((Player) n, num(a.get("level")));
		case "show": WildernessZone.show((Player) n); return null;
		case "leavePlayer": ((WildernessZone) z).leave((Player) n); return null;
		case "dropRate": return WildernessZone.getInstance().getNewDropRate(num(a.get("level")));
		case "wildBorders": {
			List<Object> out = new ArrayList<>();
			for (ZoneBorders b : WildernessZone.getInstance().getBorders()) {
				out.add(describe(b));
			}
			return out;
		}
		case "constants": return Arrays.asList(WildernessZone.WILDERNESS_PROT_ATTR, WildernessZone.WILDERNESS_HIGHER_NEXTFEE);
		case "updateOverlay": ((DarkZone) z).updateOverlay((Player) n); return null;
		case "checkDark": return DarkZone.checkDarkArea((Player) n);
		case "process": ((DarkZone) z).process(n, new UseWithEvent(num(a.get("used")), num(a.get("with")))); return null;
		case "darkOpen": DarkZone.DARKNESS_OVERLAY.open((Player) n); return null;
		case "darkClose": return DarkZone.DARKNESS_OVERLAY.close((Player) n);
		case "openOverlay": ((Player) n).getInterfaceManager().openOverlay(component(a.get("component"))); return null;
		case "closeOverlay": ((Player) n).getInterfaceManager().closeOverlay(); return null;
		case "runScript": return scripts.get(num(a.get("index"))).invoke(num(get(a, "stage", 0)));
		case "runTask": tasks.get(num(a.get("index"))).invoke(); return null;
		case "tick":
			for (int i = 0; i < num(get(a, "count", 1)); i++) {
				runner.updateAll();
			}
			return runner.getCurrentPulses().length;
		case "lightTable": {
			List<Object> out = new ArrayList<>();
			for (LightSource s : LightSource.values()) {
				out.add(Arrays.asList(s.name(), s.getName(), s.getLevel(), s.getRaw().getId(), s.getProduct().getId(), s.isOpen(), s.getInterfaceId(), s.getStrength()));
			}
			List<Object> raw = new ArrayList<>();
			for (int id : LightSource.getRawIds()) {
				raw.add(id);
			}
			out.add(raw);
			return out;
		}
		case "lightFor": return Arrays.asList(light(LightSource.forProductId(num(a.get("id")))), light(LightSource.forId(num(a.get("id")))));
		case "activeLight": return light(LightSource.getActiveLightSource((Player) n));
		case "hasLight": return LightSource.hasActiveLightSource((Player) n);
		case "gloves": {
			List<Object> out = new ArrayList<>();
			for (int i = -1; i <= 15; i++) {
				BrawlingGloves g = BrawlingGloves.forIndicator(i);
				out.add(g == null ? null : g.getId());
			}
			return out;
		}
		default: throw new IllegalArgumentException(op);
		}
	}

	static String error(Throwable e) {
		while (e.getCause() != null) {
			e = e.getCause();
		}
		if (e instanceof Failure) return "Failure";
		if (e instanceof NullPointerException) return "Null";
		if (e instanceof ClassCastException) return "ClassCast";
		if (e instanceof ConcurrentModificationException) return "ConcurrentModification";
		if (e instanceof IllegalStateException) return "IllegalState";
		if (e instanceof IllegalArgumentException) return "IllegalArgument";
		if (e instanceof ArithmeticException) return "Arithmetic";
		return e.getClass().getSimpleName();
	}

	static MapZone make(Map spec) {
		switch ((String) get(spec, "class", "wilderness")) {
		case "wilderness": return WildernessZone.getInstance();
		case "wildernessBorders": {
			List list = (List) spec.get("borders");
			ZoneBorders[] made = new ZoneBorders[list.size()];
			for (int i = 0; i < made.length; i++) {
				made[i] = newBorders(list.get(i));
			}
			return new WildernessZone(made);
		}
		case "dark": return new DarkZone();
		case "plain": return new MapZone((String) spec.get("id"), true) {
		};
		case "multiway": return MultiwayCombatZone.getInstance();
		default: throw new IllegalArgumentException((String) spec.get("class"));
		}
	}

	public static Object evaluate(Map data) {
		input = data;
		nodes = new TreeMap<>();
		zones = new LinkedHashMap<>();
		zoneIds = new IdentityHashMap<>();
		pulseIds = new IdentityHashMap<>();
		events = new ArrayList<>();
		calls = new HashMap<>();
		fired = new HashSet<>();
		scripts = new ArrayList<>();
		tasks = new ArrayList<>();
		runner = new PulseRunner();
		RegionManager.reset();
		RandomFunction.RANDOM.setSeed(((Number) get(data, "seed", 530L)).longValue());
		for (Object value : (List) get(data, "nodes", Collections.emptyList())) {
			Map spec = new HashMap((Map) value);
			int id = num(spec.get("id"));
			String kind = (String) get(spec, "kind", "player");
			nodes.put(id, kind.equals("player") ? new Player(id, spec) : kind.equals("npc") ? new NPC(id, spec)
				: kind.equals("familiar") ? new Familiar(id, spec) : new Entity(id, spec));
		}
		for (Object value : (List) get(data, "zones", Collections.emptyList())) {
			MapZone made = make((Map) value);
			zones.put((String) ((Map) value).get("id"), made);
			zoneIds.put(made, (String) ((Map) value).get("id"));
		}
		List<Object> results = new ArrayList<>();
		for (Object action : (List) data.get("actions")) {
			try {
				Map row = new LinkedHashMap();
				row.put("value", act((Map) action));
				results.add(row);
			} catch (Throwable e) {
				results.add(Collections.singletonMap("error", error(e)));
			}
		}
		Map out = new LinkedHashMap();
		out.put("results", results);
		out.put("events", events);
		Map monitors = new TreeMap();
		for (Entity n : nodes.values()) {
			Map row = new LinkedHashMap();
			row.put("zones", describe(n.getZoneMonitor().getZones()));
			row.put("overlay", n instanceof Player ? componentLabel(((Player) n).overlay()) : null);
			row.put("extension", extensionInfo(n));
			monitors.put("" + n.slot(), row);
		}
		out.put("monitors", monitors);
		Map regions = new TreeMap();
		for (Map.Entry<Integer, Region> e : new TreeMap<>(RegionManager.cached()).entrySet()) {
			if (!e.getValue().getRegionZones().isEmpty()) {
				regions.put(e.getKey().toString(), describe(e.getValue().getRegionZones()));
			}
		}
		out.put("regions", regions);
		out.put("queue", runner.getCurrentPulses().length);
		out.put("scripts", scripts.size());
		out.put("tasks", tasks.size());
		out.put("probe", RandomFunction.RANDOM.nextInt());
		return out;
	}

	public static void main(String[] args) throws Exception {
		PrintStream out = new PrintStream(System.out, false, "UTF-8");
		// Throwable.printStackTrace prints the throwable first: one event per trace.
		System.setErr(new PrintStream(new OutputStream() {
			@Override
			public void write(int b) {
			}
		}) {
			@Override
			public void println(Object x) {
				if (x instanceof Throwable) {
					events.add(new ArrayList<>(Collections.singletonList("stackTrace")));
				}
			}
		});
		BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, "UTF-8"));
		JSONParser parser = new JSONParser();
		String line;
		while ((line = reader.readLine()) != null) {
			Map data = (Map) parser.parse(line);
			Object expected = evaluate(data);
			Map result = new LinkedHashMap(data);
			result.put("expected", expected);
			out.println(JSONValue.toJSONString(result));
		}
		out.flush();
	}
}
