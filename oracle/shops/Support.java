package oracle;

import core.cache.def.impl.ItemDefinition;
import core.game.component.Component;
import core.game.component.ComponentPlugin;
import core.game.container.Container;
import core.game.container.ContainerEvent;
import core.game.container.ContainerListener;
import core.game.event.Event;
import core.game.event.ItemShopPurchaseEvent;
import core.game.event.ItemShopSellEvent;
import core.game.interaction.IntType;
import core.game.interaction.InterfaceListeners;
import core.game.node.Node;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.IronmanMode;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.item.Item;
import core.game.shops.Shop;
import core.game.shops.ShopItem;
import core.game.shops.ShopListener;
import core.game.shops.Shops;
import core.game.world.GameWorld;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives one shops case against the unchanged Shop, Shops, container,
 * interface-manager, InterfaceListeners, RunScript and packet classes.
 * Launcher loads this class and everything it reaches in a fresh class loader
 * per case, so every companion table starts as a new JVM would create it.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public final class Support {
	static Map input;
	static List<Object> events;
	static final Map<String, Integer> calls = new HashMap<>();
	static Map failures;
	/** The interface configuration load before the case: its log line is not part of the case. */
	static boolean quiet;
	public static String config;
	static final Map<String, Object> serverConfig = new HashMap<>();
	static final List<Player> players = new ArrayList<>();
	static final List<Map> specs = new ArrayList<>();
	static final Map<Integer, String> examines = new HashMap<>();
	static final List<Object[]> npcListeners = new ArrayList<>();
	static final List<Object[]> destinations = new ArrayList<>();

	/** A failure the case injects into a recorded call. */
	public static final class Injected extends RuntimeException {
		Injected(String message) {
			super(message);
		}
	}

	static int n(Object value) {
		return ((Number) value).intValue();
	}

	public static Integer index(Player p) {
		return p == null ? null : players.indexOf(p);
	}

	public static void emit(String name, Object... args) {
		if (quiet) {
			return;
		}
		List<Object> row = new ArrayList<>();
		row.add(name);
		row.addAll(Arrays.asList(args));
		events.add(row);
		int at = calls.merge(name, 1, Integer::sum);
		Object fail = failures.get(name);
		if (fail != null && n(fail) == at) {
			throw new Injected(name);
		}
	}

	public static void sent(Player owner, core.net.packet.IoBuffer b) {
		byte[] a = Arrays.copyOf(b.array(), b.toByteBuffer().position());
		StringBuilder s = new StringBuilder();
		for (byte x : a) {
			s.append(String.format("%02x", x & 255));
		}
		emit("send", index(owner), b.opcode(), s.toString());
	}

	public static Object config(String key) {
		return serverConfig.get(key);
	}

	public static int questPoints(Player p) {
		return n(specs.get(index(p)).getOrDefault("questPoints", 0));
	}

	public static int questStage(Player p, String quest) {
		Map stages = (Map) specs.get(index(p)).getOrDefault("quests", Collections.emptyMap());
		return n(stages.getOrDefault(quest, 0));
	}

	public static String examine(ItemDefinition definition) {
		emit("examine", definition.getId());
		return examines.get(definition.getId());
	}

	public static void event(Player p, Event e) {
		if (e instanceof ItemShopPurchaseEvent) {
			ItemShopPurchaseEvent x = (ItemShopPurchaseEvent) e;
			emit("purchase", index(p), x.getItemId(), x.getAmount(), x.getCurrency().getId(), x.getCurrency().getAmount());
		} else if (e instanceof ItemShopSellEvent) {
			ItemShopSellEvent x = (ItemShopSellEvent) e;
			emit("sale", index(p), x.getItemId(), x.getAmount(), x.getCurrency().getId(), x.getCurrency().getAmount());
		} else {
			throw new IllegalStateException("unexpected event " + e);
		}
	}

	public static void listen(int[] ids, IntType type, String[] options, Function2 handler) {
		List<Object> list = new ArrayList<>();
		if (ids != null) {
			for (int id : ids) {
				list.add(id);
			}
		}
		emit("listen", ids == null ? null : list, type.name(), Arrays.asList(options));
		npcListeners.add(new Object[] {ids, options, handler});
	}

	public static void destination(IntType type, String[] options, Function2 handler) {
		emit("destination", type.name(), Arrays.asList(options));
		destinations.add(new Object[] {options, handler});
	}

	public static void command(String name, String privilege) {
		emit("command", name, privilege);
	}

	/** The player's inventory listener stand-in: records what InventoryListener would receive. */
	public static final class InventoryRecorder implements ContainerListener {
		final Player owner;

		public InventoryRecorder(Player owner) {
			this.owner = owner;
		}

		@Override
		public void update(Container c, ContainerEvent event) {
			List<Object> slots = new ArrayList<>();
			for (int slot : event.getSlots()) {
				slots.add(slot);
			}
			emit("invUpdate", index(owner), slots);
		}

		@Override
		public void refresh(Container c) {
			emit("invRefresh", index(owner));
		}
	}

	/** A content item view (a recorded callback): hides ids or shows one id as another. */
	public static final class View implements Function2<Player, Item, Item> {
		final List hide;
		final List swap;

		View(List hide, List swap) {
			this.hide = hide;
			this.swap = swap;
		}

		@Override
		public Item invoke(Player p, Item item) {
			emit("view", index(p), item.getId(), item.getAmount());
			for (Object id : hide) {
				if (n(id) == item.getId()) {
					return null;
				}
			}
			if (swap.size() == 2 && n(swap.get(0)) == item.getId()) {
				return new Item(n(swap.get(1)), item.getAmount());
			}
			return item;
		}
	}

	static void field(Object target, Class<?> owner, String name, Object value) throws ReflectiveOperationException {
		Field f = owner.getDeclaredField(name);
		f.setAccessible(true);
		f.set(target, value);
	}

	static void define(int id, Map spec) throws ReflectiveOperationException {
		ItemDefinition d = new ItemDefinition();
		field(d, core.cache.def.Definition.class, "id", id);
		field(d, core.cache.def.Definition.class, "name", spec.get("name"));
		List options = (List) spec.get("options");
		if (options != null) {
			field(d, core.cache.def.Definition.class, "options", ((List<String>) options).toArray(new String[0]));
		}
		field(d, ItemDefinition.class, "stackable", Boolean.TRUE.equals(spec.get("stackable")));
		field(d, ItemDefinition.class, "unnoted", !Boolean.FALSE.equals(spec.get("unnoted")));
		field(d, ItemDefinition.class, "noteId", spec.get("noteId") == null ? -1 : n(spec.get("noteId")));
		field(d, ItemDefinition.class, "value", spec.get("value") == null ? 1 : n(spec.get("value")));
		if (spec.containsKey("tradeable")) {
			d.getHandlers().put("tradeable", spec.get("tradeable"));
		}
		for (String key : new String[] {"high_alchemy", "low_alchemy", "tokkul_price", "archery_ticket_price", "castle_wars_ticket_price"}) {
			if (spec.containsKey(key)) {
				d.getHandlers().put(key, n(spec.get(key)));
			}
		}
		examines.put(id, (String) spec.get("examine"));
		ItemDefinition.getDefinitions().put(id, d);
	}

	static Player player(Map a, String key) {
		return players.get(n(a.get(key)));
	}

	static Direction direction(Object name) {
		return name == null ? Direction.SOUTH : Direction.valueOf((String) name);
	}

	static NPC npc(Map a) {
		Object x = a.get("x");
		Location location = x == null ? new Location(3200, 3200, 0) : new Location(n(x), n(a.get("y")), n(a.get("z")));
		return new NPC(n(a.get("npc")), a.get("booth"), direction(a.get("dir")), location);
	}

	static Object status(Shop.TransactionStatus status) {
		if (status instanceof Shop.TransactionStatus.Failure) {
			return "failure:" + ((Shop.TransactionStatus.Failure) status).getReason();
		}
		return "success";
	}

	static Object act(Map a) throws Exception {
		String op = (String) a.get("op");
		Player p = a.containsKey("p") ? player(a, "p") : null;
		switch (op) {
		case "open":
			Shops.openId(p, n(a.get("shop")));
			return null;
		case "openView": {
			Shop shop = Shops.getShopsById().get(n(a.get("shop")));
			if (shop == null) {
				return "none";
			}
			shop.openFor(p, new View((List) a.getOrDefault("hide", Collections.emptyList()), (List) a.getOrDefault("swap", Collections.emptyList())));
			return null;
		}
		case "npc": {
			// The listener registered for this NPC id and option, else the one
			// registered for the option alone (the dispatch order is modelled).
			String option = (String) a.getOrDefault("option", "trade");
			int id = n(a.get("npc"));
			Function2 found = null;
			for (int pass = 0; pass < 2 && found == null; pass++) {
				for (Object[] entry : npcListeners) {
					int[] ids = (int[]) entry[0];
					if ((pass == 0) != (ids != null) || !Arrays.asList((String[]) entry[1]).contains(option)) {
						continue;
					}
					if (ids != null && Arrays.stream(ids).noneMatch(x -> x == id)) {
						continue;
					}
					found = (Function2) entry[2];
					break;
				}
			}
			if (found == null) {
				return "none";
			}
			return found.invoke(p, npc(a));
		}
		case "dest": {
			Function2 handler = (Function2) destinations.get(0)[1];
			Location l = (Location) handler.invoke(players.get(0), npc(a));
			return Arrays.asList(l.getX(), l.getY(), l.getZ());
		}
		case "button": {
			// The tail of PacketProcessor.processIfAction: component lookup,
			// hidden check, ButtonClickEvent, InterfaceListeners.run, then the plugin.
			Component c = p.getInterfaceManager().getComponent(n(a.get("iface")));
			if (c == null) {
				return "none";
			}
			if (c.isHidden()) {
				return "hidden";
			}
			ComponentPlugin plugin = c.getPlugin();
			emit("buttonClick", index(p), c.getId(), n(a.get("button")));
			if (InterfaceListeners.run(p, c, n(a.get("opcode")), n(a.get("button")), n(a.get("slot")), n(a.get("item")))) {
				return true;
			}
			if (plugin == null) {
				return "unhandled";
			}
			return plugin.handle(p, c, n(a.get("opcode")), n(a.get("button")), n(a.get("slot")), n(a.get("item")));
		}
		case "input": {
			// PacketProcessor's InputPromptResponse branch around the unchanged RunScript.
			Object value = a.containsKey("text") ? a.get("text") : (Object) Integer.valueOf(n(a.get("int")));
			Function1 script = p.getAttribute("runscript", null);
			if (script == null) {
				return "none";
			}
			try {
				core.net.packet.in.RunScript.INSTANCE.processInput(p, value, script);
			} finally {
				p.removeAttribute("parseamount");
				p.removeAttribute("runscript");
			}
			return null;
		}
		case "tick": {
			// The modelled worker: tick listeners, then GameWorld.pulse(); a
			// failing listener skips the increment (handleTickActions' catch).
			int count = n(a.get("n"));
			Shops listener = shops;
			for (int i = 0; i < count; i++) {
				try {
					listener.tick();
					GameWorld.setTicks(GameWorld.getTicks() + 1);
				} catch (Injected e) {
					emit("tickFailure", "Injected:" + e.getMessage());
				} catch (Throwable t) {
					emit("tickFailure", t.getClass().getSimpleName());
				}
			}
			return null;
		}
		case "setTicks":
			GameWorld.setTicks(n(a.get("value")));
			return null;
		case "buy":
		case "sell": {
			Shop shop = p.getAttribute("shop", null);
			if (shop == null) {
				return "noshop";
			}
			return status(op.equals("buy") ? shop.buy(p, n(a.get("slot")), n(a.get("amount"))) : shop.sell(p, n(a.get("slot")), n(a.get("amount"))));
		}
		case "price": {
			Shop shop = p.getAttribute("shop", null);
			if (shop == null) {
				return "noshop";
			}
			if (Boolean.TRUE.equals(a.get("sell"))) {
				kotlin.Pair<Container, Item> pair = shop.getSellPrice(p, n(a.get("slot")));
				return Arrays.asList(label(pair.getFirst()), pair.getSecond().getId(), pair.getSecond().getAmount());
			}
			Item price = shop.getBuyPrice(p, n(a.get("slot")));
			return Arrays.asList(price.getId(), price.getAmount());
		}
		case "invAdd":
			return p.getInventory().add(new Item(n(a.get("id")), n(a.get("amount"))));
		case "invReplace":
			p.getInventory().replace(a.get("id") == null ? null : new Item(n(a.get("id")), n(a.get("amount"))), n(a.get("slot")));
			return null;
		case "ironman":
			p.getIronmanManager().setMode(IronmanMode.valueOf((String) a.get("mode")));
			return null;
		case "attr":
			if (a.get("value") == null) {
				p.removeAttribute((String) a.get("key"));
			} else {
				p.setAttribute((String) a.get("key"), a.get("value") instanceof Number ? (Object) n(a.get("value")) : a.get("value"));
			}
			return null;
		case "config":
			if (a.get("personalized") == null) {
				serverConfig.remove("world.personalized_shops");
			} else {
				serverConfig.put("world.personalized_shops", a.get("personalized"));
			}
			return null;
		case "close":
			return p.getInterfaceManager().close();
		case "logout":
			// The interface part of Player.finishClear: close, then the single tab.
			p.getInterfaceManager().close();
			p.getInterfaceManager().closeSingleTab();
			return null;
		default:
			throw new IllegalArgumentException(op);
		}
	}

	static Shops shops;
	static final Map<Integer, String> lastDump = new HashMap<>();

	static Integer id(Component c) {
		return c == null ? null : c.getId();
	}

	/** The occupied slots of an item array as [slot, id, amount]. */
	public static List<Object> items(Item[] array) {
		List<Object> out = new ArrayList<>();
		for (int slot = 0; slot < array.length; slot++) {
			Item item = array[slot];
			if (item != null) {
				out.add(Arrays.asList(slot, item.getId(), item.getAmount()));
			}
		}
		return out;
	}

	static TreeMap<Integer, Shop> sortedShops() {
		return new TreeMap<>(Shops.getShopsById());
	}

	/** The id under which shopsById holds this very shop (lowest id), or "other". */
	static Object shopId(Shop shop) {
		if (shop == null) {
			return null;
		}
		for (Map.Entry<Integer, Shop> e : sortedShops().entrySet()) {
			if (e.getValue() == shop) {
				return e.getKey();
			}
		}
		return "other";
	}

	static Object label(Container c) {
		if (c == null) {
			return null;
		}
		if (c == Shop.Companion.getGeneralPlayerStock()) {
			return "general";
		}
		for (Map.Entry<Integer, Shop> e : sortedShops().entrySet()) {
			for (Map.Entry<Integer, Container> i : e.getValue().getStockInstances().entrySet()) {
				if (i.getValue() == c) {
					return "stock:" + e.getKey() + ":" + i.getKey();
				}
			}
			if (e.getValue().getPlayerStock() == c) {
				return "player:" + e.getKey();
			}
		}
		for (Player p : players) {
			if (p.getInventory() == c) {
				return "inv:" + index(p);
			}
		}
		return "other";
	}

	static List<Object> listeners(Container c) {
		List<Object> out = new ArrayList<>();
		for (ContainerListener l : c.getListeners()) {
			if (l == null) {
				out.add(null);
			} else if (l instanceof ShopListener) {
				ShopListener s = (ShopListener) l;
				Player owner = s.getPlayer();
				boolean current = Shop.Companion.getListenerInstances().get(owner.getDetails().getUid()) == s;
				out.add(Arrays.asList(index(owner), current));
			} else if (l instanceof InventoryRecorder) {
				out.add("inv:" + index(((InventoryRecorder) l).owner));
			} else {
				out.add("other");
			}
		}
		return out;
	}

	static List<Object> sorted(Map<Integer, ?> map) {
		List<Object> out = new ArrayList<>();
		for (Map.Entry<Integer, ?> e : new TreeMap<>(map).entrySet()) {
			out.add(Arrays.asList(e.getKey(), e.getValue()));
		}
		return out;
	}

	static Map dump(Shop shop, boolean full) {
		Map s = new LinkedHashMap();
		if (full) {
			s.put("title", shop.getTitle());
			s.put("general", shop.getGeneral());
			s.put("currency", shop.getCurrency());
			s.put("highAlch", shop.getHighAlch());
			s.put("forceShared", shop.getForceShared());
			List<Object> stock = new ArrayList<>();
			for (ShopItem item : shop.getStock()) {
				stock.add(Arrays.asList(item.getItemId(), item.getAmount(), item.getRestockRate()));
			}
			s.put("stock", stock);
			s.put("sharedGeneral", shop.getPlayerStock() == Shop.Companion.getGeneralPlayerStock());
		}
		List<Object> instances = new ArrayList<>();
		for (Map.Entry<Integer, Container> e : shop.getStockInstances().entrySet()) {
			instances.add(Arrays.asList(e.getKey(), items(e.getValue().toArray()), listeners(e.getValue())));
		}
		s.put("instances", instances);
		s.put("needsUpdate", sorted(shop.getNeedsUpdate()));
		s.put("rates", sorted(shop.getRestockRates()));
		if (shop.getPlayerStock() != Shop.Companion.getGeneralPlayerStock()) {
			s.put("player", Arrays.asList(items(shop.getPlayerStock().toArray()), listeners(shop.getPlayerStock())));
		}
		return s;
	}

	static boolean dirty(Shop shop) {
		if (!shop.getNeedsUpdate().isEmpty()) {
			return true;
		}
		int shared = core.ServerConstants.SERVER_NAME.hashCode();
		for (Map.Entry<Integer, Container> e : shop.getStockInstances().entrySet()) {
			if (e.getKey() != shared || !e.getValue().getListeners().isEmpty()) {
				return true;
			}
		}
		Container ps = shop.getPlayerStock();
		if (ps != Shop.Companion.getGeneralPlayerStock() && (!ps.getListeners().isEmpty() || !ps.isEmpty())) {
			return true;
		}
		return false;
	}

	/** FNV-1a (64-bit) over every shop's stock, needsUpdate and own player stock, in id order. */
	static String digest() {
		StringBuilder b = new StringBuilder();
		for (Map.Entry<Integer, Shop> e : sortedShops().entrySet()) {
			Shop shop = e.getValue();
			b.append('#').append(e.getKey()).append('|');
			for (Map.Entry<Integer, Container> i : shop.getStockInstances().entrySet()) {
				b.append(i.getKey()).append('=');
				append(b, i.getValue());
				b.append(';');
			}
			b.append('|');
			for (Map.Entry<Integer, Boolean> u : new TreeMap<>(shop.getNeedsUpdate()).entrySet()) {
				b.append(u.getKey()).append(':').append(u.getValue()).append(',');
			}
			b.append('|');
			if (shop.getPlayerStock() != Shop.Companion.getGeneralPlayerStock()) {
				append(b, shop.getPlayerStock());
			}
		}
		long h = 0xcbf29ce484222325L;
		for (byte x : b.toString().getBytes(StandardCharsets.UTF_8)) {
			h ^= (x & 255);
			h *= 0x100000001b3L;
		}
		return Long.toHexString(h);
	}

	static void append(StringBuilder b, Container c) {
		for (Item item : c.toArray()) {
			if (item == null) {
				b.append("_,");
			} else {
				b.append(item.getId()).append('x').append(item.getAmount()).append(',');
			}
		}
	}

	static Map state() {
		Map out = new LinkedHashMap();
		List<Object> list = new ArrayList<>();
		for (Player p : players) {
			Map s = new LinkedHashMap();
			InterfaceManager m = p.getInterfaceManager();
			s.put("inv", items(p.getInventory().toArray()));
			s.put("opened", id(m.getOpened()));
			s.put("single", id(m.getSingleTab()));
			s.put("chatbox", id(m.getChatbox()));
			s.put("overlay", id(m.getOverlay()));
			List<Object> tabs = new ArrayList<>();
			for (Component c : m.getTabs()) {
				tabs.add(id(c));
			}
			s.put("tabs", tabs);
			s.put("count", m.getPacketCount(0));
			Map attributes = p.attributes();
			Object shop = attributes.get("shop");
			s.put("shop", shop == null ? null : shopId((Shop) shop));
			s.put("main", attributes.get("shop-main"));
			s.put("cont", label((Container) attributes.get("shop-cont")));
			s.put("runscript", attributes.get("runscript") != null);
			s.put("parseamount", attributes.get("parseamount"));
			Object type = attributes.get("input-type");
			s.put("inputType", type == null ? null : type.toString());
			s.put("ironman", p.getIronmanManager().getMode().name());
			list.add(s);
		}
		out.put("players", list);
		// Shops that are (or were) dirty, only when their dump changed since the last state.
		List<Object> dirty = new ArrayList<>();
		for (Map.Entry<Integer, Shop> e : sortedShops().entrySet()) {
			if (dirty(e.getValue()) || lastDump.containsKey(e.getKey())) {
				Map s = new LinkedHashMap();
				s.put("id", e.getKey());
				s.putAll(dump(e.getValue(), false));
				String text = JSONValue.toJSONString(s);
				if (!text.equals(lastDump.get(e.getKey()))) {
					lastDump.put(e.getKey(), text);
					dirty.add(s);
				}
			}
		}
		out.put("shops", dirty);
		Container general = Shop.Companion.getGeneralPlayerStock();
		out.put("general", Arrays.asList(items(general.toArray()), listeners(general)));
		List<Object> instances = new ArrayList<>();
		for (Map.Entry<Integer, ShopListener> e : new TreeMap<>(Shop.Companion.getListenerInstances()).entrySet()) {
			instances.add(Arrays.asList(e.getKey(), index(e.getValue().getPlayer())));
		}
		out.put("listeners", instances);
		out.put("digest", digest());
		out.put("ticks", GameWorld.getTicks());
		return out;
	}

	static Map setupResult() {
		Map out = new LinkedHashMap();
		List<Object> all = new ArrayList<>();
		for (Map.Entry<Integer, Shop> e : sortedShops().entrySet()) {
			Map s = new LinkedHashMap();
			s.put("id", e.getKey());
			s.putAll(dump(e.getValue(), true));
			all.add(s);
		}
		out.put("shops", all);
		List<Object> npcs = new ArrayList<>();
		for (Map.Entry<Integer, Shop> e : new TreeMap<>(Shops.getShopsByNpc()).entrySet()) {
			npcs.add(Arrays.asList(e.getKey(), shopId(e.getValue())));
		}
		out.put("npcs", npcs);
		return out;
	}

	interface Step {
		Object run() throws Exception;
	}

	static Map step(Step body) {
		events = new ArrayList<>();
		Map out = new LinkedHashMap();
		Object result = null;
		String error = null;
		try {
			result = body.run();
		} catch (Injected e) {
			error = "Injected:" + e.getMessage();
		} catch (Throwable t) {
			error = t.getClass().getSimpleName();
		}
		out.put("result", result);
		out.put("error", error);
		out.put("events", events);
		out.put("state", state());
		return out;
	}

	public static String run(String line) throws Throwable {
		input = (Map) new JSONParser().parse(line);
		failures = (Map) input.getOrDefault("failures", Collections.emptyMap());
		config = new String(Files.readAllBytes(Paths.get(System.getProperty("shops.config"))), StandardCharsets.UTF_8);
		events = new ArrayList<>();
		quiet = true;
		new core.game.system.config.InterfaceConfigParser().load();
		quiet = false;
		for (Object entry : ((Map) input.get("items")).entrySet()) {
			Map.Entry e = (Map.Entry) entry;
			define(Integer.parseInt((String) e.getKey()), (Map) e.getValue());
		}
		Object personalized = input.get("personalized");
		if (personalized != null) {
			serverConfig.put("world.personalized_shops", personalized);
		}
		core.ServerConstants.SERVER_NAME = (String) input.get("serverName");
		core.ServerConstants.PLAYER_STOCK_CLEAR_INTERVAL = n(input.get("stockClear"));
		core.ServerConstants.PLAYER_STOCK_RECIRCULATE = Boolean.TRUE.equals(input.get("recirculate"));
		core.ServerConstants.CONFIG_PATH = System.getProperty("shops.dir") + "/" + input.get("shopsFile") + "/";
		GameWorld.setTicks(n(input.get("ticks")));
		for (Object spec : (List) input.get("players")) {
			specs.add((Map) spec);
			players.add(new Player((Map) spec));
		}
		List<Object> out = new ArrayList<>();
		Map setup = step(() -> {
			// Class scan registration, then the start-up listener (ClassScanner order).
			shops = new Shops();
			shops.defineListeners();
			shops.defineDestinationOverrides();
			shops.defineInterfaceListeners();
			shops.defineCommands();
			shops.startup();
			for (Player p : players) {
				p.getInterfaceManager().openWindowsPane(new Component(548));
				p.getInterfaceManager().openDefaultTabs();
			}
			return setupResult();
		});
		out.add(setup);
		for (Object a : (List) input.get("actions")) {
			out.add(step(() -> act((Map) a)));
		}
		Map result = new LinkedHashMap();
		result.put("expected", out);
		return JSONValue.toJSONString(result);
	}
}
