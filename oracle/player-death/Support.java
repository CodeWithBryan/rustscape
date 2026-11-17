package oracle;

import content.global.handlers.iface.DeathInterface;
import content.global.handlers.item.equipment.BarrowsEquipment;
import core.cache.def.impl.ItemDefinition;
import core.game.component.Component;
import core.game.container.Container;
import core.game.container.ContainerEvent;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.DeathTask;
import core.game.node.entity.combat.graves.Grave;
import core.game.node.entity.combat.graves.GraveController;
import core.game.node.entity.combat.graves.GraveType;
import core.game.node.entity.npc.AbstractNPC;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.GroundItem;
import core.game.node.item.GroundItemManager;
import core.game.node.item.Item;
import core.game.node.item.ItemPlugin;
import core.game.world.map.Location;
import core.game.world.map.RegionChunk;
import core.game.world.repository.Repository;
import core.plugin.Plugin;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import kotlin.UninitializedPropertyAccessException;
import kotlin.jvm.functions.Function2;
import kotlin.jvm.functions.Function6;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives the unchanged DeathTask (getContainers, start, exec, stop), Grave,
 * GraveController, GraveType, BarrowsEquipment, DeathInterface,
 * GroundItemManager, GroundItem, Item and Container classes and the inserted
 * original Player/Entity death methods over one case per input line, and
 * records per action the result, the ordered service calls and the state the
 * original classes hold.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public final class Support {
	public static int tick;
	public static RegionChunk chunk;
	static List<Object> events = new ArrayList<>();
	static Map input;
	static List<Player> players;
	static List<NPC> npcs;
	static List<AbstractNPC> graves;
	static Map<String, Function2> listeners;
	static Map<Integer, Function6> interfaces;
	static Map<String, JSONObject> archives;
	static IdentityHashMap<GroundItem, String> groundLabels;
	static Map<Entity, Double> prayer;
	static GraveController controller;

	/** Repository.uid_map: reads are recorded, the case fills it silently. */
	public static class UidMap extends HashMap<Integer, Player> {
		@Override
		public Player get(Object key) {
			emit("uidMap", key);
			return super.get(key);
		}
	}

	private Support() {
	}

	// ---- recording ----

	public static void emit(String name, Object... arguments) {
		List<Object> event = new ArrayList<>();
		event.add(name);
		for (Object argument : arguments) {
			event.add(value(argument));
		}
		events.add(event);
	}

	public static Object value(Object value) {
		if (value instanceof Location) {
			return location((Location) value);
		}
		if (value instanceof Item) {
			return item((Item) value);
		}
		if (value instanceof Node) {
			return ((Node) value).label;
		}
		if (value == null || value instanceof Boolean || value instanceof String || value instanceof Number || value instanceof List) {
			return value;
		}
		if (value instanceof Enum) {
			return ((Enum) value).name();
		}
		return value.toString();
	}

	public static List<Integer> location(Location location) {
		return location == null ? null : Arrays.asList(location.getX(), location.getY(), location.getZ());
	}

	public static List<Integer> item(Item item) {
		return item == null ? null : Arrays.asList(item.getId(), item.getAmount(), item.getCharge());
	}

	public static String label(Object node) {
		return node == null ? null : ((Node) node).label;
	}

	// ---- case inputs ----

	public static int integer(Object value) {
		return ((Number) value).intValue();
	}

	public static int integer(Map map, String key, int fallback) {
		Object value = map.get(key);
		return value == null ? fallback : integer(value);
	}

	public static boolean flag(Map map, String key, boolean fallback) {
		Object value = map.get(key);
		return value == null ? fallback : (Boolean) value;
	}

	public static double real(Map map, String key, double fallback) {
		Object value = map.get(key);
		return value == null ? fallback : Double.longBitsToDouble(Long.parseLong((String) value));
	}

	public static Location place(Object value) {
		if (value == null) {
			return null;
		}
		List list = (List) value;
		return new Location(integer(list.get(0)), integer(list.get(1)), integer(list.get(2)));
	}

	static Entity entity(Object name) {
		if (name == null) {
			return null;
		}
		String text = (String) name;
		int index = Integer.parseInt(text.substring(1));
		switch (text.charAt(0)) {
			case 'p':
				return players.get(index);
			case 'n':
				return npcs.get(index);
			default:
				return graves.get(index);
		}
	}

	public static int statLevel(Entity entity, int skill) {
		Map levels = (Map) entity.in.get("levels");
		Object level = levels == null ? null : levels.get(String.valueOf(skill));
		return level == null ? 1 : integer(level);
	}

	public static double prayerPoints(Entity entity) {
		Double value = prayer.get(entity);
		return value != null ? value : real(entity.in, "prayerPoints", 0.0);
	}

	public static void setPrayerPoints(Entity entity, double value) {
		prayer.put(entity, value);
	}

	public static Item replacement(Object value, Item item) {
		if (value == null) {
			return null;
		}
		if (value instanceof String) {
			return item;
		}
		return newItem((List) value);
	}

	static Item newItem(List list) {
		return list.size() > 2 ? new Item(integer(list.get(0)), integer(list.get(1)), integer(list.get(2)))
			: new Item(integer(list.get(0)), integer(list.get(1)));
	}

	public static JSONObject archive(String name) {
		return archives.computeIfAbsent(name, key -> new JSONObject());
	}

	public static void grave(AbstractNPC npc, int id, Location location, boolean autowalk) {
		npc.label = "g" + graves.size();
		graves.add(npc);
		emit("graveNew", npc.label, id, location, autowalk);
	}

	public static void listen(String option, Function2 handler) {
		listeners.put(option, handler);
	}

	public static void listenInterface(int component, Function6 handler) {
		interfaces.put(component, handler);
	}

	static Field field(Class<?> type, String name) {
		while (type != null) {
			try {
				Field f = type.getDeclaredField(name);
				f.setAccessible(true);
				return f;
			} catch (NoSuchFieldException e) {
				type = type.getSuperclass();
			}
		}
		throw new IllegalStateException(name);
	}

	static Object read(Object target, String name) {
		try {
			return field(target.getClass(), name).get(target);
		} catch (IllegalAccessException e) {
			throw new IllegalStateException(e);
		}
	}

	static void definitions(Map items) throws Exception {
		ItemDefinition.getDefinitions().clear();
		for (Object key : items.keySet()) {
			Map row = (Map) items.get(key);
			ItemDefinition d = new ItemDefinition();
			field(ItemDefinition.class, "id").set(d, Integer.parseInt((String) key));
			for (Object name : row.keySet()) {
				Object v = row.get(name);
				switch ((String) name) {
					case "name":
						field(ItemDefinition.class, "name").set(d, v);
						break;
					case "value":
					case "noteId":
						field(ItemDefinition.class, (String) name).set(d, integer(v));
						break;
					case "stackable":
					case "unnoted":
						field(ItemDefinition.class, (String) name).set(d, v);
						break;
					case "options":
						List list = (List) v;
						field(ItemDefinition.class, "options").set(d, list.toArray(new String[0]));
						break;
					case "handlers":
						Map handlers = (Map) v;
						for (Object h : handlers.keySet()) {
							Object hv = handlers.get(h);
							if (h.equals("wrapper")) {
								Map p = (Map) hv;
								ItemPlugin plugin = new ItemPlugin();
								plugin.id = integer(p.get("id"));
								plugin.deathItem = p.get("deathItem");
								hv = plugin;
							} else if (h.equals("equipment")) {
								hv = new Plugin(integer(hv));
							} else if (hv instanceof Long) {
								hv = integer(hv);
							}
							d.getHandlers().put((String) h, hv);
						}
						break;
					default:
						throw new IllegalArgumentException((String) name);
				}
			}
			ItemDefinition.getDefinitions().put(Integer.parseInt((String) key), d);
		}
		Item fresh = new Item(0, 0);
		for (Class<?> type : Arrays.asList(Item.class, Node.class)) {
			for (Field f : type.getDeclaredFields()) {
				if (!Modifier.isStatic(f.getModifiers())) {
					f.setAccessible(true);
					f.set(ContainerEvent.NULL_ITEM, f.get(fresh));
				}
			}
		}
	}

	static void fill(Container container, Object slots) {
		if (slots == null) {
			return;
		}
		List list = (List) slots;
		for (int slot = 0; slot < list.size(); slot++) {
			if (list.get(slot) != null) {
				container.replace(newItem((List) list.get(slot)), slot, false);
			}
		}
	}

	static void setUp(Map c) throws Exception {
		input = c;
		tick = integer(c, "tick", 0);
		chunk = new RegionChunk();
		GroundItemManager.getItems().clear();
		GraveController.Companion.getActiveGraves().clear();
		Repository.getUid_map().clear();
		archives = new HashMap<>();
		groundLabels = new IdentityHashMap<>();
		prayer = new HashMap<>();
		graves = new ArrayList<>();
		listeners = new HashMap<>();
		interfaces = new HashMap<>();
		definitions((Map) c.getOrDefault("items", new HashMap()));
		players = new ArrayList<>();
		for (Object row : (List) c.getOrDefault("players", new ArrayList())) {
			Map in = (Map) row;
			Player player = new Player();
			player.label = "p" + players.size();
			player.in = in;
			player.active = flag(in, "active", true);
			field(Node.class, "name").set(player, in.get("name"));
			field(Node.class, "location").set(player, place(in.get("location")));
			fill(player.getInventory(), in.get("inventory"));
			fill(player.getEquipment(), in.get("equipment"));
			Map attributes = (Map) in.get("attributes");
			if (attributes != null) {
				for (Object key : attributes.keySet()) {
					Object v = attributes.get(key);
					if (v instanceof Map && ((Map) v).containsKey("loc")) {
						v = place(((Map) v).get("loc"));
					} else if (v instanceof Long) {
						v = integer(v);
					}
					player.attributes.put((String) key, v);
				}
			}
			if (flag(in, "online", true)) {
				Repository.getUid_map().put(integer(in, "uid", 0), player);
			}
			players.add(player);
		}
		npcs = new ArrayList<>();
		for (Object row : (List) c.getOrDefault("npcs", new ArrayList())) {
			Map in = (Map) row;
			NPC npc = new NPC(integer(in, "id", 1), place(in.get("location")));
			npc.label = "n" + npcs.size();
			npc.in = in;
			field(Node.class, "name").set(npc, in.get("name"));
			npcs.add(npc);
		}
		controller = new GraveController();
		controller.defineListeners();
		new DeathInterface().defineInterfaceListeners();
		events.clear();
	}

	// ---- actions ----

	static Object act(Map a) throws Exception {
		String op = (String) a.get("op");
		switch (op) {
			case "containers": {
				Container[] c = DeathTask.getContainers((Player) entity(a.get("player")));
				Map result = new TreeMap();
				result.put("kept", slots(c[0]));
				result.put("lost", slots(c[1]));
				return result;
			}
			case "commence":
				((Player) entity(a.get("player"))).commenceDeath(entity(a.get("killer")));
				return null;
			case "finalize":
				((Player) entity(a.get("player"))).finalizeDeath(entity(a.get("killer")));
				return null;
			case "start":
			case "exec":
			case "stop": {
				Entity e = entity(a.get("player"));
				Node[] n = a.get("killer") == null ? new Node[0] : new Node[] {entity(a.get("killer"))};
				if (op.equals("start")) {
					DeathTask.getSingleton().start(e, n);
					return null;
				}
				if (op.equals("exec")) {
					return DeathTask.getSingleton().exec(e, n);
				}
				DeathTask.getSingleton().stop(e, n);
				return null;
			}
			case "tick":
				step();
				return null;
			case "run":
				for (int i = integer(a.get("ticks")); i > 0; i--) {
					emit("tick", tick + 1);
					step();
				}
				return null;
			case "set":
				tick = integer(a.get("tick"));
				return null;
			case "get": {
				GroundItem g = GroundItemManager.get(integer(a.get("id")), place(a.get("location")), (Player) entity(a.get("player")));
				return ground(g);
			}
			case "take": {
				GroundItem g = GroundItemManager.get(integer(a.get("id")), place(a.get("location")), (Player) entity(a.get("player")));
				if (g != null) {
					GroundItemManager.destroy(g);
				}
				return ground(g);
			}
			case "read":
			case "bless":
			case "repair":
			case "demolish":
				return listeners.get(op).invoke(entity(a.get("player")), entity(a.get("grave")));
			case "logout": {
				Player p = (Player) entity(a.get("player"));
				Repository.getUid_map().remove(integer(p.in, "uid", 0));
				p.in.put("playing", false);
				return null;
			}
			case "login": {
				Player p = (Player) entity(a.get("player"));
				Repository.getUid_map().put(integer(p.in, "uid", 0), p);
				p.in.put("playing", true);
				return null;
			}
			case "setPlayer": {
				Player p = (Player) entity(a.get("player"));
				if ("location".equals(a.get("key"))) {
					field(Node.class, "location").set(p, place(a.get("value")));
				}
				p.in.put(a.get("key"), a.get("value"));
				return null;
			}
			case "setGrave": {
				Grave g = (Grave) entity(a.get("grave"));
				if (a.get("ticksRemaining") != null) {
					g.setTicksRemaining(integer(a.get("ticksRemaining")));
				}
				if (a.get("ticksSinceDeath") != null) {
					g.setTicksSinceDeath(integer(a.get("ticksSinceDeath")));
				}
				if (a.get("ticking") != null) {
					g.in.put("ticking", a.get("ticking"));
				}
				return null;
			}
			case "addTime":
				((Grave) entity(a.get("grave"))).addTime(integer(a.get("ticks")));
				return null;
			case "collapse":
				((Grave) entity(a.get("grave"))).collapse();
				return null;
			case "save": {
				controller.save();
				return archive("active-graves").toJSONString();
			}
			case "parse": {
				archives.put("active-graves", (JSONObject) new JSONParser().parse((String) a.get("archive")));
				controller.parse();
				return null;
			}
			case "deathButton": {
				Function6 handler = interfaces.get(153);
				return handler.invoke(entity(a.get("player")), new Component(153), integer(a.get("opcode")), integer(a.get("button")), integer(a.get("slot")),
					integer(a.get("item")));
			}
			case "graveType":
				return GraveController.getGraveType((Player) entity(a.get("player"))).ordinal();
			case "allow":
				return GraveController.allowGenerate((Player) entity(a.get("player")));
			case "crumble":
				return GraveController.shouldCrumble(integer(a.get("id")));
			case "release":
				return GraveController.shouldRelease(integer(a.get("id")));
			case "checkTransform":
				return item(GraveController.checkTransform(newItem((List) a.get("item"))));
			case "barrows": {
				Item item = newItem((List) a.get("item"));
				Map result = new TreeMap();
				result.put("is", BarrowsEquipment.isBarrowsItem(item.getId()));
				result.put("repaired", BarrowsEquipment.isFullyRepaired(item.getId()));
				result.put("broken", BarrowsEquipment.isBroken(item.getId()));
				BarrowsEquipment.BarrowsItemDefinition d = BarrowsEquipment.getDefinition(item.getId());
				result.put("brokenId", d == null ? null : d.getBrokenId());
				result.put("reduced", item(BarrowsEquipment.graveDeathDurabilityReduction(item)));
				return result;
			}
			case "types": {
				List rows = new ArrayList();
				for (GraveType type : GraveType.values()) {
					rows.add(Arrays.asList(type.name(), type.getNpcId(), type.getCost(), type.getDurationMinutes(), type.isMembers(),
						type.getRequiredQuest() == null ? null : type.getRequiredQuest().toString(), type.getText()));
				}
				List ids = new ArrayList();
				for (int id : GraveType.Companion.getIds()) {
					ids.add(id);
				}
				return Arrays.asList(rows, ids);
			}
			default:
				throw new IllegalArgumentException(op);
		}
	}

	static void step() {
		tick++;
		for (Object phase : (List) input.get("order")) {
			switch ((String) phase) {
				case "graves":
					controller.tick();
					break;
				case "ground":
					GroundItemManager.pulse();
					break;
				case "npcs":
					for (AbstractNPC g : new ArrayList<>(graves)) {
						if (Boolean.TRUE.equals(g.in.get("ticking"))) {
							g.tick();
						}
					}
					break;
				default:
					throw new IllegalArgumentException((String) phase);
			}
		}
	}

	// ---- state ----

	static List slots(Container container) {
		List list = new ArrayList();
		for (Item item : container.toArray()) {
			list.add(item(item));
		}
		return list;
	}

	static String groundLabel(GroundItem g) {
		return g == null ? null : groundLabels.computeIfAbsent(g, key -> "i" + groundLabels.size());
	}

	static Object ground(GroundItem g) {
		return groundLabel(g);
	}

	static Map attributes(Entity entity) {
		Map result = new TreeMap();
		for (Map.Entry<String, Object> entry : entity.attributes.entrySet()) {
			result.put(entry.getKey(), value(entry.getValue()));
		}
		return result;
	}

	static Map state() throws Exception {
		Map state = new TreeMap();
		state.put("tick", tick);
		List ps = new ArrayList();
		for (Player p : players) {
			Map m = new TreeMap();
			m.put("inventory", slots(p.getInventory()));
			m.put("equipment", slots(p.getEquipment()));
			m.put("attributes", attributes(p));
			m.put("disabled", p.getImpactHandler().getDisabledTicks());
			Double points = prayer.get(p);
			m.put("prayerPoints", points == null ? null : Double.doubleToRawLongBits(points) + "");
			ps.add(m);
		}
		state.put("players", ps);
		List manager = new ArrayList();
		for (GroundItem g : GroundItemManager.getItems()) {
			manager.add(groundLabel(g));
		}
		List chunkItems = new ArrayList();
		for (GroundItem g : chunk.getItems()) {
			chunkItems.add(groundLabel(g));
		}
		state.put("manager", manager);
		state.put("chunk", chunkItems);
		List active = new ArrayList();
		for (Map.Entry<Integer, Grave> entry : GraveController.Companion.getActiveGraves().entrySet()) {
			active.add(Arrays.asList(entry.getKey(), entry.getValue().label));
		}
		state.put("active", active);
		List gs = new ArrayList();
		for (AbstractNPC npc : graves) {
			Grave g = (Grave) npc;
			Map m = new TreeMap();
			m.put("id", g.stateId());
			m.put("location", location(g.stateLocation()));
			m.put("owner", g.getOwnerUid());
			m.put("username", g.getOwnerUsername());
			m.put("remaining", g.getTicksRemaining());
			m.put("since", g.getTicksSinceDeath());
			Object type = read(g, "type");
			m.put("type", type == null ? null : ((GraveType) type).name());
			List items = new ArrayList();
			for (GroundItem item : g.getItems()) {
				items.add(groundLabel(item));
			}
			m.put("items", items);
			m.put("attributes", attributes(g));
			gs.add(m);
		}
		state.put("graves", gs);
		List grounds = new ArrayList();
		List<GroundItem> ordered = new ArrayList<>(groundLabels.keySet());
		ordered.sort((x, y) -> Integer.compare(Integer.parseInt(groundLabels.get(x).substring(1)), Integer.parseInt(groundLabels.get(y).substring(1))));
		for (GroundItem g : ordered) {
			Player dropper = (Player) read(g, "dropper");
			grounds.add(Arrays.asList(g.getId(), g.getAmount(), g.getCharge(), location(g.getLocation()), dropper == null ? null : dropper.label,
				read(g, "dropperUid"), read(g, "ticks"), read(g, "decayTime"), read(g, "remainPrivate"), read(g, "removed")));
		}
		state.put("grounds", grounds);
		List ns = new ArrayList();
		for (NPC n : npcs) {
			ns.add(attributes(n));
		}
		state.put("npcs", ns);
		return state;
	}

	static String category(Throwable error) {
		if (error instanceof NullPointerException) {
			return "Null";
		}
		if (error instanceof ClassCastException) {
			return "Type";
		}
		if (error instanceof IndexOutOfBoundsException) {
			return "Index";
		}
		if (error instanceof NumberFormatException) {
			return "Number";
		}
		if (error instanceof UninitializedPropertyAccessException) {
			return "Uninitialized";
		}
		return error.getClass().getName() + ": " + error.getMessage();
	}

	public static void main(String[] args) throws Exception {
		BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
		PrintStream out = new PrintStream(System.out, true, "UTF-8");
		String line;
		while ((line = reader.readLine()) != null) {
			if (line.isEmpty()) {
				continue;
			}
			Map c = (Map) new JSONParser().parse(line);
			setUp(c);
			List expected = new ArrayList();
			for (Object action : (List) c.get("actions")) {
				events.clear();
				Map step = new TreeMap();
				Object result = null;
				String error = null;
				try {
					result = act((Map) action);
				} catch (Exception e) {
					error = category(e);
				}
				step.put("result", value(result instanceof Map || result instanceof List ? null : result));
				if (result instanceof Map || result instanceof List) {
					step.put("result", result);
				}
				step.put("error", error);
				step.put("events", new ArrayList<>(events));
				step.put("state", state());
				chunk.flags.clear();
				expected.add(step);
			}
			// The case as read: the run above mutates its player maps.
			Map row = new TreeMap((Map) new JSONParser().parse(line));
			row.put("expected", expected);
			out.println(JSONValue.toJSONString(row));
		}
	}
}
