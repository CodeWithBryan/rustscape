package oracle;

import content.global.skill.summoning.familiar.Familiar;
import core.ServerConstants;
import core.game.interaction.Option;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.music.MusicZone;
import core.game.node.entity.player.link.request.RequestType;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.system.task.Pulse;
import core.game.world.PulseRunner;
import core.game.world.map.Location;
import core.game.world.map.Region;
import core.game.world.map.RegionChunk;
import core.game.world.map.RegionManager;
import core.game.world.map.path.Pathfinder;
import core.game.world.map.zone.MapZone;
import core.game.world.map.zone.RegionZone;
import core.game.world.map.zone.ZoneBorders;
import core.game.world.map.zone.ZoneBuilder;
import core.game.world.map.zone.ZoneMonitor;
import core.game.world.map.zone.ZoneRestriction;
import core.game.world.map.zone.ZoneType;
import core.game.world.map.zone.impl.BankZone;
import core.game.world.map.zone.impl.KaramjaZone;
import core.game.world.map.zone.impl.ModeratorZone;
import core.game.world.map.zone.impl.MultiwayCombatZone;
import core.tools.RandomFunction;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
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
import java.util.TreeSet;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives the unchanged zone classes and Region.java from one JSON case per
 * line. Every adapter records through emit; a case can make an event run
 * further actions or fail.
 */
@SuppressWarnings({"unchecked", "rawtypes"})
public class Support {
	public static Map<String, Object> input;
	public static PulseRunner runner;
	static TreeMap<Integer, Entity> nodes;
	static Map<String, MapZone> zones;
	static IdentityHashMap<MapZone, String> zoneIds;
	static Map<String, ZoneBorders> borders;
	static Map<String, MusicZone> musics;
	static IdentityHashMap<MusicZone, String> musicIds;
	static List<Object> events;
	static Map<String, Integer> calls;
	static Set<Integer> fired;
	static int permittedCalls;

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

