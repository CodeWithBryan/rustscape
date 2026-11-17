package oracle;

import content.region.wilderness.handlers.revenants.RevenantNPC;
import core.ServerConstants;
import core.api.LogoutListener;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.system.communication.ClanRepository;
import core.game.world.GameSettings;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import core.game.world.repository.DisconnectionQueue;
import core.game.world.repository.NodeList;
import core.game.world.repository.Repository;
import core.net.IoSession;
import java.io.IOException;
import java.lang.reflect.Field;
import java.net.Socket;
import java.net.SocketAddress;
import java.net.SocketOption;
import java.nio.ByteBuffer;
import java.nio.channels.SelectableChannel;
import java.nio.channels.SelectionKey;
import java.nio.channels.Selector;
import java.nio.channels.SocketChannel;
import java.nio.channels.spi.SelectorProvider;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.NoSuchElementException;
import java.util.Set;
import java.util.TreeMap;
import kotlin.Unit;
import kotlin.jvm.functions.Function1;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives one case against the unchanged NodeList, NodeListIterator, Repository,
 * DisconnectionQueue, TaskExecutor and TickUtils. Launcher loads this class and
 * everything it reaches in a fresh class loader per case, so the Repository
 * object and its maps start as a new JVM would create them.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public final class Support {
	static Map input;
	static List<Object> events;
	static Map<String, Integer> calls;
	static boolean quiet;
	public static int ticks;
	static final Map<String, Player> players = new TreeMap<>();
	static final Map<String, NPC> npcs = new TreeMap<>();
	static final Map<String, IoSession> sessions = new TreeMap<>();
	static final Map<String, Map> properties = new HashMap<>();
	static final List<Runnable> tasks = new ArrayList<>();
	static final List<Player> renderable = new ArrayList<>();
	static final List<NPC> chunk = new ArrayList<>();
	static final Map<String, ClanRepository> clans = new HashMap<>();
	static final Map<String, Iterator<? extends Node>> iterators = new HashMap<>();
	static GameSettings settings;

	/** A failure the case injects into a recorded call. */
	public static final class Injected extends RuntimeException {
		Injected(String message) {
			super(message);
		}

		@Override
		public void printStackTrace() {
			emit("stackTrace", "exception:" + getMessage());
		}
	}

	public static final class InjectedError extends Error {
		InjectedError(String message) {
			super(message);
		}

		@Override
		public void printStackTrace() {
			emit("stackTrace", "error:" + getMessage());
		}
	}

	public static final class InjectedIo extends IOException {
		InjectedIo(String message) {
			super(message);
		}

		@Override
		public void printStackTrace() {
			emit("stackTrace", "io:" + getMessage());
		}
	}

	/** UpdateSequence.renderablePlayers as far as this corpus needs it: a recorded removal. */
	public static final class RenderList {
		public boolean remove(Object player) {
			emit("renderableRemove", ((Node) player).label);
			return renderable.remove(player);
		}
	}

	static final class Key extends SelectionKey {
		final String label;
		final Channel channel;
		boolean valid = true;

		Key(String label) {
			this.label = label;
			this.channel = new Channel(label);
		}

		@Override public SelectableChannel channel() { return channel; }
		@Override public Selector selector() { return null; }
		@Override public boolean isValid() { return valid; }
		@Override public void cancel() { valid = false; emit("keyCancel", label); }
		@Override public int interestOps() { return 0; }
		@Override public SelectionKey interestOps(int ops) { return this; }
		@Override public int readyOps() { return 0; }
	}

	static final class Channel extends SocketChannel {
		final Socket socket;

		Channel(String label) {
			super(SelectorProvider.provider());
			socket = new Socket() {
				@Override
				public synchronized void close() throws IOException {
					emitIo("socketClose", label);
				}
			};
		}

		@Override public SocketChannel bind(SocketAddress local) { return this; }
		@Override public <T> SocketChannel setOption(SocketOption<T> name, T value) { return this; }
		@Override public <T> T getOption(SocketOption<T> name) { return null; }
		@Override public SocketChannel shutdownInput() { return this; }
		@Override public SocketChannel shutdownOutput() { return this; }
		@Override public Socket socket() { return socket; }
		@Override public boolean isConnected() { return true; }
		@Override public boolean isConnectionPending() { return false; }
		@Override public boolean connect(SocketAddress remote) { return true; }
		@Override public boolean finishConnect() { return true; }
		@Override public SocketAddress getRemoteAddress() { return null; }
		@Override public int read(ByteBuffer dst) { return -1; }
		@Override public long read(ByteBuffer[] dsts, int offset, int length) { return -1; }
		@Override public int write(ByteBuffer src) { return src.remaining(); }
		@Override public long write(ByteBuffer[] srcs, int offset, int length) { return 0; }
		@Override public SocketAddress getLocalAddress() { return null; }
		@Override public Set<SocketOption<?>> supportedOptions() { return Collections.emptySet(); }
		@Override protected void implCloseSelectableChannel() { }
		@Override protected void implConfigureBlocking(boolean block) { }
	}

	static int n(Object value) {
		return ((Number) value).intValue();
	}

	static Object or(Map map, String key, Object fallback) {
		Object value = map.get(key);
		return value == null ? fallback : value;
	}

	static boolean truth(Object value) {
		return Boolean.TRUE.equals(value);
	}

	// What the adapters ask.

	public static boolean flag(Object node, String key) {
		return truth(properties.get(((Node) node).label).get(key));
	}

	public static int number(Object node, String key, int fallback) {
		return n(or(properties.get(((Node) node).label), key, fallback));
	}

	/** The one attribute Repository reads: the Grand Exchange news mute. */
	public static Object attribute(Object node, String key) {
		return core.game.world.repository.RepositoryKt.GE_NEWS_MUTE_ATTRIBUTE.equals(key) && flag(node, "geMuted") ? Boolean.TRUE : null;
	}

	public static ClanRepository clan(String label) {
		Object owner = properties.get(label).get("clan");
		return owner == null ? null : clans.computeIfAbsent((String) owner, ClanRepository::new);
	}

	public static GameSettings settings() {
		return settings;
	}

	public static List<NPC> chunkNpcs() {
		return chunk;
	}

	public static void launch(Runnable task) {
		emit("launch");
		tasks.add(task);
	}

	private static String record(String event, Object... args) {
		if (!quiet) {
			List<Object> row = new ArrayList<>();
			row.add(event);
			row.addAll(Arrays.asList(args));
			events.add(row);
		}
		int count = calls.merge(event, 1, Integer::sum);
		for (Object entry : (List) or(input, "hooks", Collections.emptyList())) {
			Map hook = (Map) entry;
			if (event.equals(hook.get("event")) && count == n(or(hook, "at", 1))) {
				for (Object action : (List) hook.get("actions")) {
					act((Map) action);
				}
			}
		}
		Map failures = (Map) or(input, "failures", Collections.emptyMap());
		Map planned = (Map) failures.get(event);
		return planned == null ? null : (String) planned.get(Integer.toString(count));
	}

	/** Records a call, runs the case's hooks for it and then throws the failure the case planned, if any. */
	public static void emit(String event, Object... args) {
		String kind = record(event, args);
		if (kind == null) {
			return;
		}
		String message = event + "#" + calls.get(event);
		if (kind.equals("error")) {
			throw new InjectedError(message);
		}
		if (kind.equals("exception")) {
			throw new Injected(message);
		}
		throw new IllegalArgumentException("a " + kind + " failure cannot leave " + event);
	}

	/** The one call that may throw a checked IOException: Socket.close. */
	public static void emitIo(String event, Object... args) throws IOException {
		String kind = record(event, args);
		if (kind == null) {
			return;
		}
		String message = event + "#" + calls.get(event);
		if (kind.equals("io")) {
			throw new InjectedIo(message);
		}
		if (kind.equals("error")) {
			throw new InjectedError(message);
		}
		throw new Injected(message);
	}

	// Case set-up. JSON objects arrive as hash maps, so every walk over one is in sorted label order.

	static Set sorted(String key) {
		return new TreeMap<String, Object>((Map) or(input, key, Collections.emptyMap())).entrySet();
	}

	static void setUp() throws Exception {
		ServerConstants.MAX_PLAYERS = n(or(input, "maxPlayers", ServerConstants.MAX_PLAYERS));
		ServerConstants.MAX_NPCS = n(or(input, "maxNpcs", ServerConstants.MAX_NPCS));
		ServerConstants.ENABLE_GLOBAL_CHAT = input.containsKey("globalChat") ? truth(input.get("globalChat")) : ServerConstants.ENABLE_GLOBAL_CHAT;
		ticks = n(or(input, "ticks", 0));
		settings = input.containsKey("worldId") && input.get("worldId") == null ? null : new GameSettings(n(or(input, "worldId", 1)));
		Thread.currentThread().setName("main");
		for (Object entry : sorted("sessions")) {
			Map.Entry<String, Map> e = (Map.Entry<String, Map>) entry;
			Map spec = e.getValue();
			IoSession session = new IoSession(truth(spec.get("key")) ? new Key(e.getKey()) : null, (String) spec.get("username"));
			session.label = e.getKey();
			session.oracleActive(!Boolean.FALSE.equals(spec.get("active")));
			sessions.put(e.getKey(), session);
		}
		for (Object entry : sorted("players")) {
			Map.Entry<String, Map> e = (Map.Entry<String, Map>) entry;
			Map spec = e.getValue();
			properties.put(e.getKey(), new HashMap(spec));
			PlayerDetails details = new PlayerDetails((String) spec.get("name"));
			details.label = e.getKey();
			details.oracleUid(n(or(spec, "uid", 0)));
			details.setSession(spec.get("session") == null ? null : sessions.get(spec.get("session")));
			Player player = new Player(details);
			// The node keeps the name it was constructed with; the account name may differ from it afterwards.
			if (spec.containsKey("username")) {
				details.oracleUsername((String) spec.get("username"));
			}
			player.label = e.getKey();
			player.oracleArtificial(truth(spec.get("artificial")));
			player.isAfkLogout = (Boolean) spec.get("afk");
			player.setPlaying(!Boolean.FALSE.equals(spec.get("playing")));
			for (Object key : (List) or(spec, "listeners", Collections.emptyList())) {
				player.logoutListeners.put((String) key, listener((String) key));
			}
			if (!Boolean.FALSE.equals(spec.get("renderable"))) {
				renderable.add(player);
			}
			players.put(e.getKey(), player);
		}
		for (Object entry : sorted("sessions")) {
			Map.Entry<String, Map> e = (Map.Entry<String, Map>) entry;
			Object object = e.getValue().get("object");
			if (object != null) {
				sessions.get(e.getKey()).setObject(players.containsKey(object) ? players.get(object) : object);
			}
		}
		for (Object entry : sorted("npcs")) {
			Map.Entry<String, Map> e = (Map.Entry<String, Map>) entry;
			Map spec = e.getValue();
			properties.put(e.getKey(), new HashMap(spec));
			NPC npc = truth(spec.get("revenant")) ? new RevenantNPC(e.getKey()) : new NPC(e.getKey());
			npc.label = e.getKey();
			npc.oracleId(n(or(spec, "id", 0)));
			npc.oracleLocation(location(spec.get("location")));
			npcs.put(e.getKey(), npc);
		}
		for (Object label : (List) or(input, "chunk", Collections.emptyList())) {
			chunk.add(npcs.get(label));
		}
		for (Object label : (List) or(input, "worldListeners", Collections.emptyList())) {
			String name = (String) label;
			GameWorld.getLogoutListeners().add(new LogoutListener() {
				@Override
				public void logout(Player player) {
					emit("worldLogout", name, player.label);
				}
			});
		}
	}

	static Function1<Player, Unit> listener(String key) {
		return player -> {
			emit("playerLogout", key, player.label);
			return Unit.INSTANCE;
		};
	}

	static Location location(Object value) {
		if (value == null) {
			return null;
		}
		List list = (List) value;
		return new Location(n(list.get(0)), n(list.get(1)), n(list.get(2)));
	}

	static Node node(Object label) {
		if (label == null) {
			return null;
		}
		Node found = players.containsKey(label) ? players.get(label) : npcs.get(label);
		if (found == null) {
			throw new IllegalArgumentException("unknown node " + label);
		}
		return found;
	}

	static Player player(Object label) {
		return (Player) node(label);
	}

	static Object label(Object node) {
		return node == null ? null : ((Node) node).label;
	}

	static NodeList list(Object name) {
		return "npcs".equals(name) ? Repository.getNpcs() : Repository.getPlayers();
	}

	static List<Node> nodes(Object labels) {
		List<Node> out = new ArrayList<>();
		for (Object label : (List) labels) {
			out.add(node(label));
		}
		return out;
	}

	static List<Object> labels(Object[] nodes) {
		List<Object> out = new ArrayList<>();
		for (Object node : nodes) {
			out.add(label(node));
		}
		return out;
	}

	static void set(Map a) {
		String label = (String) a.get("node");
		String field = (String) a.get("field");
		Object value = a.get("value");
		Node node = node(label);
		switch (field) {
			case "uid": player(label).getDetails().oracleUid(n(value)); break;
			case "name": node.oracleName((String) value); break;
			case "username": player(label).getDetails().oracleUsername((String) value); break;
			case "afk": player(label).isAfkLogout = (Boolean) value; break;
			case "artificial": player(label).oracleArtificial(truth(value)); break;
			case "session": player(label).getDetails().setSession(value == null ? null : sessions.get(value)); break;
			case "id": ((NPC) node).oracleId(n(value)); break;
			case "location": ((Entity) node).oracleLocation(location(value)); break;
			default: properties.get(label).put(field, value);
		}
	}

	/** Calls the synthetic method Kotlin callers reach when they leave default arguments out. */
	static void callDefault(String name, Object... args) {
		for (java.lang.reflect.Method method : Repository.class.getDeclaredMethods()) {
			if (!method.getName().equals(name + "$default")) {
				continue;
			}
			Object[] full = args;
			if (method.getParameterCount() == args.length + 1) {
				full = new Object[args.length + 1];
				full[0] = Repository.INSTANCE;
				System.arraycopy(args, 0, full, 1, args.length);
			}
			try {
				method.setAccessible(true);
				method.invoke(null, full);
			} catch (java.lang.reflect.InvocationTargetException e) {
				if (e.getCause() instanceof RuntimeException) {
					throw (RuntimeException) e.getCause();
				}
				throw (Error) e.getCause();
			} catch (IllegalAccessException e) {
				throw new IllegalArgumentException(e);
			}
			return;
		}
		throw new IllegalArgumentException(name);
	}

	// Actions.

	static Object act(Map a) {
		String op = (String) a.get("op");
		DisconnectionQueue queue = Repository.getDisconnectionQueue();
		switch (op) {
			case "addPlayer": Repository.addPlayer(player(a.get("node"))); return null;
			case "removePlayer": Repository.removePlayer(player(a.get("node"))); return null;
			case "byName": return label(Repository.getPlayerByName((String) a.get("name")));
			case "byUid": return label(Repository.getPlayerByUid(n(a.get("uid"))));
			case "news1": Repository.sendNews((String) a.get("text")); return null;
			// The two default arguments as a Kotlin caller gets them: 12 and "CC6600".
			case "news2": callDefault("sendNews", a.get("text"), n(a.get("icon")), null, 4, null); return null;
			case "newsDefault": callDefault("sendNews", a.get("text"), 0, null, 6, null); return null;
			case "geNewsDefault": callDefault("sendGrandExchangeNews", a.get("text"), 0, null, 6, null); return null;
			case "news3": Repository.sendNews((String) a.get("text"), n(a.get("icon")), (String) a.get("color")); return null;
			case "news4": Repository.sendNews((String) a.get("text"), n(a.get("icon")), (String) a.get("color"), truth(a.get("ge"))); return null;
			case "geNews3": Repository.sendGrandExchangeNews((String) a.get("text"), n(a.get("icon")), (String) a.get("color")); return null;
			case "addRenderable": Repository.addRenderableNPC((NPC) node(a.get("node"))); return null;
			case "removeRenderable": Repository.removeRenderableNPC((NPC) node(a.get("node"))); return null;
			case "findNpcId": return label(Repository.findNPC(n(a.get("id"))));
			case "findNpcAt": return label(Repository.findNPC(location(a.get("location"))));
			case "renderableNpcs": return labels(Repository.getRenderableNpcs().toArray());
			case "lobbyAdd": return Repository.getLobbyPlayers().add(player(a.get("node")));
			case "lobbyRemove": return Repository.getLobbyPlayers().remove(player(a.get("node")));

			case "nlAdd": return list(a.get("list")).add(node(a.get("node")));
			case "nlAddAll": return list(a.get("list")).addAll(nodes(a.get("nodes")));
			case "nlGet": return label(list(a.get("list")).get(n(a.get("index"))));
			case "nlRemoveAt": return list(a.get("list")).remove(n(a.get("index")));
			case "nlRemove": return list(a.get("list")).remove((Object) node(a.get("node")));
			case "nlRemoveAll": return list(a.get("list")).removeAll(nodes(a.get("nodes")));
			case "nlRetainAll": return list(a.get("list")).retainAll(nodes(a.get("nodes")));
			case "nlContains": return list(a.get("list")).contains(node(a.get("node")));
			case "nlContainsAll": return list(a.get("list")).containsAll(nodes(a.get("nodes")));
			case "nlClear": list(a.get("list")).clear(); return null;
			case "nlIsEmpty": return list(a.get("list")).isEmpty();
			case "nlSize": return list(a.get("list")).size();
			case "nlLength": return list(a.get("list")).length();
			case "nlToArray": return labels(list(a.get("list")).toArray());
			case "nlIndexOf": return list(a.get("list")).indexOf(node(a.get("node")));
			case "itNew": iterators.put((String) a.get("it"), list(a.get("list")).iterator()); return null;
			case "itHasNext": return iterators.get(a.get("it")).hasNext();
			case "itNext": return label(iterators.get(a.get("it")).next());
			case "itRemove": iterators.get(a.get("it")).remove(); return null;

			case "qAdd":
				if (a.containsKey("clear")) {
					queue.add(player(a.get("node")), truth(a.get("clear")));
				} else {
					queue.add(player(a.get("node")));
				}
				return null;
			case "qUpdate": queue.update(); return null;
			case "qClear": queue.clear(); return null;
			case "qGet": return label(queue.get((String) a.get("name")));
			case "qContains": return queue.contains((String) a.get("name"));
			case "qRemove": queue.remove((String) a.get("name")); return null;
			case "qIsEmpty": return queue.isEmpty();
			case "qSave": return queue.save(player(a.get("node")), truth(a.get("sql")));
			case "tick":
				// The part of MajorUpdateWorker.tickOffline this corpus needs: update, then the tick counter.
				for (int i = 0, count = n(or(a, "count", 1)); i < count; i++) {
					queue.update();
					ticks++;
				}
				return null;
			case "setTicks": ticks = n(a.get("value")); return null;
			case "runTasks": {
				List<Runnable> pending = new ArrayList<>(tasks);
				tasks.clear();
				for (Runnable task : pending) {
					task.run();
				}
				return pending.size();
			}

			case "clear": player(a.get("node")).clear(); return null;
			case "finishClear": player(a.get("node")).finishClear(); return null;
			case "allowRemoval": return player(a.get("node")).allowRemoval();
			case "sendLogout": player(a.get("node")).getPacketDispatch().sendLogout(); return null;
			case "disconnect": sessions.get(a.get("session")).disconnect(); return null;
			case "sessionPlayer": return label(sessions.get(a.get("session")).getPlayer());
			case "set": set(a); return null;
			case "setSession": {
				IoSession session = sessions.get(a.get("session"));
				if (a.containsKey("active")) {
					session.oracleActive(truth(a.get("active")));
				}
				if (a.containsKey("object")) {
					Object object = a.get("object");
					session.setObject(players.containsKey(object) ? players.get(object) : object);
				}
				return null;
			}
			case "renderableAdd": renderable.add(player(a.get("node"))); return null;
			default: throw new IllegalArgumentException(op);
		}
	}

	static String category(Throwable t) {
		if (t instanceof Injected) return "exception:" + t.getMessage();
		if (t instanceof InjectedError) return "error:" + t.getMessage();
		if (t instanceof InjectedIo) return "io:" + t.getMessage();
		if (t instanceof ArrayIndexOutOfBoundsException) return "ArrayIndexOutOfBounds";
		if (t instanceof IndexOutOfBoundsException) return "IndexOutOfBounds";
		if (t instanceof NegativeArraySizeException) return "NegativeArraySize";
		if (t instanceof NullPointerException) return "NullPointer";
		if (t instanceof NoSuchElementException) return "NoSuchElement";
		if (t instanceof IllegalStateException) return "IllegalState";
		throw new IllegalStateException("unexpected failure", t);
	}

	// State.

	static Object field(Object target, String name) throws Exception {
		Field field = target.getClass().getDeclaredField(name);
		field.setAccessible(true);
		return field.get(target);
	}

	static List<Object> slots(NodeList list) throws Exception {
		Node[] nodes = (Node[]) field(list, "nodes");
		List<Object> out = new ArrayList<>();
		for (int i = 0; i < nodes.length; i++) {
			if (nodes[i] != null) {
				out.add(Arrays.asList(i, nodes[i].label));
			}
		}
		return out;
	}

	static Map<String, Object> state() throws Exception {
		Map<String, Object> state = new LinkedHashMap<>();
		state.put("players", slots(Repository.getPlayers()));
		state.put("playersSize", Repository.getPlayers().size());
		state.put("npcs", slots(Repository.getNpcs()));
		state.put("npcsSize", Repository.getNpcs().size());
		List<Object> uids = new ArrayList<>();
		for (Map.Entry<Integer, Player> e : Repository.INSTANCE.getUid_map().entrySet()) {
			uids.add(Arrays.asList(e.getKey(), label(e.getValue())));
		}
		state.put("uids", uids);
		List<Object> names = new ArrayList<>();
		for (Map.Entry<String, Player> e : Repository.getPlayerNames().entrySet()) {
			names.add(Arrays.asList(e.getKey(), label(e.getValue())));
		}
		state.put("names", names);
		state.put("renderableNpcs", labels(Repository.INSTANCE.getRENDERABLE_NPCS().toArray()));
		state.put("lobby", labels(Repository.getLobbyPlayers().toArray()));
		state.put("loggedIn", new ArrayList<>(Repository.INSTANCE.getLOGGED_IN_PLAYERS()));
		DisconnectionQueue queue = Repository.getDisconnectionQueue();
		List<Object> queued = new ArrayList<>();
		for (Object entry : ((Map) field(queue, "queue")).entrySet()) {
			Map.Entry e = (Map.Entry) entry;
			Object value = e.getValue();
			queued.add(Arrays.asList(e.getKey(), label(value.getClass().getMethod("getPlayer").invoke(value)), value.getClass().getMethod("isClear").invoke(value)));
		}
		state.put("queue", queued);
		List<Object> timers = new ArrayList<>();
		for (Object entry : ((Map) field(queue, "queueTimers")).entrySet()) {
			Map.Entry e = (Map.Entry) entry;
			timers.add(Arrays.asList(e.getKey(), e.getValue()));
		}
		state.put("timers", timers);
		List<Object> playerRows = new ArrayList<>();
		for (Player player : players.values()) {
			playerRows.add(Arrays.asList(player.label, player.getIndex(), player.isPlaying()));
		}
		state.put("playerNodes", playerRows);
		List<Object> npcRows = new ArrayList<>();
		for (NPC npc : npcs.values()) {
			npcRows.add(Arrays.asList(npc.label, npc.getIndex(), npc.isRenderable()));
		}
		state.put("npcNodes", npcRows);
		List<Object> sessionRows = new ArrayList<>();
		for (IoSession session : sessions.values()) {
			Object object = session.oracleObject();
			sessionRows.add(Arrays.asList(session.label, session.isActive(), object == null ? null : object instanceof Player ? ((Player) object).label : "other"));
		}
		state.put("sessions", sessionRows);
		state.put("renderablePlayers", labels(renderable.toArray()));
		state.put("tasks", tasks.size());
		state.put("ticks", ticks);
		state.put("thread", Thread.currentThread().getName());
		return state;
	}

	// java.util.HashMap itself, for the order the queue and the name map are walked in.

	static Object mapCase() {
		HashMap<String, Integer> map = input.get("capacity") == null ? new HashMap<>() : new HashMap<>(n(input.get("capacity")));
		List<Object> dumps = new ArrayList<>();
		for (Object entry : (List) input.get("steps")) {
			List step = (List) entry;
			String op = (String) step.get(0);
			switch (op) {
				case "put": dumps.add(map.put((String) step.get(1), n(step.get(2)))); break;
				case "remove": dumps.add(map.remove((String) step.get(1))); break;
				case "get": dumps.add(map.get((String) step.get(1))); break;
				case "clear": map.clear(); dumps.add(null); break;
				case "dump": {
					List<Object> rows = new ArrayList<>();
					for (Map.Entry<String, Integer> e : map.entrySet()) {
						rows.add(Arrays.asList(e.getKey(), e.getValue()));
					}
					dumps.add(rows);
					break;
				}
				default: throw new IllegalArgumentException(op);
			}
		}
		return dumps;
	}

	static Object intMapCase() {
		HashMap<Integer, Integer> map = input.get("capacity") == null ? new HashMap<>() : new HashMap<>(n(input.get("capacity")));
		List<Object> dumps = new ArrayList<>();
		for (Object entry : (List) input.get("steps")) {
			List step = (List) entry;
			String op = (String) step.get(0);
			switch (op) {
				case "put": dumps.add(map.put(n(step.get(1)), n(step.get(2)))); break;
				case "remove": dumps.add(map.remove(n(step.get(1)))); break;
				case "get": dumps.add(map.get(n(step.get(1)))); break;
				case "clear": map.clear(); dumps.add(null); break;
				case "dump": {
					List<Object> rows = new ArrayList<>();
					for (Map.Entry<Integer, Integer> e : map.entrySet()) {
						rows.add(Arrays.asList(e.getKey(), e.getValue()));
					}
					dumps.add(rows);
					break;
				}
				default: throw new IllegalArgumentException(op);
			}
		}
		return dumps;
	}

	/** One case in, the case with its expectations out. Called by Launcher through reflection. */
	public static String run(String line) throws Exception {
		input = (Map) new JSONParser().parse(line);
		if ("map".equals(input.get("kind"))) {
			input.put("expected", mapCase());
			return JSONValue.toJSONString(input);
		}
		if ("intMap".equals(input.get("kind"))) {
			input.put("expected", intMapCase());
			return JSONValue.toJSONString(input);
		}
		events = new ArrayList<>();
		calls = new HashMap<>();
		setUp();
		List<Object> steps = new ArrayList<>();
		for (Object entry : (List) input.get("actions")) {
			Map action = (Map) entry;
			events.clear();
			quiet = truth(action.get("quiet"));
			Object result = null;
			String error = null;
			try {
				result = act(action);
			} catch (IllegalArgumentException e) {
				throw e;
			} catch (Throwable t) {
				error = category(t);
			}
			Map<String, Object> step = new LinkedHashMap<>();
			step.put("result", result);
			step.put("error", error);
			step.put("events", new ArrayList<>(events));
			if (!quiet) {
				step.put("state", state());
			}
			steps.add(step);
			quiet = false;
		}
		input.put("expected", steps);
		return JSONValue.toJSONString(input);
	}
}
