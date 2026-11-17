package oracle;

import com.google.protobuf.Descriptors;
import com.google.protobuf.Message;
import core.auth.AuthProvider;
import core.auth.UserAccountInfo;
import core.game.bots.AIPlayer;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.info.Rights;
import core.game.system.communication.ClanEntry;
import core.game.system.communication.ClanRank;
import core.game.system.communication.ClanRepository;
import core.game.system.communication.CommunicationInfo;
import core.game.system.communication.Contact;
import core.game.system.communication.GlobalChat;
import core.game.system.task.Pulse;
import core.game.world.GameSettings;
import core.game.world.repository.Repository;
import core.game.world.update.flag.context.ChatMessage;
import core.net.IoSession;
import core.net.amsc.ManagementServerState;
import core.net.packet.IoBuffer;
import core.net.packet.PacketHeader;
import core.net.packet.PacketProcessor;
import core.net.packet.PacketWriteQueue;
import core.net.packet.context.ClanContext;
import core.net.packet.context.ContactContext;
import core.net.packet.context.MessageContext;
import core.net.packet.in.Decoders530;
import core.net.packet.in.Login;
import core.net.packet.in.Packet;
import core.storage.AccountStorageProvider;
import core.tools.CP1252;
import core.tools.StringUtils;
import core.worker.ManagementEvents;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.sql.Timestamp;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Deque;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Queue;
import java.util.TreeMap;
import kotlin.Unit;
import kotlin.jvm.functions.Function2;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;
import org.json.simple.JSONValue;