	/** The scripted answer for this call of a zone hook, or null for the base class. */
	static Boolean answer(String key) {
		List answers = (List) ((Map) get(input, "answers", Collections.emptyMap())).get(key);
		if (answers == null) {
			return null;
		}
		return (Boolean) answers.get(Math.min(calls.get(key) - 1, answers.size() - 1));
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
		List answers = (List) get(input, "permitted", Collections.singletonList(true));
		boolean answer = (Boolean) answers.get(Math.min(permittedCalls++, answers.size() - 1));
		emit("teleportPermitted");
		return answer;
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

	static Object targetLabel(Node node) {
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

	static Option option(Object name) {
		return name == null ? null : new Option((String) name);
	}

	static CombatStyle style(Object name) {
		return name == null ? null : CombatStyle.valueOf((String) name);
	}

	static RequestType request(Object name) {
		return name == null ? null : new RequestType((String) name);
	}

	static Item used(Object id) {
		return id == null ? null : new Item(num(id));
	}

	/** A zone whose every overridable member records and answers from the case. */
	public static class Scripted extends MapZone {
		final String id;

		public Scripted(String id, String name, boolean overlappable, ZoneRestriction... restrictions) {
			super(name, overlappable, restrictions);
			this.id = id;
		}

		Boolean ask(String hook, Object... args) {
			emit(id + "." + hook, args);
			return answer(id + "." + hook);
		}

		public void fire(boolean fire) {
			this.fireRandomEvents = fire;
		}

		public static void say(Entity e, String text) {
			message(e, text);
		}

		@Override
		public boolean enter(Entity e) {
			Boolean a = ask("enter", slot(e));
			return a == null ? super.enter(e) : a;
		}

		@Override
		public boolean leave(Entity e, boolean logout) {
			Boolean a = ask("leave", slot(e), logout);
			return a == null ? super.leave(e, logout) : a;
		}

		@Override
		public boolean canLogout(Player p) {
			Boolean a = ask("canLogout", slot(p));
			return a == null ? super.canLogout(p) : a;
		}

		@Override
		public boolean death(Entity e, Entity killer) {
			Boolean a = ask("death", slot(e), slot(killer));
			return a == null ? super.death(e, killer) : a;
		}

		@Override
		public boolean interact(Entity e, Node target, Option option) {
			Boolean a = ask("interact", slot(e), targetLabel(target), option == null ? null : option.getName());
			return a == null ? super.interact(e, target, option) : a;
		}

		@Override
		public boolean handleUseWith(Player player, Item used, Node with) {
			Boolean a = ask("useWith", slot(player), used == null ? null : used.getId(), targetLabel(with));
			return a == null ? super.handleUseWith(player, used, with) : a;
		}

		@Override
		public boolean actionButton(Player player, int interfaceId, int buttonId, int slot, int itemId, int opcode) {
			Boolean a = ask("button", slot(player), interfaceId, buttonId, slot, itemId, opcode);
			return a == null ? super.actionButton(player, interfaceId, buttonId, slot, itemId, opcode) : a;
		}

		@Override
		public boolean continuePvp(Entity e, Node target, CombatStyle style, boolean message) {
			Boolean a = ask("pvp", slot(e), targetLabel(target), style == null ? null : style.name(), message);
			return a == null ? super.continuePvp(e, target, style, message) : a;
		}

		@Override
		public boolean continueAttack(Entity e, Node target, CombatStyle style, boolean message) {
			Boolean a = ask("attack", slot(e), targetLabel(target), style == null ? null : style.name(), message);
			return a == null ? super.continueAttack(e, target, style, message) : a;
		}

		@Override
		public boolean ignoreMultiBoundaries(Entity attacker, Entity victim) {
			Boolean a = ask("ignoreMulti", slot(attacker), slot(victim));
			return a == null ? super.ignoreMultiBoundaries(attacker, victim) : a;
		}

		@Override
		public boolean teleport(Entity e, int type, Node node) {
			Boolean a = ask("teleport", slot(e), type, targetLabel(node));
			return a == null ? super.teleport(e, type, node) : a;
		}

		@Override
		public boolean startDeath(Entity e, Entity killer) {
			Boolean a = ask("startDeath", slot(e), slot(killer));
			return a == null ? super.startDeath(e, killer) : a;
		}

		@Override
		public boolean canRequest(RequestType type, Player player, Player target) {
			Boolean a = ask("request", type == null ? null : type.getName(), slot(player), slot(target));
			return a == null ? super.canRequest(type, player, target) : a;
		}

		@Override
		public boolean move(Entity e, Location from, Location to) {
			Boolean a = ask("move", slot(e), coord(from), coord(to));
			return a == null ? super.move(e, from, to) : a;
		}

		@Override
		public boolean parseCommand(Player player, String name, String[] arguments) {
			Boolean a = ask("command", slot(player), name, Arrays.asList(arguments));
			return a == null ? super.parseCommand(player, name, arguments) : a;
		}

		@Override
		public void locationUpdate(Entity e, Location last) {
			ask("locationUpdate", slot(e), coord(last));
			super.locationUpdate(e, last);
		}

		@Override
		public void configure() {
			ask("configure");
			super.configure();
		}

		@Override
		public boolean isDynamicZone() {
			Boolean a = ask("dynamic");
			return a == null ? super.isDynamicZone() : a;
		}
	}

	/** The same behaviour under another class, for MapZone.equals. */
	public static class Scripted2 extends Scripted {
		public Scripted2(String id, String name, boolean overlappable, ZoneRestriction... restrictions) {
			super(id, name, overlappable, restrictions);
		}
	}

	/** MapZone with nothing overridden. */
	public static class Plain extends MapZone {
		public Plain(String name, boolean overlappable, ZoneRestriction... restrictions) {
			super(name, overlappable, restrictions);
		}
	}

	static MapZone zone(Object id) {
		return id == null ? null : zones.get((String) id);
	}

	static ZoneBorders border(Object id) {
		return id == null ? null : borders.get((String) id);
	}

	static Region region(Object id) {
		return RegionManager.forId(num(id));
	}

	static RegionChunk chunk(Object value) {
		Location l = loc(value);
		return RegionManager.forId(l.getRegionId()).getChunks()[(l.getX() & 63) >> 3][(l.getY() & 63) >> 3][l.getZ()];
	}

	static List<Object> describe(ZoneBorders b) {
		return Arrays.asList(b.getSouthWestX(), b.getSouthWestY(), b.getNorthEastX(), b.getNorthEastY(), b.getPlane());
	}

	static List<Object> describe(List<RegionZone> list) {
		List<Object> out = new ArrayList<>();
		for (RegionZone z : list) {
			out.add(Arrays.asList(zoneIds.get(z.getZone()), describe(z.getBorders())));
		}
		return out;
	}

	static Pulse pulse(Region region) {
		try {
			Field field = Region.class.getDeclaredField("activityPulse");
			field.setAccessible(true);
			return (Pulse) field.get(region);
		} catch (ReflectiveOperationException e) {
			throw new IllegalStateException(e);
		}
	}

	static Object regionInfo(Region r) {
		Map<String, Object> out = new LinkedHashMap<>();
		Pulse pulse = pulse(r);
		out.put("active", r.isActive());
		out.put("loaded", r.isLoaded());
		out.put("viewers", new ArrayList<>(r.getViewingPlayers()));
		out.put("pulse", Arrays.asList(pulse.isRunning(), pulse.getTicksPassed(), pulse.getDelay()));
		out.put("pulseClass", pulse.getClass().getName());
		out.put("zones", describe(r.getRegionZones()));
		out.put("music", r.getMusic());
		List<Object> ids = new ArrayList<>();
		for (MusicZone z : r.getMusicZones()) {
			ids.add(musicIds.get(z));
		}
		out.put("musicZones", ids);
		return out;
	}

	static Object randomSummary(ZoneBorders b, List<Location> drawn) {
		boolean inside = true;
		TreeSet<Integer> xs = new TreeSet<>(), ys = new TreeSet<>(), zs = new TreeSet<>();
		for (Location l : drawn) {
			inside &= b.getSouthWestX() <= l.getX() && l.getX() <= b.getNorthEastX() && b.getSouthWestY() <= l.getY() && l.getY() <= b.getNorthEastY();
			xs.add(l.getX());
			ys.add(l.getY());
			zs.add(l.getZ());
		}
		Map<String, Object> out = new LinkedHashMap<>();
		out.put("inside", inside);
		// The generators are unseeded: a value set is a fact only when 96 draws
		// cover a span of at most three values (missed with p < 1e-16).
		boolean many = drawn.size() >= 96;
		out.put("xs", many && b.getNorthEastX() - b.getSouthWestX() <= 2 ? new ArrayList<>(xs) : null);
		out.put("ys", many && b.getNorthEastY() - b.getSouthWestY() <= 2 ? new ArrayList<>(ys) : null);
		out.put("zs", new ArrayList<>(zs));
		return out;
	}

	static String[] words(Object value) {
		List list = (List) value;
		String[] out = new String[list.size()];
		for (int i = 0; i < out.length; i++) {
			out[i] = (String) list.get(i);
		}
		return out;
	}

	static Object hook(MapZone z, Map a) {
		Entity n = entity(a.get("node"));
		switch ((String) a.get("hook")) {
		case "enter": return z.enter(n);
		case "leave": return z.leave(n, flag(a, "logout", false));
		case "canLogout": return z.canLogout((Player) n);
		case "death": return z.death(n, entity(a.get("killer")));
		case "interact": return z.interact(n, target(a.get("target")), option(a.get("option")));
		case "useWith": return z.handleUseWith((Player) n, used(a.get("used")), target(a.get("with")));
		case "button": return z.actionButton((Player) n, 1, 2, 3, 4, 5);
		case "pvp": return z.continuePvp(n, target(a.get("target")), style(a.get("style")), flag(a, "message", false));
		case "attack": return z.continueAttack(n, target(a.get("target")), style(a.get("style")), flag(a, "message", false));
		case "ignoreMulti": return z.ignoreMultiBoundaries(n, entity(a.get("victim")));
		case "teleport": return z.teleport(n, num(get(a, "type", 0)), target(a.get("target")));
		case "startDeath": return z.startDeath(n, entity(a.get("killer")));
		case "request": return z.canRequest(request(a.get("request")), (Player) n, (Player) entity(a.get("other")));
		case "move": return z.move(n, loc(a.get("from")), loc(a.get("to")));
		case "command": return z.parseCommand((Player) n, (String) a.get("name"), words(get(a, "args", Collections.emptyList())));
		case "locationUpdate": z.locationUpdate(n, loc(a.get("last"))); return null;
		case "dynamic": return z.isDynamicZone();
		default: throw new IllegalArgumentException((String) a.get("hook"));
		}
	}

	public static Object act(Map a) {
		String op = (String) a.get("op");
		Entity n = entity(a.get("node"));
		ZoneMonitor m = n == null ? null : n.getZoneMonitor();
		MapZone z = zone(a.get("zone"));
		ZoneBorders b = border(a.get("borders"));
		switch (op) {
		case "newBorders": {
			List v = (List) a.get("args");
			int x1 = num(v.get(0)), y1 = num(v.get(1)), x2 = num(v.get(2)), y2 = num(v.get(3));
			ZoneBorders made = a.containsKey("zeroCheck") ? new ZoneBorders(x1, y1, x2, y2, num(v.get(4)), (Boolean) a.get("zeroCheck"))
				: v.size() == 5 ? new ZoneBorders(x1, y1, x2, y2, num(v.get(4))) : new ZoneBorders(x1, y1, x2, y2);
			borders.put((String) a.get("id"), made);
			return null;
		}
		case "between":
			borders.put((String) a.get("id"), new ZoneBorders(loc(a.get("a")), loc(a.get("b"))));
			return null;
		case "forRegion":
			borders.put((String) a.get("id"), ZoneBorders.forRegion(num(a.get("region"))));
			return null;
		case "inside": {
			List v = (List) a.get("at");
			return b.insideBorder(num(v.get(0)), num(v.get(1)), num(v.get(2)));
		}
		case "insideXY": {
			List v = (List) a.get("at");
			return b.insideBorder(num(v.get(0)), num(v.get(1)));
		}
		case "insideLocation": return b.insideBorder(loc(a.get("location")));
		case "insideNode": return b.insideBorder((Node) n);
		case "insideRegion": return b.insideRegion(n);
		case "regionIds": return b.getRegionIds();
		case "corners": {
			List<Object> out = new ArrayList<>(describe(b));
			out.add(b.getExceptions() == null ? null : b.getExceptions().size());
			return out;
		}
		case "addException": b.addException(border(a.get("exception"))); return null;
		case "setPlane": b.setPlane(num(a.get("plane"))); return null;
		case "bordersEquals": {
			ZoneBorders x = border(a.get("a")), y = border(a.get("b"));
			return Arrays.asList(x.equals(y), x.hashCode() == y.hashCode());
		}
		case "bordersHash": return b.hashCode();
		case "bordersString": return b.toString();
		case "weighted": return coord(b.getWeightedRandomLoc(num(a.get("intensity"))));
		case "randomLoc": {
			List<Location> drawn = new ArrayList<>();
			for (int i = 0; i < num(a.get("count")); i++) {
				drawn.add(b.getRandomLoc());
			}
			return randomSummary(b, drawn);
		}
		case "randomWalkable": return randomSummary(b, Collections.singletonList(b.getRandomWalkableLoc()));
		case "zoneInfo":
			return Arrays.asList(z.getName(), z.getName() == null ? null : z.getUid(), z.isOverlappable(), z.isFireRandoms(), z.getRestriction(), z.getZoneType(), z.hashCode());
		case "zoneSet":
			if (a.containsKey("name")) z.setName((String) a.get("name"));
			if (a.containsKey("uid")) z.setUid(num(a.get("uid")));
			if (a.containsKey("overlappable")) z.setOverlappable((Boolean) a.get("overlappable"));
			if (a.containsKey("type")) z.setZoneType(num(a.get("type")));
			if (a.containsKey("restriction")) z.addRestriction(num(a.get("restriction")));
			if (a.containsKey("restrictionName")) z.addRestriction(ZoneRestriction.valueOf((String) a.get("restrictionName")));
			if (a.containsKey("fire")) ((Scripted) z).fire((Boolean) a.get("fire"));
			if (a.containsKey("disableRandoms")) z.disableRandomEvents();
			return null;
		case "zoneEquals": return zone(a.get("a")).equals(zone(a.get("b")));
		case "zoneRestricted": return z.isRestricted(num(a.get("flag")));
		case "regionZone": {
			RegionZone x = new RegionZone(zone(a.get("za")), border(a.get("ba")));
			RegionZone y = new RegionZone(zone(a.get("zb")), border(a.get("bb")));
			return Arrays.asList(x.equals(y), x.hashCode());
		}
		case "configure": ZoneBuilder.configure(z); return null;
		case "register": z.register(b); return null;
		case "unregister": z.unregister(b); return null;
		case "registerRegion": z.registerRegion(num(a.get("region"))); return null;
		case "registerRegionBorders": z.registerRegion(num(a.get("region")), b); return null;
		case "unregisterRegion": z.unregisterRegion(num(a.get("region"))); return null;
		case "enums": {
			List<Object> flags = new ArrayList<>(), types = new ArrayList<>();
			for (ZoneRestriction r : ZoneRestriction.values()) flags.add(r.getFlag());
			for (ZoneType t : ZoneType.values()) types.add(t.getId());
			return Arrays.asList(flags, types);
		}
		case "say": Scripted.say(n, (String) a.get("text")); return null;
		case "checkMulti": return MapZone.checkMulti(n, entity(a.get("target")), flag(a, "message", false));
		case "standingIn": {
			List v = (List) a.get("args");
			return Pathfinder.isStandingIn(num(v.get(0)), num(v.get(1)), num(v.get(2)), num(v.get(3)), num(v.get(4)), num(v.get(5)), num(v.get(6)), num(v.get(7)));
		}
		case "hook": return hook(z, a);
		case "update": return m.updateLocation(loc(a.get("last")));
		case "moveCheck": return m.move(loc(a.get("from")), loc(a.get("to")));
		case "interact": return m.interact(target(a.get("target")), option(a.get("option")));
		case "canLogout": return m.canLogout();
		case "teleport": return m.teleport(num(a.get("type")), target(a.get("target")));
		case "death": return m.handleDeath(entity(a.get("killer")));
		case "startDeath": return m.startDeath(entity(a.get("entity")), entity(a.get("killer")));
		case "clear": return m.clear();
		case "restricted": return m.isRestricted(num(a.get("flag")));
		case "type": return m.getType();
		case "pvp": return m.isPvPable(entity(a.get("player")), target(a.get("target")), style(a.get("style")), flag(a, "message", false));
		case "continueAttack": return m.continueAttack(target(a.get("target")), style(a.get("style")), flag(a, "message", false));
		case "useWith": return m.useWith(used(a.get("used")), target(a.get("with")));
		case "button": return m.clickButton(1, 2, 3, 4, 5);
		case "ignoreMulti": return m.isIgnoreMultiBoundaries(entity(a.get("victim")));
		case "canFire": return m.canFireRandomEvent();
		case "command": return m.parseCommand((Player) entity(a.get("player")), (String) a.get("name"), words(get(a, "args", Collections.emptyList())));
		case "canRequest": return m.canRequest(request(a.get("request")), (Player) entity(a.get("target")));
		case "inZone": return m.isInZone((String) a.get("name"));
		case "removeZone": m.remove(z); return null;
		case "checkMusic": m.checkMusicZones(); return null;
		case "teleBlocked": return n.isTeleBlocked();
		case "monitorAdd": m.getZones().add(new RegionZone(z, b)); return null;
		case "monitorClear": m.getZones().clear(); return null;
		case "musicAdd": m.getMusicZones().add(musics.get((String) a.get("music"))); return null;
		case "view": {
			Region r = region(a.get("region"));
			String name = (String) a.get("name");
			return num(a.get("delta")) > 0 ? r.incrementViewAmount(name) : r.decrementViewAmount(name);
		}
		case "viewed": return region(a.get("region")).isViewed();
		case "checkInactive": return region(a.get("region")).checkInactive();
		case "isInactive": return region(a.get("region")).isInactive((Boolean) a.get("run"));
		case "flagActive": region(a.get("region")).flagActive(); return null;
		case "flagInactive": return a.get("force") == null ? region(a.get("region")).flagInactive() : region(a.get("region")).flagInactive((Boolean) a.get("force"));
		case "unload": return region(a.get("region")).unload((Boolean) a.get("force"));
		case "timeout": region(a.get("region")).setRegionTimeOut(num(a.get("ticks"))); return null;
		case "regionInfo": return regionInfo(region(a.get("region")));
		case "newMusic": {
			MusicZone made = new MusicZone(num(a.get("music")), b);
			musics.put((String) a.get("id"), made);
			musicIds.put(made, (String) a.get("id"));
			return null;
		}
		case "musicZone": region(a.get("region")).getMusicZones().add(musics.get((String) a.get("music"))); return null;
		case "setMusic": region(a.get("region")).setMusic(num(a.get("music"))); return null;
		case "musicEnter": return musics.get((String) a.get("music")).enter(n);
		case "tick":
			for (int i = 0; i < num(get(a, "count", 1)); i++) {
				runner.updateAll();
			}
			return runner.getCurrentPulses().length;
		case "chunkAdd":
			if ("players".equals(a.get("list"))) chunk(a.get("chunk")).getPlayers().add((Player) n);
			else chunk(a.get("chunk")).getNpcs().add((NPC) n);
			return null;
		case "chunkRemove":
			if ("players".equals(a.get("list"))) chunk(a.get("chunk")).getPlayers().remove(n);
			else chunk(a.get("chunk")).getNpcs().remove(n);
			return null;
		case "chunkClearList":
			if ("players".equals(a.get("list"))) chunk(a.get("chunk")).getPlayers().clear();
			else chunk(a.get("chunk")).getNpcs().clear();
			return null;
		case "chunkStuff": {
			RegionChunk c = chunk(a.get("chunk"));
			c.updateFlags().add(new Object());
			c.getObjects()[1][2][0] = new Scenery();
			c.statics()[1][2][0] = new Scenery();
			return null;
		}
		case "set":
			for (Object key : a.keySet()) {
				if (!key.equals("op") && !key.equals("node")) {
					n.state().put((String) key, a.get(key));
				}
			}
			return null;
		case "toggle": ModeratorZone.toggle((Player) n, (Boolean) a.get("on")); return null;
		case "home": ModeratorZone.home((Player) n); return null;
		case "modTeleport": ModeratorZone.teleport((Player) n); return null;
		case "setHome": ServerConstants.HOME_LOCATION = loc(a.get("location")); return null;
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
		String id = (String) spec.get("id");
		String name = spec.containsKey("name") ? (String) spec.get("name") : id;
		boolean overlappable = flag(spec, "overlappable", true);
		List names = (List) get(spec, "restrictions", Collections.emptyList());
		ZoneRestriction[] restrictions = new ZoneRestriction[names.size()];
		for (int i = 0; i < restrictions.length; i++) {
			restrictions[i] = ZoneRestriction.valueOf((String) names.get(i));
		}
		switch ((String) get(spec, "class", "scripted")) {
		case "scripted": return new Scripted(id, name, overlappable, restrictions);
		case "scripted2": return new Scripted2(id, name, overlappable, restrictions);
		case "plain": return new Plain(name, overlappable, restrictions);
		case "bank": return new BankZone();
		case "karamja": return new KaramjaZone();
		case "moderator": return new ModeratorZone();
		case "multiway": return MultiwayCombatZone.getInstance();
		default: throw new IllegalArgumentException((String) spec.get("class"));
		}
	}

	public static Object evaluate(Map data) {
		input = data;
		nodes = new TreeMap<>();
		zones = new LinkedHashMap<>();
		zoneIds = new IdentityHashMap<>();
		borders = new HashMap<>();
		musics = new HashMap<>();
		musicIds = new IdentityHashMap<>();
		events = new ArrayList<>();
		calls = new HashMap<>();
		fired = new HashSet<>();
		permittedCalls = 0;
		runner = new PulseRunner();
		RegionManager.reset();
		ModeratorZone.open = true;
		ServerConstants.HOME_LOCATION = null;
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
			List<Object> ids = new ArrayList<>();
			for (MusicZone z : n.getZoneMonitor().getMusicZones()) {
				ids.add(musicIds.get(z));
			}
			row.put("music", ids);
			monitors.put("" + n.slot(), row);
		}
		out.put("monitors", monitors);
		Map regions = new TreeMap();
		Map chunks = new TreeMap();
		for (Map.Entry<Integer, Region> e : new TreeMap<>(RegionManager.cached()).entrySet()) {
			regions.put(e.getKey().toString(), regionInfo(e.getValue()));
			for (int x = 0; x < 8; x++) {
				for (int y = 0; y < 8; y++) {
					for (int plane = 0; plane < 4; plane++) {
						RegionChunk c = e.getValue().getChunks()[x][y][plane];
						int objects = 0;
						for (Scenery[][][] layer : new Scenery[][][][] {c.getObjects(), c.statics()}) {
							for (Scenery[][] column : layer) {
								for (Scenery[] cell : column) {
									for (Scenery s : cell) {
										objects += s == null ? 0 : 1;
									}
								}
							}
						}
						if (c.getPlayers().isEmpty() && c.getNpcs().isEmpty() && c.updateFlags().isEmpty() && objects == 0) {
							continue;
						}
						Map row = new LinkedHashMap();
						List<Object> players = new ArrayList<>(), npcs = new ArrayList<>();
						for (Player p : c.getPlayers()) players.add(slot(p));
						for (NPC npc : c.getNpcs()) npcs.add(slot(npc));
						row.put("players", players);
						row.put("npcs", npcs);
						row.put("flags", c.updateFlags().size());
						row.put("objects", objects);
						chunks.put(e.getKey() + ":" + x + "," + y + "," + plane, row);
					}
				}
			}
		}
		out.put("regions", regions);
		out.put("chunks", chunks);
		out.put("queue", runner.getCurrentPulses().length);
		out.put("probe", RandomFunction.RANDOM.nextInt());
		out.put("open", ModeratorZone.open);
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
