package oracle;

import content.global.handlers.iface.TradeInterfacePlugin;
import core.cache.def.impl.ItemDefinition;
import core.game.bots.AIRepository;
import core.game.bots.GeneralBotCreator;
import core.game.component.Component;
import core.game.component.ComponentPlugin;
import core.game.container.Container;
import core.game.container.ContainerEvent;
import core.game.container.ContainerListener;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.IronmanMode;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.request.RequestType;
import core.game.node.entity.player.link.request.trade.TradeModule;
import core.game.node.item.Item;
import core.game.system.task.Pulse;
import core.game.world.GameSettings;
import core.game.world.GameWorld;
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
import kotlin.jvm.functions.Function1;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives one trade case against the unchanged request, trade, container,
 * interface-manager and packet classes. Launcher loads this class and
 * everything it reaches in a fresh class loader per case, so TradeModule's
 * shared companion components and every static table start as a new JVM
 * would create them.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public final class Support {
	static Map input;
	static List<Object> events;
	static final Map<String, Integer> calls = new HashMap<>();
	static Map failures;
	public static String config;
	static final List<Player> players = new ArrayList<>();
	static final List<TradeModule> modules = new ArrayList<>();
	static final List<Object[]> pulses = new ArrayList<>();
	static final Map<Integer, String> examines = new HashMap<>();

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

	public static List<Object> items(Item[] array) {
		List<Object> out = new ArrayList<>();
		if (array == null) {
			return null;
		}
		for (Item item : array) {
			out.add(item == null ? null : Arrays.asList(item.getId(), item.getAmount()));
		}
		return out;
	}

	public static String examine(ItemDefinition definition) {
		emit("examine", definition.getId());
		return examines.get(definition.getId());
	}

	public static boolean zone(Player p, RequestType type, Player target) {
		emit("zone", index(p), type == RequestType.Companion.getTRADE() ? "TRADE" : "ASSIST", index(target));
		List refuse = (List) input.getOrDefault("zoneRefuse", Collections.emptyList());
		for (Object entry : refuse) {
			if (n(entry) == index(p)) {
				return false;
			}
		}
		return true;
	}

	public static void module(TradeModule module) {
		if (!modules.contains(module)) {
			modules.add(module);
		}
	}

	public static void pulse(Pulse pulse) {
		try {
			Field slot = pulse.getClass().getDeclaredField("val$slot");
			slot.setAccessible(true);
			Player owner = (Player) pulse.checks[0];
			emit("alertSubmit", index(owner), slot.getInt(pulse));
			pulses.add(new Object[] {owner, slot.getInt(pulse), pulse});
		} catch (ReflectiveOperationException e) {
			throw new IllegalStateException(e);
		}
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
		if (spec.containsKey("tradeable")) {
			d.getHandlers().put("tradeable", spec.get("tradeable"));
		}
		examines.put(id, (String) spec.get("examine"));
		ItemDefinition.getDefinitions().put(id, d);
	}

	static Player player(Map a, String key) {
		return players.get(n(a.get(key)));
	}

	static Object act(Map a) throws Exception {
		String op = (String) a.get("op");
		Player p = a.containsKey("p") ? player(a, "p") : null;
		switch (op) {
		case "request": {
			RequestType type = "assist".equals(a.get("kind")) ? RequestType.Companion.getASSIST() : RequestType.Companion.getTRADE();
			return p.getRequestManager().request(player(a, "t"), type);
		}
		case "button": {
			// The tail of PacketProcessor.processIfAction: component lookup,
			// hidden check, then the component's plugin.
			Component c = p.getInterfaceManager().getComponent(n(a.get("iface")));
			if (c == null) {
				return "none";
			}
			if (c.isHidden()) {
				return "hidden";
			}
			ComponentPlugin plugin = c.getPlugin();
			if (plugin == null) {
				return "noplugin";
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
		case "tick":
			GameWorld.setTicks(GameWorld.getTicks() + n(a.get("n")));
			return null;
		case "pulse": {
			List<Object[]> run = new ArrayList<>();
			for (Object[] entry : pulses) {
				if (entry[0] == p) {
					run.add(entry);
				}
			}
			pulses.removeAll(run);
			for (Object[] entry : run) {
				((Pulse) entry[2]).pulse();
			}
			return run.size();
		}
		case "move":
			p.setLocation(new Location(n(a.get("x")), n(a.get("y")), n(a.get("z"))));
			return null;
		case "active":
			p.setActive((Boolean) a.get("value"));
			return null;
		case "ironman":
			p.getIronmanManager().setMode(IronmanMode.valueOf((String) a.get("mode")));
			return null;
		case "attr":
			if (a.get("value") == null) {
				p.removeAttribute((String) a.get("key"));
			} else {
				p.setAttribute((String) a.get("key"), n(a.get("value")));
			}
			return null;
		case "invAdd":
			return p.getInventory().add(new Item(n(a.get("id")), n(a.get("amount"))));
		case "invReplace":
			p.getInventory().replace(a.get("id") == null ? null : new Item(n(a.get("id")), n(a.get("amount"))), n(a.get("slot")));
			return null;
		case "close":
			return p.getInterfaceManager().close();
		case "open":
			p.getInterfaceManager().open(new Component(n(a.get("id"))));
			return null;
		case "logout":
			// The interface part of Player.finishClear: close, then the single tab.
			p.getInterfaceManager().close();
			p.getInterfaceManager().closeSingleTab();
			p.setActive(false);
			return null;
		default:
			throw new IllegalArgumentException(op);
		}
	}

	static Integer id(Component c) {
		return c == null ? null : c.getId();
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
			TradeModule module = TradeModule.getExtension(p);
			s.put("module", module == null ? null : modules.indexOf(module));
			s.put("target", index(p.getRequestManager().getTarget()));
			Object last = p.getAttribute("lastRequest", null);
			s.put("last", last == null ? null : last == RequestType.Companion.getTRADE() ? "TRADE" : "ASSIST");
			for (String key : new String[] {"busy", "alert", "alertSlot", "parseamount"}) {
				s.put(key, p.getAttribute(key, null));
			}
			s.put("runscript", p.getAttribute("runscript", null) != null);
			Object type = p.getAttribute("input-type", null);
			s.put("inputType", type == null ? null : type.toString());
			s.put("active", p.isActive());
			list.add(s);
		}
		out.put("players", list);
		List<Object> trades = new ArrayList<>();
		for (TradeModule m : modules) {
			Map t = new LinkedHashMap();
			t.put("owner", index(m.getPlayer()));
			t.put("target", index(m.getTarget()));
			t.put("stage", m.getStage());
			t.put("accepted", m.isAccepted());
			t.put("modified", m.isModified());
			t.put("retained", m.isRetained());
			t.put("items", items(m.getContainer().toArray()));
			trades.add(t);
		}
		out.put("modules", trades);
		List<Object> pending = new ArrayList<>();
		for (Object[] entry : pulses) {
			pending.add(Arrays.asList(index((Player) entry[0]), entry[1]));
		}
		out.put("pulses", pending);
		out.put("ticks", GameWorld.getTicks());
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
		config = new String(Files.readAllBytes(Paths.get(System.getProperty("trade.config"))), StandardCharsets.UTF_8);
		events = new ArrayList<>();
		new core.game.system.config.InterfaceConfigParser().load();
		for (Object entry : ((Map) input.get("items")).entrySet()) {
			Map.Entry e = (Map.Entry) entry;
			define(Integer.parseInt((String) e.getKey()), (Map) e.getValue());
		}
		new TradeInterfacePlugin().newInstance(null);
		RequestType.Companion.getTRADE();
		GameWorld.setTicks(n(input.get("ticks")));
		GameWorld.setSettings(new GameSettings(Boolean.TRUE.equals(input.get("dev"))));
		Map bots = (Map) input.getOrDefault("bots", Collections.emptyMap());
		GeneralBotCreator creator = new GeneralBotCreator();
		for (Object entry : bots.entrySet()) {
			Map.Entry e = (Map.Entry) entry;
			core.game.bots.Script script = "doubling".equals(e.getValue()) ? new content.global.bots.DoublingMoney() : new OtherScript();
			if (script instanceof content.global.bots.DoublingMoney) {
				((content.global.bots.DoublingMoney) script).name = (String) e.getKey();
			}
			AIRepository.Companion.getPulseRepository().put((String) e.getKey(), creator.new BotScriptPulse(script, false));
		}
		int index = 0;
		for (Object spec : (List) input.get("players")) {
			players.add(new Player(index++, (Map) spec));
		}
		List<Object> out = new ArrayList<>();
		out.add(step(() -> {
			for (Player p : players) {
				p.getInterfaceManager().openWindowsPane(new Component(548));
				p.getInterfaceManager().openDefaultTabs();
			}
			return null;
		}));
		for (Object a : (List) input.get("actions")) {
			out.add(step(() -> act((Map) a)));
		}
		Map result = new LinkedHashMap();
		result.put("expected", out);
		return JSONValue.toJSONString(result);
	}

	/** A bot script that is not DoublingMoney. */
	public static final class OtherScript extends core.game.bots.Script {
	}
}
