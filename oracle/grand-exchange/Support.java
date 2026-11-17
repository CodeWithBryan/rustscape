package oracle;

import content.global.handlers.iface.ge.ExchangeItemSets;
import content.global.handlers.iface.ge.GrandExchangeInterface;
import content.global.handlers.iface.ge.StockMarket;
import core.cache.def.impl.CS2Mapping;
import core.cache.def.impl.ItemDefinition;
import core.game.component.Component;
import core.game.component.ComponentPlugin;
import core.game.container.Container;
import core.game.container.ContainerEvent;
import core.game.container.ContainerListener;
import core.game.container.access.InterfaceContainer;
import core.game.ge.GEAutoStock;
import core.game.ge.GEDB;
import core.game.ge.GEGuidePrice;
import core.game.ge.GEItemSet;
import core.game.ge.GrandExchange;
import core.game.ge.GrandExchangeOffer;
import core.game.ge.GrandExchangeRecords;
import core.game.ge.GrandExchangeTimer;
import core.game.ge.OfferState;
import core.game.ge.PriceIndex;
import core.game.interaction.InterfaceListeners;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.IronmanMode;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.sql.Connection;
import java.sql.PreparedStatement;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.concurrent.LinkedBlockingDeque;
import kotlin.jvm.functions.Function0;
import kotlin.jvm.functions.Function1;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives one Grand Exchange case against the unchanged GE, StockMarket,
 * GrandExchangeInterface, ExchangeItemSets, container, interface-manager,
 * InterfaceListeners, RunScript and packet classes over a real SQLite file.
 * Launcher loads this class and everything it reaches in a fresh class loader
 * per case, so every static (pendingOffers, GEDB.db, InterfaceContainer.index,
 * the registries) starts as a new JVM would create it.
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
	static final List<Player> players = new ArrayList<>();
	static final List<Map> specs = new ArrayList<>();
	static final List<Boolean> online = new ArrayList<>();
	static final Map<Integer, String> examines = new HashMap<>();
	static final Map<String, Function2> commands = new LinkedHashMap<>();
	static final List<Pulse> pulses = new ArrayList<>();
	static Function0 worker;
	static boolean workerDead;
	static long now;
	static long step;
	static int sharedUid;
	static Map cs2;
	static String dataPath;
	static String workDir;
	static GrandExchange ge;
	static GrandExchangeRecords hook;
	static String lastDb;

	/** A failure the case injects into a recorded call. */
	public static final class Injected extends RuntimeException {
		Injected(String message) {
			super(message);
		}
	}

	/** Thrown by the worker's queue when it is empty: the thread would wait in takeFirst. */
	public static final class Waiting extends RuntimeException {
		Waiting() {
			super("waiting", null, false, false);
		}
	}

	/** pendingOffers with takeFirst ending the worker step instead of blocking on an empty queue. */
	public static final class Queue extends LinkedBlockingDeque<GrandExchangeOffer> {
		@Override
		public GrandExchangeOffer takeFirst() throws InterruptedException {
			if (isEmpty()) {
				throw new Waiting();
			}
			return super.takeFirst();
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

	// ------------------------------------------------------------ services

	public static long clock() {
		long value = now;
		emit("clock", value);
		now = value + step;
		return value;
	}

	public static String examine(ItemDefinition definition) {
		emit("examine", definition.getId());
		return examines.getOrDefault(definition.getId(), "null");
	}

	public static PlayerDetails details(String username) {
		emit("details", username);
		return new PlayerDetails(null, sharedUid);
	}

	public static CS2Mapping cs2(int id) {
		emit("cs2", id);
		Object mapping = cs2 == null ? null : cs2.get(String.valueOf(id));
		if (mapping == null) {
			return null;
		}
		HashMap<Integer, Object> map = new HashMap<>();
		for (Object e : ((Map) mapping).entrySet()) {
			Map.Entry entry = (Map.Entry) e;
			map.put(Integer.parseInt((String) entry.getKey()), entry.getValue());
		}
		return new CS2Mapping(map);
	}

	public static List<Player> online() {
		List<Player> out = new ArrayList<>();
		for (int i = 0; i < players.size(); i++) {
			if (online.get(i)) {
				out.add(players.get(i));
			}
		}
		return out;
	}

	/** Repository.uid_map: the online players by uid (the last login wins). */
	public static final class UidMap extends HashMap<Integer, Player> {
		@Override
		public Player get(Object key) {
			emit("uidMap", key);
			Player found = null;
			for (Player p : online()) {
				if (Integer.valueOf(p.getDetails().getUid()).equals(key)) {
					found = p;
				}
			}
			return found;
		}
	}

	public static void startWorker(Function0 body) {
		emit("workerStart");
		worker = body;
	}

	public static void submit(Pulse pulse) {
		String kind = pulse.getClass().getName().startsWith("content.global.handlers.iface.ge.GrandExchangeInterface") ? "interface" : "cheat";
		emit("submit", kind, pulse.delay);
		pulses.add(pulse);
	}

	public static void command(String name, String privilege, String usage, String description, Function2 handle) {
		emit("command", name, privilege, usage, description);
		commands.put(name, handle);
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

	// ---------------------------------------------------------- database

	/** The provider's connection, recording each statement with its JDBC call and bound parameters. */
	public static Connection wrap(Connection real) {
		return (Connection) Proxy.newProxyInstance(Support.class.getClassLoader(), new Class[] {Connection.class}, (proxy, method, args) -> {
			Object result = invoke(real, method, args);
			if (method.getName().equals("prepareStatement")) {
				return statement((Statement) result, (String) args[0], PreparedStatement.class);
			}
			if (method.getName().equals("createStatement")) {
				return statement((Statement) result, null, Statement.class);
			}
			return result;
		});
	}

	static Object invoke(Object target, Method method, Object[] args) throws Throwable {
		try {
			return method.invoke(target, args);
		} catch (InvocationTargetException e) {
			throw e.getCause();
		}
	}

	static Object statement(Statement real, String prepared, Class<?> type) {
		TreeMap<Integer, Object> params = new TreeMap<>();
		InvocationHandler handler = (proxy, method, args) -> {
			String name = method.getName();
			if (name.equals("setInt") || name.equals("setLong") || name.equals("setString") || name.equals("setBoolean")) {
				String tag = name.equals("setInt") ? "i" : name.equals("setLong") ? "l" : name.equals("setString") ? "s" : "b";
				params.put(n(args[0]), Arrays.asList(tag, args[1]));
			}
			if (name.equals("executeQuery") || name.equals("executeUpdate") || name.equals("execute")) {
				String kind = name.equals("executeQuery") ? "query" : name.equals("executeUpdate") ? "update" : "execute";
				String sql = prepared != null ? prepared : (String) args[0];
				emit("sql", kind, sql, new ArrayList<>(params.values()));
			}
			return invoke(real, method, args);
		};
		return Proxy.newProxyInstance(Support.class.getClassLoader(), new Class[] {type}, handler);
	}

	/** An unrecorded connection of the harness (the dump and seeds), not the provider's. */
	static Connection raw() throws Exception {
		org.sqlite.SQLiteDataSource ds = new org.sqlite.SQLiteDataSource();
		ds.setUrl("jdbc:sqlite:" + dbFile());
		return ds.getConnection();
	}

	static String dbFile() {
		return dataPath + "grandexchange.db";
	}

	/** Every table in sqlite_master order, each row as rowid and quote() of every column. */
	static Object dumpDb() throws Exception {
		if (!new File(dbFile()).isFile()) {
			return null;
		}
		Map out = new LinkedHashMap();
		try (Connection c = raw(); Statement s = c.createStatement()) {
			List<Object> master = new ArrayList<>();
			List<String> tables = new ArrayList<>();
			try (ResultSet r = s.executeQuery("SELECT type, name, tbl_name, sql FROM sqlite_master ORDER BY rowid")) {
				while (r.next()) {
					master.add(Arrays.asList(r.getString(1), r.getString(2), r.getString(3), r.getString(4)));
					if ("table".equals(r.getString(1))) {
						tables.add(r.getString(2));
					}
				}
			}
			out.put("master", master);
			for (String table : tables) {
				List<String> columns = new ArrayList<>();
				try (ResultSet r = s.executeQuery("PRAGMA table_info(" + table + ")")) {
					while (r.next()) {
						columns.add(r.getString(2));
					}
				}
				StringBuilder q = new StringBuilder("SELECT quote(rowid)");
				for (String column : columns) {
					q.append(", quote(").append(column).append(')');
				}
				q.append(" FROM ").append(table).append(" ORDER BY rowid");
				List<Object> rows = new ArrayList<>();
				try (ResultSet r = s.executeQuery(q.toString())) {
					while (r.next()) {
						List<Object> row = new ArrayList<>();
						for (int i = 1; i <= columns.size() + 1; i++) {
							row.add(r.getString(i));
						}
						rows.add(row);
					}
				}
				out.put(table, rows);
			}
		}
		return out;
	}

	static void rawSql(List statements) throws Exception {
		new File(dbFile()).getParentFile().mkdirs();
		try (Connection c = raw(); Statement s = c.createStatement()) {
			for (Object sql : statements) {
				s.executeUpdate((String) sql);
			}
		}
	}

	// ------------------------------------------------------------- set-up

	static void field(Object target, Class<?> owner, String name, Object value) throws ReflectiveOperationException {
		Field f = owner.getDeclaredField(name);
		f.setAccessible(true);
		f.set(target, value);
	}

	static Object read(Object target, Class<?> owner, String name) throws ReflectiveOperationException {
		Field f = owner.getDeclaredField(name);
		f.setAccessible(true);
		return f.get(target);
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
		for (String key : new String[] {"high_alchemy", "low_alchemy"}) {
			if (spec.containsKey(key)) {
				d.getHandlers().put(key, n(spec.get(key)));
			}
		}
		examines.put(id, (String) spec.get("examine"));
		ItemDefinition.getDefinitions().put(id, d);
	}

	static void constants(Map c) {
		if (c.containsKey("cheater")) {
			core.ServerConstants.I_AM_A_CHEATER = Boolean.TRUE.equals(c.get("cheater"));
		}
		if (c.containsKey("influence")) {
			core.ServerConstants.BOTS_INFLUENCE_PRICE_INDEX = Boolean.TRUE.equals(c.get("influence"));
		}
		if (c.containsKey("limit")) {
			core.ServerConstants.BOTSTOCK_LIMIT = n(c.get("limit"));
		}
		if (c.containsKey("autostock")) {
			core.ServerConstants.GE_AUTOSTOCK_ENABLED = Boolean.TRUE.equals(c.get("autostock"));
		}
	}

	static Field indexField() throws ReflectiveOperationException {
		Field f = InterfaceContainer.class.getDeclaredField("index");
		f.setAccessible(true);
		return f;
	}

	/** Replaces GrandExchange.pendingOffers (a static final) with the stopping queue before first use. */
	static void installQueue() throws Exception {
		Class.forName("core.game.ge.GrandExchange", true, Support.class.getClassLoader());
		Field f = GrandExchange.class.getDeclaredField("pendingOffers");
		Field theUnsafe = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
		theUnsafe.setAccessible(true);
		Object unsafe = theUnsafe.get(null);
		Class<?> u = unsafe.getClass();
		Object base = u.getMethod("staticFieldBase", Field.class).invoke(unsafe, f);
		long offset = (Long) u.getMethod("staticFieldOffset", Field.class).invoke(unsafe, f);
		u.getMethod("putObject", Object.class, long.class, Object.class).invoke(unsafe, base, offset, new Queue());
	}

	static Map setup() throws Throwable {
		new StockMarket().defineInterfaceListeners();
		new ExchangeItemSets().defineInterfaceListeners();
		new GrandExchangeInterface().newInstance(null);
		ge = new GrandExchange();
		ge.defineCommands();
		hook = new GrandExchangeRecords();
		for (Player p : players) {
			// LoginConfiguration's order: the pane, the chatbox, the default tabs.
			p.getInterfaceManager().openWindowsPane(new Component(548));
			p.getInterfaceManager().openChatbox(137);
			p.getInterfaceManager().openDefaultTabs();
		}
		String startup = (String) input.getOrDefault("startup", "full");
		if (startup.equals("full")) {
			ge.startup();
		} else if (startup.equals("init")) {
			GEDB.INSTANCE.init();
		}
		for (int i = 0; i < players.size(); i++) {
			if (!Boolean.FALSE.equals(specs.get(i).get("login"))) {
				hook.login(players.get(i));
			}
		}
		Map out = new LinkedHashMap();
		List<Object> sets = new ArrayList<>();
		for (GEItemSet set : GEItemSet.values()) {
			List<Object> parts = new ArrayList<>();
			for (int id : set.getComponents()) {
				parts.add(id);
			}
			sets.add(Arrays.asList(set.name(), set.getItemId(), parts));
		}
		out.put("sets", sets);
		List<Object> array = new ArrayList<>();
		for (Item item : GEItemSet.getItemArray()) {
			array.add(Arrays.asList(item.getId(), item.getAmount()));
		}
		out.put("setItems", array);
		List<Object> guides = new ArrayList<>();
		for (GEGuidePrice.GuideType type : GEGuidePrice.GuideType.values()) {
			List<Object> items = new ArrayList<>();
			for (GEGuidePrice.GuideItem item : type.getItems()) {
				items.add(Arrays.asList(item.getItem(), item.getChildData()[0]));
			}
			guides.add(Arrays.asList(type.name(), type.getChildData()[0], type.getChildData()[1], items));
		}
		out.put("guides", guides);
		List<Object> states = new ArrayList<>();
		for (OfferState s : OfferState.values()) {
			states.add(s.name());
		}
		out.put("states", states);
		return out;
	}

	// ------------------------------------------------------------ actions

	static Player player(Map a, String key) {
		return players.get(n(a.get(key)));
	}

	static GrandExchangeOffer offerSpec(Map spec) {
		GrandExchangeOffer o = new GrandExchangeOffer();
		o.setItemID(n(spec.getOrDefault("item", 0)));
		o.setAmount(n(spec.getOrDefault("amount", 0)));
		o.setOfferedValue(n(spec.getOrDefault("value", 0)));
		o.setSell(Boolean.TRUE.equals(spec.get("sell")));
		o.setIndex(n(spec.getOrDefault("index", 0)));
		o.setUid(((Number) spec.getOrDefault("uid", 0)).longValue());
		o.setOfferState(OfferState.valueOf((String) spec.getOrDefault("state", "PENDING")));
		return o;
	}

	static Object act(Map a) throws Throwable {
		String op = (String) a.get("op");
		Player p = a.containsKey("p") && a.get("p") != null ? player(a, "p") : null;
		switch (op) {
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
			int opcode = n(a.get("opcode")), button = n(a.get("button")), slot = n(a.get("slot")), item = n(a.getOrDefault("item", -1));
			if (InterfaceListeners.run(p, c, opcode, button, slot, item)) {
				return true;
			}
			if (plugin == null) {
				return "unhandled";
			}
			return plugin.handle(p, c, opcode, button, slot, item);
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
		case "pulse": {
			// The submitted pulses, in order; a pulse that is not finished stays.
			List<Object> out = new ArrayList<>();
			List<Pulse> due = new ArrayList<>(pulses);
			pulses.clear();
			for (int i = 0; i < due.size(); i++) {
				boolean done;
				try {
					done = due.get(i).pulse();
				} catch (Throwable t) {
					pulses.addAll(due.subList(i + 1, due.size()));
					throw t;
				}
				out.add(done);
				if (!done) {
					pulses.add(due.get(i));
				}
			}
			return out;
		}
		case "worker": {
			if (worker == null) {
				return "notStarted";
			}
			if (workerDead) {
				return "dead";
			}
			try {
				worker.invoke();
			} catch (Waiting w) {
				return "waiting";
			} catch (Throwable t) {
				workerDead = true;
				throw t;
			}
			return "returned";
		}
		case "boot":
			ge.boot();
			return null;
		case "timer":
			return new GrandExchangeTimer().run(p != null ? p : new NPC(1, null, Direction.SOUTH, new Location(0, 0, 0)));
		case "openFor":
			StockMarket.openFor(p);
			return null;
		case "open":
			core.api.ContentAPIKt.openInterface(p, n(a.get("iface")));
			return null;
		case "close":
			return p.getInterfaceManager().close();
		case "closeChatbox":
			p.getInterfaceManager().closeChatbox();
			return null;
		case "setOfferItem":
			core.net.packet.PacketProcessor.process(new core.net.packet.in.Packet.GESetOfferItem(p, n(a.get("item"))));
			return null;
		case "collection":
			GrandExchangeRecords.getInstance(p).openCollectionBox();
			return null;
		case "history":
			GrandExchangeRecords.getInstance(p).openHistoryLog(p);
			return null;
		case "guide":
			GEGuidePrice.open(p, GEGuidePrice.GuideType.valueOf((String) a.get("type")));
			return null;
		case "sets":
			ExchangeItemSets.openFor(p);
			return null;
		case "login":
			hook.login(p);
			return null;
		case "parse":
			hook.parsePlayer(p, (JSONObject) new JSONParser().parse((String) a.get("json")));
			return null;
		case "save": {
			JSONObject save = new JSONObject();
			hook.savePlayer(p, save);
			return save.toJSONString();
		}
		case "botOffer":
			return GrandExchange.Companion.addBotOffer(n(a.get("item")), n(a.get("amount")));
		case "autostock": {
			File file = new File(workDir, "data" + File.separator + "eco" + File.separator + "autostock.json");
			file.getParentFile().mkdirs();
			if (a.get("file") == null) {
				file.delete();
			} else {
				Files.write(file.toPath(), ((String) a.get("file")).getBytes(StandardCharsets.UTF_8));
			}
			GEAutoStock.Companion.autostock();
			return null;
		}
		case "stats":
			return GrandExchange.getOfferStats(n(a.get("item")), Boolean.TRUE.equals(a.get("sale")));
		case "price":
			return GrandExchange.getRecommendedPrice(n(a.get("item")), Boolean.TRUE.equals(a.get("bot")));
		case "valid": {
			List<Object> out = new ArrayList<>();
			for (GrandExchangeOffer o : GrandExchange.Companion.getValidOffers()) {
				out.add(summary(o, null));
			}
			return out;
		}
		case "bots": {
			List<Object> out = new ArrayList<>();
			for (GrandExchangeOffer o : GrandExchange.Companion.getBotOffers()) {
				out.add(summary(o, null));
			}
			return out;
		}
		case "botStock":
			return GrandExchange.Companion.getBotstockForId(n(a.get("item")));
		case "canTrade":
			return PriceIndex.canTrade(n(a.get("item")));
		case "ban":
			PriceIndex.INSTANCE.banItem(n(a.get("item")));
			return null;
		case "allow":
			PriceIndex.INSTANCE.allowItem(n(a.get("item")));
			return null;
		case "value":
			return PriceIndex.INSTANCE.getValue(n(a.get("item")));
		case "addTrade":
			PriceIndex.INSTANCE.addTrade(n(a.get("item")), n(a.get("amount")), n(a.get("price")));
			return null;
		case "dispatch":
			return GrandExchange.Companion.dispatch(p, offerSpec((Map) a.get("offer")));
		case "command": {
			Function2 handler = commands.get(a.get("name"));
			String[] strings = ((List<String>) a.get("args")).toArray(new String[0]);
			handler.invoke(p, strings);
			return null;
		}
		case "awaiting":
			return core.api.ContentAPIKt.hasAwaitingGrandExchangeCollections(p);
		case "active":
			return GrandExchangeRecords.getInstance(p).hasActiveOffer();
		case "format":
			return GrandExchangeRecords.getInstance(p).format();
		case "cacheValue": {
			GrandExchangeOffer o = GrandExchangeRecords.getInstance(p).getOffer(n(a.get("slot")));
			return o == null ? null : o.cacheValue();
		}
		case "invReplace":
			p.getInventory().replace(a.get("id") == null ? null : new Item(n(a.get("id")), n(a.get("amount"))), n(a.get("slot")));
			return null;
		case "invAdd":
			return p.getInventory().add(new Item(n(a.get("id")), n(a.get("amount"))));
		case "attr":
			if (a.get("value") == null) {
				p.removeAttribute((String) a.get("key"));
			} else {
				p.setAttribute((String) a.get("key"), n(a.get("value")));
			}
			return null;
		case "sql":
			rawSql((List) a.get("statements"));
			return null;
		case "clock":
			now = ((Number) a.get("now")).longValue();
			step = ((Number) a.get("step")).longValue();
			return null;
		case "constants":
			constants(a);
			return null;
		case "online":
			online.set(n(a.get("p")), Boolean.TRUE.equals(a.get("value")));
			return null;
		case "pin":
			p.pinUnlocked = Boolean.TRUE.equals(a.get("value"));
			return null;
		case "ironman":
			p.getIronmanManager().setMode(IronmanMode.valueOf((String) a.get("mode")));
			return null;
		default:
			throw new IllegalArgumentException(op);
		}
	}

	// -------------------------------------------------------------- state

	static IdentityHashMap<Object, String> labels;

	static Object label(Object o, String prefix) {
		if (o == null) {
			return null;
		}
		String found = labels.get(o);
		if (found == null) {
			found = prefix + (labels.size() + 1);
			labels.put(o, found);
		}
		return found;
	}

	static Object summary(GrandExchangeOffer o, Object label) {
		List<Object> withdraw = new ArrayList<>();
		for (Item item : o.getWithdraw()) {
			withdraw.add(item == null ? null : Arrays.asList(item.getId(), item.getAmount()));
		}
		return Arrays.asList(label, o.getItemID(), o.getAmount(), o.getCompletedAmount(), o.getOfferedValue(), o.getIndex(), o.getSell(),
				o.getOfferState().name(), o.getUid(), o.getTimeStamp(), withdraw, o.getTotalCoinExchange(), index(o.getPlayer()), o.getPlayerUID(),
				o.isLimitation(), o.isBot());
	}

	static List<Object> offerTable;

	static Object offer(GrandExchangeOffer o) {
		if (o == null) {
			return null;
		}
		boolean known = labels.containsKey(o);
		Object l = label(o, "o");
		if (!known) {
			offerTable.add(summary(o, l));
		}
		return l;
	}

	static Object records(GrandExchangeRecords r) throws ReflectiveOperationException {
		if (r == null) {
			return null;
		}
		List<Object> slots = new ArrayList<>();
		for (GrandExchangeRecords.OfferRecord record : r.getOfferRecords()) {
			slots.add(record == null ? null : Arrays.asList(record.getUid(), record.getSlot()));
		}
		List<Object> history = new ArrayList<>();
		for (GrandExchangeOffer o : r.getHistory()) {
			history.add(offer(o));
		}
		Player owner = (Player) read(r, GrandExchangeRecords.class, "player");
		return Arrays.asList(label(r, "r"), owner == null ? null : index(owner), slots, history, r.getUpdateNotification());
	}

	static Integer id(Component c) {
		return c == null ? null : c.getId();
	}

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

	static List<Object> listeners(Player p) throws ReflectiveOperationException {
		List<Object> out = new ArrayList<>();
		Object attribute = p.attributes().get("ge-listener");
		for (ContainerListener l : p.getInventory().getListeners()) {
			if (l == null) {
				out.add(null);
			} else if (l instanceof InventoryRecorder) {
				out.add("rec:" + index(((InventoryRecorder) l).owner));
			} else if (l.getClass().getName().startsWith("content.global.handlers.iface.ge.ExchangeItemSets")) {
				Player owner = (Player) read(l, l.getClass(), "player");
				out.add(Arrays.asList("sets", index(owner), l == attribute));
			} else {
				out.add("other");
			}
		}
		return out;
	}

	static Map state() throws Exception {
		labels = new IdentityHashMap<>();
		offerTable = new ArrayList<>();
		Map out = new LinkedHashMap();
		List<Object> list = new ArrayList<>();
		for (Player p : players) {
			Map s = new LinkedHashMap();
			InterfaceManager m = p.getInterfaceManager();
			s.put("inv", items(p.getInventory().toArray()));
			s.put("listeners", listeners(p));
			s.put("opened", id(m.getOpened()));
			s.put("single", id(m.getSingleTab()));
			s.put("chatbox", id(m.getChatbox()));
			s.put("count", m.getPacketCount(0));
			Map attributes = p.attributes();
			s.put("geIndex", attributes.get("ge-index"));
			s.put("temp", offer((GrandExchangeOffer) attributes.get("ge-temp")));
			s.put("records", records((GrandExchangeRecords) attributes.get("ge-records")));
			Object guide = attributes.get("guide-price");
			s.put("guide", guide == null ? null : ((GEGuidePrice.GuideType) guide).name());
			s.put("key", attributes.get("container-key"));
			s.put("setsListener", attributes.get("ge-listener") != null);
			s.put("runscript", attributes.get("runscript") != null);
			s.put("parseamount", attributes.get("parseamount"));
			Object type = attributes.get("input-type");
			s.put("inputType", type == null ? null : type.toString());
			s.put("ironman", p.getIronmanManager().getMode().name());
			list.add(s);
		}
		out.put("players", list);
		List<Object> pending = new ArrayList<>();
		for (GrandExchangeOffer o : GrandExchange.Companion.getPendingOffers()) {
			pending.add(offer(o));
		}
		out.put("pending", pending);
		out.put("hook", records(hook));
		out.put("offers", offerTable);
		List<Object> due = new ArrayList<>();
		for (Pulse pulse : pulses) {
			due.add(pulse.getClass().getName().startsWith("content.global.handlers.iface.ge.GrandExchangeInterface") ? "interface" : "cheat");
		}
		out.put("pulses", due);
		out.put("worker", worker == null ? "NotStarted" : workerDead ? "Dead" : "Waiting");
		out.put("index", indexField().get(null));
		String db = JSONValue.toJSONString(dumpDb());
		if (db.equals(lastDb)) {
			out.put("db", "same");
		} else {
			lastDb = db;
			out.put("db", JSONValue.parse(db));
		}
		return out;
	}

	interface Step {
		Object run() throws Throwable;
	}

	static Map step(Step body) throws Throwable {
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
		config = new String(Files.readAllBytes(Paths.get(System.getProperty("ge.config"))), StandardCharsets.UTF_8);
		workDir = System.getProperty("user.dir");
		dataPath = new File(System.getProperty("ge.db"), String.valueOf(input.get("name"))).getAbsolutePath() + File.separator;
		events = new ArrayList<>();
		quiet = true;
		new core.game.system.config.InterfaceConfigParser().load();
		quiet = false;
		installQueue();
		TreeSet<Integer> ids = new TreeSet<>();
		Map items = (Map) input.get("items");
		for (Object key : items.keySet()) {
			ids.add(Integer.parseInt((String) key));
		}
		for (int id : ids) {
			define(id, (Map) items.get(String.valueOf(id)));
		}
		constants((Map) input.getOrDefault("constants", Collections.emptyMap()));
		core.ServerConstants.GRAND_EXCHANGE_DATA_PATH = dataPath;
		now = ((Number) input.getOrDefault("clock", 1000000L)).longValue();
		step = ((Number) input.getOrDefault("step", 1L)).longValue();
		sharedUid = n(input.getOrDefault("sharedUid", 0));
		cs2 = (Map) input.get("cs2");
		if (input.containsKey("containerIndex")) {
			indexField().set(null, n(input.get("containerIndex")));
		}
		new File(dbFile()).delete();
		if (input.containsKey("seed")) {
			rawSql((List) input.get("seed"));
		}
		for (Object spec : (List) input.get("players")) {
			specs.add((Map) spec);
			players.add(new Player((Map) spec));
			online.add(!Boolean.FALSE.equals(((Map) spec).get("online")));
		}
		List<Object> out = new ArrayList<>();
		out.add(step(Support::setup));
		for (Object a : (List) input.get("actions")) {
			out.add(step(() -> act((Map) a)));
		}
		Map result = new LinkedHashMap();
		result.put("expected", out);
		return JSONValue.toJSONString(result);
	}
}