/**
 * Drives one case against the unchanged communication, clan, management-event,
 * chat and outgoing-packet sources. Launcher loads this class and everything it
 * reaches in a fresh class loader per case, so the static clan registry, the
 * management queue and the packet write queue start empty.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public final class Support {
	static Map input;
	static Map world;
	static List<Object> events;
	static final Map<String, Integer> calls = new HashMap<>();
	static final Map<String, Player> players = new TreeMap<>();
	static final Map<String, Map<String, Object>> attributes = new HashMap<>();
	static final Map<String, List<String>> locks = new HashMap<>();
	static final Map<String, Integer> cipherDraws = new HashMap<>();
	static final List<Pulse> pulses = new ArrayList<>();
	static final Map<String, Function2> commands = new HashMap<>();
	static final IdentityHashMap<ClanRepository, Integer> clanIds = new IdentityHashMap<>();
	static Runnable management;
	static long clock;
	static int seedIndex;

	/** A failure the case injects into a recorded call. */
	public static final class Injected extends RuntimeException {
		Injected(String message) {
			super(message);
		}
	}

	/** Ends a pump of the management coroutine where the original would block on an empty queue. */
	static final class PumpStop extends Error {
		PumpStop() {
			super("management queue is empty");
		}
	}

	static Object number(Object value) {
		return value instanceof Long ? Integer.valueOf(((Long) value).intValue()) : value;
	}

	static int integer(Object value) {
		return ((Number) value).intValue();
	}

	static String str(Map map, String key, String fallback) {
		Object value = map.get(key);
		return value == null ? fallback : (String) value;
	}

	public static void emit(Object... fields) {
		JSONArray event = new JSONArray();
		for (Object field : fields) {
			event.add(field);
		}
		events.add(event);
	}

	/** Records a call into another system, then fails it if the case says this call fails. */
	public static void call(String name, Object... args) {
		Object[] fields = new Object[args.length + 1];
		fields[0] = name;
		System.arraycopy(args, 0, fields, 1, args.length);
		emit(fields);
		check(name);
	}

	static void check(String name) {
		int count = calls.merge(name, 1, Integer::sum);
		Map failures = (Map) input.get("failures");
		if (failures == null) {
			return;
		}
		List at = (List) failures.get(name);
		if (at != null && at.contains((long) count)) {
			throw new Injected(name + "#" + count);
		}
	}

	public static String label(Object player) {
		return player instanceof Player ? ((Player) player).oracleLabel() : null;
	}

	// --- Case inputs read by adapters.

	public static String text(String key) {
		return str(world, key, "");
	}

	public static boolean flag(String key) {
		return Boolean.TRUE.equals(world.get(key));
	}

	public static GameSettings gameSettings() {
		return new GameSettings(str(world, "name", "2009scape"), flag("devMode"), integer(world.get("worldId")), flag("defaultClan"));
	}

	public static ManagementServerState managementState() {
		return ManagementServerState.valueOf(str(world, "ms", "CONNECTING"));
	}

	public static IoSession managementSession() {
		return flag("msSession") ? new IoSession(null) : null;
	}

	public static boolean resizable(Player player) {
		return Boolean.TRUE.equals(((Map) ((Map) input.get("players")).get(label(player))).get("resizable"));
	}

	// --- Redirected host inputs (the rewrite tool sends the original call sites here).

	public static long currentTimeMillis() {
		emit("clock", clock);
		return clock;
	}

	public static long nanoTime() {
		throw new IllegalStateException("nanoTime is not an input of this corpus");
	}

	public static long nextSeed() {
		List seeds = (List) input.get("seeds");
		long seed = ((Number) seeds.get(seedIndex++)).longValue();
		emit("random", seed);
		return seed;
	}

	// --- Recording adapters.

	public static Object getAttribute(Entity entity, String key, Object fail) {
		String owner = label(entity);
		emit("getAttribute", owner, key);
		check("getAttribute");
		Object value = attributes.computeIfAbsent(owner, k -> new HashMap<>()).get(key.replace("/save:", ""));
		return value != null ? value : fail;
	}

	public static void setAttribute(Entity entity, String key, Object value) {
		String owner = label(entity);
		emit("setAttribute", owner, key, json(value));
		check("setAttribute");
		String stored = key.startsWith("/save:") ? key.substring(6) : key;
		attributes.computeIfAbsent(owner, k -> new HashMap<>()).put(stored, value);
	}

	public static void removeAttribute(Entity entity, String key) {
		call("removeAttribute", label(entity), key);
		attributes.computeIfAbsent(label(entity), k -> new HashMap<>()).remove(key);
	}

	public static boolean isLocked(Entity owner, String key) {
		call("isLocked", label(owner), key);
		return locks.computeIfAbsent(label(owner), k -> new ArrayList<>()).contains(key);
	}

	public static void lock(Entity owner, String key, int ticks) {
		call("lock", label(owner), key, ticks);
		locks.computeIfAbsent(label(owner), k -> new ArrayList<>()).add(key);
	}

	public static boolean registerMask(Entity owner, String flag, Object context, boolean sync) {
		ChatMessage message = (ChatMessage) context;
		call("mask", label(owner), flag, sync, label(message.getPlayer()), message.getText(), message.getEffects(), message.getNumChars(),
			message.getChatIcon(), message.isQuickChat);
		return true;
	}

	public static void write(Player owner, Object context) {
		if (!(context instanceof IoBuffer)) {
			call("writeObject", label(owner), context.getClass().getSimpleName());
			return;
		}
		IoBuffer buffer = (IoBuffer) context;
		ByteBuffer data = buffer.toByteBuffer();
		StringBuilder hex = new StringBuilder();
		for (int i = 0; i < data.position(); i++) {
			hex.append(String.format("%02x", data.get(i) & 0xff));
		}
		if (owner == null) {
			call("msWrite", buffer.opcode(), buffer.getHeader().name(), hex.toString());
		} else {
			call("write", label(owner), buffer.opcode(), buffer.getHeader().name(), hex.toString());
		}
	}

	public static int cipher(Player owner) {
		String player = label(owner);
		call("cipher", player);
		List values = (List) ((Map) ((Map) input.get("players")).get(player)).get("cipher");
		int index = cipherDraws.merge(player, 1, Integer::sum) - 1;
		return values == null || values.isEmpty() ? 0 : integer(values.get(index % values.size()));
	}

	public static void submit(Pulse pulse) {
		pulses.add(pulse);
		call("submit", pulses.size() - 1, pulse.oracleDelay());
	}

	public static void stopPulse(Pulse pulse) {
		call("pulseStop", pulses.indexOf(pulse));
	}

	public static void log(Class<?> origin, String level, String message) {
		StringBuilder kept = new StringBuilder();
		// Stack frames belong to the JVM, not to the original's behaviour.
		for (String line : message.split("\\r?\\n")) {
			if (!line.startsWith("\tat ") && !line.startsWith("\t... ")) {
				kept.append(line).append('\n');
			}
		}
		call("log", origin.getSimpleName(), level, kept.toString());
	}

	public static void define(String name, String privilege, String usage, String description, Function2 handle) {
		commands.put(name, handle);
		call("define", name, privilege, usage, description);
	}

	public static void unprocessed(Object packet) {
		call("unprocessed", packet.getClass().getSimpleName());
	}

	public static AuthProvider<?> authenticator() {
		return new AuthProvider<Object>() {
			@Override
			public boolean createAccountWith(UserAccountInfo info) {
				call("createAccount", info.getUsername(), info.getPassword(), info.getRights());
				return true;
			}
		};
	}

	public static AccountStorageProvider accountStorage() {
		return new AccountStorageProvider() {
			@Override
			public UserAccountInfo getAccountInfo(String username) {
				call("accountInfo", username);
				Map accounts = (Map) input.get("accounts");
				Map account = accounts == null ? null : (Map) accounts.get(username);
				if (account == null) {
					return UserAccountInfo.createDefault();
				}
				UserAccountInfo info = new UserAccountInfo(username, str(account, "password", ""), 0, integer(account.getOrDefault("rights", 0L)), 0, "", "", 0L,
					0L, str(account, "contacts", ""), str(account, "blocked", ""), str(account, "clanName", ""), str(account, "currentClan", ""),
					str(account, "clanReqs", "1,0,8,9"), 0L, 0L, false, new Timestamp(0L));
				info.setInitialReferenceValues();
				return info;
			}

			@Override
			public List<String> getOnlineFriends(String username) {
				call("onlineFriends", username);
				Map friends = (Map) input.get("onlineFriends");
				return friends == null || friends.get(username) == null ? new ArrayList<>() : new ArrayList<>((List<String>) friends.get(username));
			}

			@Override
			public void update(UserAccountInfo info) {
				call("accountUpdate", info.getUsername(), info.getClanName(), info.getClanReqs(), info.getRights());
			}
		};
	}

	// --- The management coroutine.

	public static void launch(Runnable block) {
		management = block;
	}

	static Object field(Class<?> type, Object owner, String name) throws Exception {
		Field field = type.getDeclaredField(name);
		field.setAccessible(true);
		return field.get(java.lang.reflect.Modifier.isStatic(field.getModifiers()) ? null : owner);
	}

	static Deque<Message> managementQueue() throws Exception {
		return (Deque<Message>) field(ManagementEvents.class, ManagementEvents.INSTANCE, "eventQueue");
	}

	public static void beforeTake() throws Exception {
		Message next = managementQueue().peek();
		if (next == null) {
			throw new PumpStop();
		}
		emit("take", describe(next));
	}

	static void pump() throws Exception {
		Object init = ManagementEvents.INSTANCE;
		try {
			management.run();
		} catch (PumpStop stop) {
			// The queue is drained; the original coroutine would wait here.
		}
	}

	// --- JSON views.

	static Object json(Object value) {
		if (value == null || value instanceof String || value instanceof Boolean || value instanceof Integer || value instanceof Long) {
			return value;
		}
		if (value instanceof Enum) {
			return ((Enum) value).name();
		}
		return value.toString();
	}

	static Map<String, Object> describe(Message message) {
		Map<String, Object> out = new LinkedHashMap<>();
		out.put("@", message.getDescriptorForType().getName());
		for (Descriptors.FieldDescriptor field : message.getDescriptorForType().getFields()) {
			Object value = message.getField(field);
			if (field.isRepeated()) {
				JSONArray items = new JSONArray();
				for (Object item : (List) value) {
					items.add(item instanceof Message ? describe((Message) item) : protoValue(item));
				}
				out.put(field.getName(), items);
			} else {
				out.put(field.getName(), value instanceof Message ? describe((Message) value) : protoValue(value));
			}
		}
		return out;
	}

	static Object protoValue(Object value) {
		if (value instanceof Descriptors.EnumValueDescriptor) {
			return ((Descriptors.EnumValueDescriptor) value).getName();
		}
		return value;
	}

	static Message build(Map spec) throws Exception {
		Class<?> type = Class.forName("proto.management." + spec.get("@"));
		Message.Builder builder = (Message.Builder) type.getMethod("newBuilder").invoke(null);
		fill(builder, spec);
		return builder.build();
	}

	static void fill(Message.Builder builder, Map spec) {
		for (Object key : spec.keySet()) {
			if ("@".equals(key)) {
				continue;
			}
			Descriptors.FieldDescriptor field = builder.getDescriptorForType().findFieldByName((String) key);
			Object value = spec.get(key);
			if (field.isRepeated()) {
				for (Object item : (List) value) {
					builder.addRepeatedField(field, protoInput(builder, field, item));
				}
			} else {
				builder.setField(field, protoInput(builder, field, value));
			}
		}
	}

	static Object protoInput(Message.Builder builder, Descriptors.FieldDescriptor field, Object value) {
		switch (field.getJavaType()) {
			case INT:
				return integer(value);
			case LONG:
				return ((Number) value).longValue();
			case ENUM:
				return field.getEnumType().findValueByName((String) value);
			case MESSAGE:
				Message.Builder nested = builder.newBuilderForField(field);
				fill(nested, (Map) value);
				return nested.build();
			default:
				return value;
		}
	}

	static Object describePacket(Packet packet) {
		JSONArray out = new JSONArray();
		out.add(packet.getClass().getSimpleName());
		if (packet instanceof Packet.AddFriend) {
			out.add(((Packet.AddFriend) packet).getUsername());
		} else if (packet instanceof Packet.RemoveFriend) {
			out.add(((Packet.RemoveFriend) packet).getUsername());
		} else if (packet instanceof Packet.AddIgnore) {
			out.add(((Packet.AddIgnore) packet).getUsername());
		} else if (packet instanceof Packet.RemoveIgnore) {
			out.add(((Packet.RemoveIgnore) packet).getUsername());
		} else if (packet instanceof Packet.JoinClan) {
			out.add(((Packet.JoinClan) packet).getClanName());
		} else if (packet instanceof Packet.SetClanRank) {
			out.add(((Packet.SetClanRank) packet).getUsername());
			out.add(((Packet.SetClanRank) packet).getRank());
		} else if (packet instanceof Packet.KickFromClan) {
			out.add(((Packet.KickFromClan) packet).getUsername());
		} else if (packet instanceof Packet.PrivateMessage) {
			out.add(((Packet.PrivateMessage) packet).getUsername());
			out.add(((Packet.PrivateMessage) packet).getMessage());
		} else if (packet instanceof Packet.ChatMessage) {
			out.add(((Packet.ChatMessage) packet).getEffects());
			out.add(((Packet.ChatMessage) packet).getMessage());
		} else if (packet instanceof Packet.ChatSetting) {
			out.add(((Packet.ChatSetting) packet).getPublic());
			out.add(((Packet.ChatSetting) packet).getPrivate());
			out.add(((Packet.ChatSetting) packet).getTrade());
		} else if (packet instanceof Packet.DecodingError) {
			String message = ((Packet.DecodingError) packet).getMessage();
			int end = message.indexOf('\n');
			out.add((end < 0 ? message : message.substring(0, end)).replace("\r", ""));
		}
		return out;
	}

	// --- State snapshot.

	static Integer clanId(ClanRepository clan) {
		if (clan == null) {
			return null;
		}
		return clanIds.computeIfAbsent(clan, k -> clanIds.size());
	}

	static Object ranks(CommunicationInfo info) {
		JSONArray out = new JSONArray();
		for (ClanRank rank : new ClanRank[] {info.getJoinRequirement(), info.getMessageRequirement(), info.getKickRequirement(), info.getLootRequirement()}) {
			out.add(rank == null ? null : rank.ordinal());
		}
		return out;
	}

	static String saved(java.util.function.Supplier<String> text) {
		try {
			return text.get();
		} catch (RuntimeException e) {
			return "!" + e.getClass().getSimpleName();
		}
	}

	static Map<String, Object> snapshot() throws Exception {
		Map<String, Object> state = new LinkedHashMap<>();
		Map<String, Object> registry = new TreeMap<>();
		for (Map.Entry<String, ClanRepository> entry : new TreeMap<>(ClanRepository.getClans()).entrySet()) {
			registry.put(entry.getKey(), clanId(entry.getValue()));
		}
		Map<String, Object> people = new LinkedHashMap<>();
		for (Map.Entry<String, Player> entry : players.entrySet()) {
			CommunicationInfo info = entry.getValue().getDetails().getCommunication();
			Map<String, Object> view = new LinkedHashMap<>();
			JSONArray contacts = new JSONArray();
			for (Map.Entry<String, Contact> contact : info.getContacts().entrySet()) {
				JSONArray row = new JSONArray();
				row.add(contact.getKey());
				row.add(contact.getValue().getUsername());
				row.add(contact.getValue().getRank() == null ? null : contact.getValue().getRank().ordinal());
				row.add(contact.getValue().getWorldId());
				contacts.add(row);
			}
			view.put("contacts", contacts);
			view.put("blocked", new JSONArray() {{ addAll(info.getBlocked()); }});
			view.put("clanName", field(CommunicationInfo.class, info, "clanName"));
			view.put("currentClan", field(CommunicationInfo.class, info, "currentClan"));
			view.put("reqs", ranks(info));
			view.put("lootShare", info.isLootShare());
			view.put("clan", clanId(info.getClan()));
			view.put("lootPulse", pulses.indexOf(field(CommunicationInfo.class, info, "lootSharePulse")));
			// What PlayerDetails.save writes back to the account.
			JSONArray saved = new JSONArray();
			saved.add(saved(info::getContactString));
			saved.add(saved(info::getBlockedString));
			saved.add(saved(info::getClanReqString));
			view.put("saved", saved);
			view.put("settings", new JSONArray() {{
				add(entry.getValue().getSettings().getPublicChatSetting());
				add(entry.getValue().getSettings().getPrivateChatSetting());
				add(entry.getValue().getSettings().getTradeSetting());
			}});
			people.put(entry.getKey(), view);
		}
		Deque<Message> queue = managementQueue();
		JSONArray pending = new JSONArray();
		for (Message message : queue) {
			pending.add(describe(message));
		}
		Map<String, Object> waiting = new TreeMap<>();
		for (Map.Entry<String, Deque<Message>> entry : ((Map<String, Deque<Message>>) field(ManagementEvents.class, ManagementEvents.INSTANCE, "waitingOnClanInfo")).entrySet()) {
			JSONArray list = new JSONArray();
			for (Message message : entry.getValue()) {
				list.add(describe(message));
			}
			waiting.put(entry.getKey(), list);
		}
		Map<String, Object> requested = new TreeMap<>((Map<String, Boolean>) field(ManagementEvents.class, ManagementEvents.INSTANCE, "hasRequestedClanInfo"));
		JSONArray writes = new JSONArray();
		for (Object queued : (Collection) field(PacketWriteQueue.class, null, "packetsToWrite")) {
			Object out = queued.getClass().getMethod("getOut").invoke(queued);
			Object context = queued.getClass().getMethod("getContext").invoke(queued);
			JSONArray row = new JSONArray();
			row.add(out.getClass().getSimpleName());
			if (context instanceof ContactContext) {
				ContactContext c = (ContactContext) context;
				row.add(label(c.getPlayer()));
				row.add(c.getType());
				row.add(c.getName());
				row.add(c.getWorldId());
			} else if (context instanceof MessageContext) {
				MessageContext c = (MessageContext) context;
				row.add(label(c.getPlayer()));
				row.add(c.getOpcode());
				row.add(c.getOther());
				row.add(c.getChatIcon());
				row.add(c.getMessage());
			} else if (context instanceof ClanContext) {
				ClanContext c = (ClanContext) context;
				row.add(label(c.getPlayer()));
				row.add(clanId(c.getClan()));
				row.add(c.isLeave());
			}
			writes.add(row);
		}
		Map<String, Object> clans = new TreeMap<>();
		boolean grew = true;
		while (grew) {
			grew = false;
			for (Map.Entry<ClanRepository, Integer> entry : new ArrayList<>(clanIds.entrySet())) {
				String key = String.valueOf(entry.getValue());
				if (clans.containsKey(key)) {
					continue;
				}
				ClanRepository clan = entry.getKey();
				Map<String, Object> view = new LinkedHashMap<>();
				view.put("owner", clan.getOwner());
				view.put("name", clan.getName());
				JSONArray reqs = new JSONArray();
				for (ClanRank rank : new ClanRank[] {clan.getJoinRequirement(), clan.getMessageRequirement(), clan.getKickRequirement(), clan.getLootRequirement()}) {
					reqs.add(rank == null ? null : rank.ordinal());
				}
				view.put("reqs", reqs);
				Map<String, Object> ranks = new TreeMap<>();
				for (Map.Entry<String, ClanRank> rank : clan.getRanks().entrySet()) {
					ranks.put(String.valueOf(rank.getKey()), rank.getValue() == null ? null : rank.getValue().ordinal());
				}
				view.put("ranks", ranks);
				view.put("banned", new TreeMap<>(clan.getBanned()));
				JSONArray members = new JSONArray();
				for (ClanEntry member : clan.getPlayers()) {
					JSONArray row = new JSONArray();
					row.add(member.getName());
					row.add(member.getWorldId());
					row.add(label(member.getPlayer()));
					members.add(row);
				}
				view.put("players", members);
				clans.put(key, view);
				grew = true;
			}
		}
		state.put("registry", registry);
		state.put("players", people);
		state.put("pending", pending);
		state.put("waiting", waiting);
		state.put("requested", requested);
		state.put("writes", writes);
		state.put("clans", clans);
		return state;
	}

	// --- Actions.

	static Player player(Map action, String key) {
		Object label = action.get(key);
		return label == null ? null : players.get(label);
	}

	static ClanRepository clan(Map action) {
		if (action.containsKey("clanOf")) {
			return player(action, "clanOf").getCommunication().getClan();
		}
		return ClanRepository.getClans().get(action.get("clan"));
	}

	static byte[] encrypt(String text) {
		byte[] encoded = CP1252.toBytes(text);
		byte[] bytes = new byte[256];
		int length = StringUtils.encryptPlayerChat(bytes, 0, 0, encoded.length, encoded);
		byte[] out = new byte[length];
		System.arraycopy(bytes, 0, out, 0, length);
		return out;
	}

	static byte[] payload(Map action) {
		String op = (String) action.get("op");
		ByteBuffer buffer = ByteBuffer.allocate(512);
		if ("chat".equals(op)) {
			String text = (String) action.get("text");
			List effects = (List) action.get("effects");
			buffer.put((byte) integer(effects.get(0)));
			buffer.put((byte) integer(effects.get(1)));
			int count = action.containsKey("count") ? integer(action.get("count")) : text.length();
			if (count >= 128) {
				buffer.putShort((short) (count + 32768));
			} else {
				buffer.put((byte) count);
			}
			buffer.put(encrypt(text));
		} else if ("pm".equals(op)) {
			String text = (String) action.get("text");
			buffer.putLong(StringUtils.stringToLong((String) action.get("to")));
			int count = action.containsKey("count") ? integer(action.get("count")) : text.length();
			buffer.put((byte) count);
			buffer.put(encrypt(text));
		} else {
			String hex = (String) action.get("hex");
			for (int i = 0; i < hex.length(); i += 2) {
				buffer.put((byte) Integer.parseInt(hex.substring(i, i + 2), 16));
			}
		}
		byte[] out = new byte[buffer.position()];
		buffer.flip();
		buffer.get(out);
		return out;
	}

	static String hex(byte[] bytes) {
		StringBuilder out = new StringBuilder();
		for (byte b : bytes) {
			out.append(String.format("%02x", b & 0xff));
		}
		return out.toString();
	}

	static ClanRank rank(Map action) {
		Object value = action.get("rank");
		return value == null ? null : ClanRank.values()[integer(value)];
	}

	static Object perform(Map action, JSONObject step) throws Exception {
		String op = (String) action.get("op");
		Player p = player(action, "player");
		switch (op) {
			case "packet":
			case "chat":
			case "pm": {
				byte[] bytes = payload(action);
				int opcode = "chat".equals(op) ? 237 : "pm".equals(op) ? 201 : integer(action.get("opcode"));
				step.put("input", hex(bytes));
				IoBuffer buffer = new IoBuffer(opcode, PacketHeader.NORMAL, ByteBuffer.wrap(bytes));
				Packet packet = Decoders530.process(p, opcode, buffer);
				emit("decoded", describePacket(packet));
				if (!(packet instanceof Packet.NoProcess) && !(packet instanceof Packet.UnhandledOp) && !(packet instanceof Packet.DecodingError)) {
					PacketProcessor.oracleProcess(packet);
				}
				return null;
			}
			case "add":
				CommunicationInfo.add(p, (String) action.get("name"));
				return null;
			case "remove":
				CommunicationInfo.remove(p, (String) action.get("name"), Boolean.TRUE.equals(action.get("block")));
				return null;
			case "block":
				CommunicationInfo.block(p, (String) action.get("name"));
				return null;
			case "updateRank":
				CommunicationInfo.updateClanRank(p, (String) action.get("name"), rank(action));
				return null;
			case "hasContact":
				return CommunicationInfo.hasContact(p, (String) action.get("name"));
			case "showActive":
				if (action.containsKey("target")) {
					return CommunicationInfo.showActive(p, player(action, "target"));
				}
				return CommunicationInfo.showActive(p, (String) action.get("name"));
			case "sendMessage":
				CommunicationInfo.sendMessage(p, (String) action.get("name"), (String) action.get("text"));
				return null;
			case "toggleLoot":
				p.getCommunication().toggleLootshare(p);
				return null;
			case "runPulse":
				return pulses.get(integer(action.get("id"))).pulse();
			case "setClan":
				p.getCommunication().setClan(action.containsKey("clan") || action.containsKey("clanOf") ? clan(action) : null);
				return null;
			case "login":
				Login.oracleSendMSEvents(p.getDetails());
				return null;
			case "logout":
				p.oracleSendLogoutEvents();
				return null;
			case "removePlayer":
				Repository.getPlayers().remove(p);
				Repository.getPlayerNames().remove(p.getName());
				return null;
			case "addPlayer":
				Repository.getPlayers().add(p);
				Repository.getPlayerNames().put(p.getName(), p);
				return null;
			case "pump":
				pump();
				return null;
			case "flush":
				PacketWriteQueue.flush();
				return null;
			case "publish":
				ManagementEvents.publish(build((Map) action.get("event")));
				return null;
			case "clock":
				clock = ((Number) action.get("value")).longValue();
				return null;
			case "unlock":
				locks.computeIfAbsent((String) action.get("player"), k -> new ArrayList<>()).remove(action.get("key"));
				return null;
			case "attribute":
				attributes.computeIfAbsent((String) action.get("player"), k -> new HashMap<>()).put((String) action.get("key"), value(action.get("value")));
				return null;
			case "clanGet":
				return clanId(ClanRepository.get((String) action.get("owner"), Boolean.TRUE.equals(action.get("create"))));
			case "clanEnter":
				return clan(action).enter(p);
			case "clanLeave":
				if (action.containsKey("message")) {
					clan(action).leave(p, Boolean.TRUE.equals(action.get("remove")), (String) action.get("message"));
				} else {
					clan(action).leave(p, Boolean.TRUE.equals(action.get("remove")));
				}
				return null;
			case "clanKick":
				clan(action).kick(p, player(action, "target"));
				return null;
			case "clanMessage":
				clan(action).message(p, (String) action.get("text"));
				return null;
			case "clanRank":
				clan(action).rank((String) action.get("name"), rank(action));
				return null;
			case "clanClean":
				clan(action).clean(Boolean.TRUE.equals(action.get("disable")));
				return null;
			case "clanDelete":
				clan(action).delete();
				return null;
			case "clanUpdate":
				clan(action).update();
				return null;
			case "clanIsBanned":
				return clan(action).isBanned((String) action.get("name"));
			case "clanGetRank":
				ClanRank r = clan(action).getRank(p);
				return r == null ? null : r.ordinal();
			case "clanSet": {
				ClanRepository c = clan(action);
				String which = (String) action.get("which");
				if ("join".equals(which)) {
					c.setJoinRequirement(rank(action));
				} else if ("message".equals(which)) {
					c.setMessageRequirement(rank(action));
				} else if ("kick".equals(which)) {
					c.setKickRequirement(rank(action));
				} else if ("loot".equals(which)) {
					c.setLootRequirement(rank(action));
				} else {
					c.setName((String) action.get("name"));
				}
				return null;
			}
			case "openSettings":
				ClanRepository.openSettings(p);
				return null;
			case "defineGlobal":
				new GlobalChat().defineCommands();
				return null;
			case "command":
				commands.get(action.get("name")).invoke(p, new String[0]);
				return null;
			case "globalProcess":
				GlobalChat.Companion.process((String) action.get("sender"), (String) action.get("text"), integer(action.get("rights")));
				return null;
			case "split": {
				JSONArray out = new JSONArray();
				out.addAll(PacketProcessor.INSTANCE.splitChatMessage((String) action.get("text"), integer(action.get("clanLength")),
					Boolean.TRUE.equals(action.get("rank"))));
				return out;
			}
			case "chatMessage": {
				ChatMessage message = new ChatMessage(p, (String) action.get("text"), integer(action.get("effects")), integer(action.get("count")));
				JSONArray out = new JSONArray();
				out.add(label(message.getPlayer()));
				out.add(message.getText());
				out.add(message.getEffects());
				out.add(message.getNumChars());
				out.add(message.getChatIcon());
				out.add(message.isQuickChat);
				return out;
			}
			default:
				throw new IllegalArgumentException("unknown op " + op);
		}
	}

	static Object value(Object raw) {
		if (raw instanceof Map) {
			return Rights.values()[integer(((Map) raw).get("rights"))];
		}
		return number(raw);
	}

	public static String run(String line) {
		input = (Map) JSONValue.parse(line);
		world = (Map) input.get("world");
		clock = ((Number) input.getOrDefault("clock", 0L)).longValue();
		// UserAccountInfo.default reads the clock when the class initializes; that read is taken
		// before the case is recorded, so the case starts with the class already loaded.
		events = new ArrayList<>();
		Object init = UserAccountInfo.Companion.getDefault();
		events = new ArrayList<>();
		JSONObject result = new JSONObject();
		try {
			Map cast = (Map) input.get("players");
			for (Object key : new TreeMap<>(cast).keySet()) {
				String label = (String) key;
				Map spec = (Map) cast.get(label);
				PlayerDetails details = new PlayerDetails(str(spec, "username", str(spec, "name", label)));
				UserAccountInfo account = details.accountInfo;
				account.setRights(integer(spec.getOrDefault("rights", 0L)));
				account.setMuteEndTime(((Number) spec.getOrDefault("muteEnd", 0L)).longValue());
				account.setContacts(str(spec, "contacts", ""));
				account.setBlocked(str(spec, "blocked", ""));
				account.setClanName(str(spec, "clanName", ""));
				account.setCurrentClan(str(spec, "currentClan", ""));
				account.setClanReqs(str(spec, "clanReqs", "1,0,8,9"));
				details.getCommunication().parse(account);
				String kind = str(spec, "kind", "player");
				boolean session = !Boolean.FALSE.equals(spec.get("session"));
				Player player = "ai".equals(kind) ? new AIPlayer(label, (String) spec.get("name"), details, session)
					: new Player(label, (String) spec.get("name"), details, session, "artificial".equals(kind));
				List settings = (List) spec.get("settings");
				if (settings != null) {
					player.getSettings().oracleInitial(integer(settings.get(0)), integer(settings.get(1)), integer(settings.get(2)));
				}
				Map attrs = (Map) spec.get("attributes");
				if (attrs != null) {
					for (Object name : attrs.keySet()) {
						attributes.computeIfAbsent(label, k -> new HashMap<>()).put((String) name, value(attrs.get(name)));
					}
				}
				players.put(label, player);
			}
			List online = (List) input.get("online");
			for (Object label : online) {
				Player player = players.get(label);
				Repository.getPlayers().add(player);
				Repository.getPlayerNames().put(player.getName(), player);
			}
			result.put("setup", events);
			result.put("setupState", snapshot());
		} catch (Throwable t) {
			throw new RuntimeException("case setup failed", t);
		}
		JSONArray steps = new JSONArray();
		for (Object item : (List) input.get("actions")) {
			Map action = (Map) item;
			events = new ArrayList<>();
			JSONObject step = new JSONObject();
			Object value = null;
			String error = null;
			try {
				value = perform(action, step);
			} catch (Injected e) {
				error = "Injected:" + e.getMessage();
			} catch (Throwable t) {
				Throwable cause = t instanceof java.lang.reflect.InvocationTargetException ? t.getCause() : t;
				error = cause.getClass().getSimpleName();
				if (cause instanceof Injected) {
					error = "Injected:" + cause.getMessage();
				}
			}
			step.put("result", value);
			step.put("error", error);
			step.put("events", events);
			try {
				step.put("state", snapshot());
			} catch (Exception e) {
				throw new RuntimeException(e);
			}
			steps.add(step);
		}
		result.put("steps", steps);
		result.put("name", input.get("name"));
		return result.toJSONString();
	}
}
