package oracle;

import content.global.skill.prayer.Bones;
import core.api.utils.NPCDropTable;
import core.api.utils.WeightedItem;
import core.cache.def.impl.ItemDefinition;
import core.cache.def.impl.NPCDefinition;
import core.game.bots.AIPlayer;
import core.game.bots.GeneralBotCreator;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.DeathTask;
import core.game.node.entity.impl.PulseType;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.GroundItem;
import core.game.node.item.GroundItemManager;
import core.game.node.item.Item;
import core.game.node.item.ItemPlugin;
import core.game.system.task.Pulse;
import core.game.world.PulseRunner;
import core.game.world.map.Location;
import core.game.world.map.RegionChunk;
import core.game.world.update.flag.chunk.ItemUpdateFlag;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import core.tools.RandomFunction;
import core.tools.StringUtils;
import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.TreeMap;
import java.util.concurrent.ThreadLocalRandom;
import java.util.concurrent.atomic.AtomicLong;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives the unchanged NPCDropTables, GroundItemManager, GroundItem, DeathTask,
 * NodeTask, Pulse, PulseManager and PulseRunner over one case per input line
 * and records, per action, the result, the ordered service calls and the
 * state the original classes hold.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public final class Support {
	public static int tick;
	public static PulseRunner runner;
	public static RegionChunk chunk;
	public static List<Player> local;
	public static List<Player> repository;
	public static Map<Integer, Player> byUid;
	static List<Object> events = new ArrayList<>();
	static Map input;
	static int hashes;
	static List<Player> players;
	static List<NPC> npcs;
	static List<Player.Clan> clans;
	static Map<Integer, ItemDefinition> items;
	static Map<String, Object> prices;
	static int behaviorCalls;
	static Field threadSeed;
	static Field collectionsRandom;
	static Field randomState;

	/** An NPC subclass that overrides getDropLocation, as content subclasses do. */
	static final class Elsewhere extends NPC {
		Location drop;

		Elsewhere(int id) {
			super(id);
		}

		@Override
		public Location getDropLocation() {
			return drop;
		}
	}

	private Support() {
	}

	// ---- recording ----

	public static int nextHash() {
		return ++hashes;
	}

	public static void emit(String name, Object... arguments) {
		List<Object> event = new ArrayList<>();
		event.add(name);
		event.addAll(Arrays.asList(arguments));
		events.add(event);
	}

	public static List<Integer> location(Location location) {
		return location == null ? null : Arrays.asList(location.getX(), location.getY(), location.getZ());
	}

	public static List<Integer> item(Item item) {
		return item == null ? null : Arrays.asList(item.getId(), item.getAmount());
	}

	static String label(Entity entity) {
		return entity == null ? null : entity.label;
	}

	// ---- case inputs ----

	static int integer(Object value) {
		return ((Number) value).intValue();
	}

	static boolean flag(Map map, String key, boolean fallback) {
		Object value = map.get(key);
		return value == null ? fallback : (Boolean) value;
	}

	static Location place(Object value) {
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
		return text.charAt(0) == 'p' ? players.get(index) : npcs.get(index);
	}

	public static ItemDefinition itemDefinition(int id) {
		ItemDefinition definition = items.get(id);
		if (definition == null) {
			definition = new ItemDefinition();
			definition.id = id;
			items.put(id, definition);
		}
		return definition;
	}

	public static int price(int id) {
		Object value = prices.get(String.valueOf(id));
		return value == null ? 0 : integer(value);
	}

	public static Item replacement(Object value, Item item) {
		if (value == null) {
			return null;
		}
		if (value instanceof String) {
			return item;
		}
		List list = (List) value;
		return new Item(integer(list.get(0)), integer(list.get(1)));
	}

	/** npc.behavior.onDropTableRolled: record the list, then change it as the case says. */
	public static void rolled(NPC npc, Entity killer, ArrayList<Item> drops) {
		List<Object> list = new ArrayList<>();
		for (Item drop : drops) {
			list.add(item(drop));
		}
		emit("rolled", npc.label, killer.label, list);
		behaviorCalls++;
		Map behavior = (Map) input.get("behavior");
		if (behavior == null) {
			return;
		}
		if (behavior.get("failAt") != null && integer(behavior.get("failAt")) == behaviorCalls) {
			throw new IllegalStateException("behavior");
		}
		if (flag(behavior, "removeFirst", false) && !drops.isEmpty()) {
			drops.remove(0);
		}
		if (behavior.get("add") != null) {
			List add = (List) behavior.get("add");
			drops.add(new Item(integer(add.get(0)), integer(add.get(1))));
		}
	}

	static WeightedItem weighted(List row, int offset) {
		return new WeightedItem(integer(row.get(offset)), integer(row.get(offset + 1)), integer(row.get(offset + 2)),
			Double.longBitsToDouble(((Number) row.get(offset + 3)).longValue()), (Boolean) row.get(offset + 4));
	}

	static NPCDropTable table(List adds) {
		NPCDropTable table = new NPCDropTable();
		for (Object entry : adds) {
			List row = (List) entry;
			WeightedItem item = weighted(row, 1);
			switch ((String) row.get(0)) {
				case "charm":
					table.addToCharms(item);
					break;
				case "tertiary":
					table.addToTertiary(item);
					break;
				default:
					table.add(item);
			}
		}
		return table;
	}

	static void reset() throws Exception {
		hashes = 0;
		behaviorCalls = 0;
		tick = input.get("tick") == null ? 1000 : integer(input.get("tick"));
		runner = new PulseRunner();
		chunk = new RegionChunk();
		GroundItemManager.getItems().clear();
		items = new HashMap<>();
		prices = (Map) input.getOrDefault("prices", Collections.emptyMap());
		Map<String, Object> seeds = new HashMap<>();
		seeds.put("random", 530L);
		seeds.put("shuffle", 531L);
		seeds.put("tlr", 532L);
		seeds.putAll((Map) input.getOrDefault("seeds", Collections.emptyMap()));
		RandomFunction.RANDOM.setSeed(((Number) seeds.get("random")).longValue());
		collectionsRandom.set(null, new Random(((Number) seeds.get("shuffle")).longValue()));
		threadSeed.setLong(Thread.currentThread(), ((Number) seeds.get("tlr")).longValue());

		for (Object entry : ((Map) input.getOrDefault("items", Collections.emptyMap())).entrySet()) {
			Map.Entry row = (Map.Entry) entry;
			Map fields = (Map) row.getValue();
			ItemDefinition definition = new ItemDefinition();
			definition.id = Integer.parseInt((String) row.getKey());
			if (fields.containsKey("name")) {
				definition.name = (String) fields.get("name");
			}
			definition.stackable = flag(fields, "stackable", false);
			definition.unnoted = flag(fields, "unnoted", true);
			definition.noteId = fields.get("noteId") == null ? -1 : integer(fields.get("noteId"));
			if (fields.containsKey("options")) {
				List options = (List) fields.get("options");
				definition.options = options == null ? null : (String[]) options.toArray(new String[0]);
			}
			for (String key : new String[] {"tradeable", "rare_item"}) {
				if (fields.containsKey(key)) {
					Object value = fields.get(key);
					definition.handlers.put(key, value instanceof Number ? (Object) integer(value) : value);
				}
			}
			if (fields.get("plugin") != null) {
				Map config = (Map) fields.get("plugin");
				ItemPlugin plugin = new ItemPlugin();
				plugin.id = integer(config.get("id"));
				plugin.create = flag(config, "create", true);
				plugin.replacement = config.get("item");
				definition.handlers.put("wrapper", plugin);
			}
			items.put(definition.id, definition);
		}

		clans = new ArrayList<>();
		for (Object entry : (List) input.getOrDefault("clans", Collections.emptyList())) {
			Player.Clan clan = new Player.Clan();
			clan.index = clans.size();
			clan.lootRequirement = integer(entry);
			clans.add(clan);
		}
		players = new ArrayList<>();
		byUid = new HashMap<>();
		for (Object entry : (List) input.getOrDefault("players", Collections.emptyList())) {
			Map fields = (Map) entry;
			Player player = flag(fields, "ai", false) ? new AIPlayer() : new Player();
			player.label = "p" + players.size();
			player.accountName = fields.containsKey("name") ? (String) fields.get("name") : "Player" + players.size();
			player.uid = fields.get("uid") == null ? 100 + players.size() : integer(fields.get("uid"));
			player.setLocation(place(fields.getOrDefault("location", Arrays.asList(3200L, 3200L, 0L))));
			player.clan = fields.get("clan") == null ? null : clans.get(integer(fields.get("clan")));
			player.lootShare = flag(fields, "lootShare", false);
			player.lootRequirement = fields.get("lootReq") == null ? 0 : integer(fields.get("lootReq"));
			player.ironman = flag(fields, "ironman", false);
			player.lootSharePoints = fields.get("lsp") == null ? 0 : integer(fields.get("lsp"));
			player.resizable = flag(fields, "resizable", false);
			player.boneCrusher = flag(fields, "boneCrusher", false);
			player.wealth = flag(fields, "wealth", false);
			if (fields.get("bot") != null) {
				player.bot = new GeneralBotCreator.BotScriptPulse();
				player.bot.running = (Boolean) fields.get("bot");
			}
			if (fields.get("hash") != null) {
				player.hash = integer(fields.get("hash"));
			}
			player.zoneStart = flag(fields, "zoneStart", true);
			player.properties.spawnLocation = place(fields.getOrDefault("spawn", Arrays.asList(3222L, 3218L, 0L)));
			player.properties.safeRespawn = place(fields.get("safeRespawn"));
			player.properties.safeZone = flag(fields, "safeZone", false);
			if (fields.get("deathAnim") != null) {
				List anim = (List) fields.get("deathAnim");
				player.properties.deathAnimation = new Animation(integer(anim.get(0)), integer(anim.get(1)));
			}
			players.add(player);
			if (flag(fields, "online", true)) {
				byUid.put(player.uid, player);
			}
		}
		npcs = new ArrayList<>();
		for (Object entry : (List) input.getOrDefault("npcs", Collections.emptyList())) {
			Map fields = (Map) entry;
			int id = integer(fields.get("id"));
			NPC npc;
			if (fields.containsKey("dropLocation")) {
				Elsewhere elsewhere = new Elsewhere(id);
				elsewhere.drop = place(fields.get("dropLocation"));
				npc = elsewhere;
			} else {
				npc = new NPC(id);
			}
			npc.label = "n" + npcs.size();
			if (fields.get("originalId") != null) {
				npc.originalId = integer(fields.get("originalId"));
			}
			npc.setLocation(place(fields.getOrDefault("location", Arrays.asList(3210L, 3210L, 0L))));
			npc.properties.spawnLocation = fields.containsKey("spawn") ? place(fields.get("spawn")) : place(fields.getOrDefault("location", Arrays.asList(3210L, 3210L, 0L)));
			npc.properties.safeRespawn = place(fields.get("safeRespawn"));
			npc.properties.safeZone = flag(fields, "safeZone", false);
			npc.properties.multiZone = flag(fields, "multi", false);
			npc.setRespawn(flag(fields, "respawn", true));
			npc.zoneStart = flag(fields, "zoneStart", true);
			npc.zoneHandle = flag(fields, "zoneHandle", false);
			if (fields.get("hash") != null) {
				npc.hash = integer(fields.get("hash"));
			}
			if (fields.get("deathAnim") != null) {
				List anim = (List) fields.get("deathAnim");
				npc.properties.deathAnimation = new Animation(integer(anim.get(0)), integer(anim.get(1)));
			}
			if (fields.get("deathGfx") != null) {
				npc.properties.deathGfx = new Graphics(integer(fields.get("deathGfx")));
			}
			if (flag(fields, "disableDrop", false)) {
				npc.attributes.put("disable:drop", true);
			}
			if (flag(fields, "definition", true)) {
				NPCDefinition definition = new NPCDefinition(id);
				definition.name = (String) fields.get("name");
				Map config = (Map) fields.getOrDefault("config", Collections.emptyMap());
				for (Object row : config.entrySet()) {
					Map.Entry pair = (Map.Entry) row;
					Object value = pair.getValue();
					if (value instanceof List) {
						List list = (List) value;
						int[] array = new int[list.size()];
						for (int i = 0; i < array.length; i++) {
							array[i] = integer(list.get(i));
						}
						value = array;
					} else if (value instanceof Number) {
						value = integer(value);
					}
					definition.handlers.put((String) pair.getKey(), value);
				}
				Object tables = fields.get("tables");
				if ("null".equals(tables)) {
					definition.setDropTables(null);
				} else if (tables != null) {
					// The statement DropTableParser uses to hand a table to a definition.
					definition.getDropTables().table = table((List) tables);
				}
				npc.definition = definition;
			}
			npcs.add(npc);
		}
		local = players(input.get("local"));
		repository = players(input.get("repository"));
		if (repository == null) {
			repository = new ArrayList<>(players);
		}
	}

	static List<Player> players(Object value) {
		if (value == null) {
			return null;
		}
		List<Player> out = new ArrayList<>();
		for (Object index : (List) value) {
			out.add(index == null ? null : players.get(integer(index)));
		}
		return out;
	}

	// ---- actions ----

	static Object act(Map a) throws Exception {
		String op = (String) a.get("op");
		switch (op) {
			case "bones": {
				List<Object> out = new ArrayList<>();
				for (Bones bone : Bones.values()) {
					out.add(Arrays.asList(bone.getItemId(), String.valueOf(Double.doubleToRawLongBits(bone.getExperience())),
						Bones.forId(bone.getItemId()) == bone));
				}
				return out;
			}
			case "boneFor": {
				Bones bone = Bones.forId(integer(a.get("id")));
				return bone == null ? null : String.valueOf(Double.doubleToRawLongBits(bone.getExperience()));
			}
			case "format": {
				List<Object> out = new ArrayList<>();
				for (Object value : (List) a.get("values")) {
					out.add(StringUtils.getFormattedNumber(integer(value)));
				}
				return out;
			}
			case "drop": {
				NPC npc = npcs.get(integer(a.get("npc")));
				npc.definition.getDropTables().drop(npc, entity(a.get("looter")));
				return null;
			}
			case "createDrop": {
				NPC npc = npcs.get(integer(a.get("npc")));
				List row = (List) a.get("item");
				Item item = row == null ? null : new Item(integer(row.get(0)), integer(row.get(1)));
				Player player = a.get("player") == null ? null : players.get(integer(a.get("player")));
				npc.definition.getDropTables().createDrop(item, player, npc, place(a.get("location")));
				return null;
			}
			case "getLooter": {
				NPC npc = npcs.get(integer(a.get("npc")));
				List row = (List) a.get("item");
				Player looter = npc.definition.getDropTables().getLooter(players.get(integer(a.get("player"))), npc,
					new Item(integer(row.get(0)), integer(row.get(1))));
				return label(looter);
			}
			case "hit": {
				Entity target = entity(a.get("target"));
				target.lifepoints = flag(a, "lethal", false) ? 0 : 1;
				target.getImpactHandler().impact(entity(a.get("source")), integer(a.get("amount")));
				return null;
			}
			case "credit":
				return label(entity(a.get("target")).getImpactHandler().getMostDamageEntity(entity(a.get("killer"))));
			case "startDeath":
				entity(a.get("target")).startDeath(entity(a.get("killer")));
				return null;
			case "isDead":
				return DeathTask.isDead(entity(a.get("target")));
			case "strong": {
				// Another strong pulse already holds the entity's pulse manager.
				Pulse pulse = new Pulse(integer(a.get("delay"))) {
					@Override
					public boolean pulse() {
						return true;
					}
				};
				entity(a.get("target")).getPulseManager().run(pulse, PulseType.STRONG);
				return null;
			}
			case "tick": {
				int count = a.get("count") == null ? 1 : integer(a.get("count"));
				for (int i = 0; i < count; i++) {
					tick++;
					emit("tick", tick);
					runner.updateAll();
					if (flag(a, "npcs", false)) {
						for (NPC npc : npcs) {
							npc.tick();
						}
					}
				}
				return null;
			}
			case "groundPulse":
				GroundItemManager.pulse();
				return null;
			case "set": {
				if (a.get("online") != null) {
					Player player = players.get(integer(a.get("player")));
					if ((Boolean) a.get("online")) {
						byUid.put(player.uid, player);
					} else {
						byUid.remove(player.uid);
					}
				}
				if (a.get("active") != null) {
					entity(a.get("target")).active = (Boolean) a.get("active");
				}
				if (a.containsKey("disableDrop")) {
					entity(a.get("target")).attributes.put("disable:drop", a.get("disableDrop"));
				}
				if (a.get("tick") != null) {
					tick = integer(a.get("tick"));
				}
				return null;
			}
			default:
				throw new IllegalArgumentException(op);
		}
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
		if (error instanceof IllegalStateException && "behavior".equals(error.getMessage())) {
			return "Behavior";
		}
		if (error instanceof IllegalArgumentException) {
			return "BadWeight";
		}
		return error.getClass().getName();
	}

	// ---- state ----

	static long probe(Random random) throws Exception {
		Random copy = new Random(0);
		((AtomicLong) randomState.get(copy)).set(((AtomicLong) randomState.get(random)).get());
		return copy.nextLong();
	}

	static Map<String, Object> attributes(Entity entity) {
		Map<String, Object> out = new TreeMap<>();
		for (Map.Entry<String, Object> entry : entity.attributes.entrySet()) {
			if (!entry.getKey().startsWith("botting:")) {
				out.put(entry.getKey(), entry.getValue());
			}
		}
		return out;
	}

	static Map<String, Object> impacts(Entity entity) {
		Map<String, Object> out = new TreeMap<>();
		List<Object> playerLog = new ArrayList<>();
		for (Map.Entry<Integer, Integer> entry : entity.getImpactHandler().getPlayerImpactLog().entrySet()) {
			playerLog.add(Arrays.asList(entry.getKey(), entry.getValue()));
		}
		List<Object> npcLog = new ArrayList<>();
		for (Map.Entry<Entity, Integer> entry : entity.getImpactHandler().getNpcImpactLog().entrySet()) {
			npcLog.add(Arrays.asList(label(entry.getKey()), entry.getValue()));
		}
		out.put("attributes", attributes(entity));
		out.put("disabledTicks", entity.getImpactHandler().getDisabledTicks());
		out.put("playerLog", playerLog);
		out.put("npcLog", npcLog);
		out.put("active", entity.active);
		return out;
	}

	static Map<String, Object> state() throws Exception {
		Map<String, Object> out = new TreeMap<>();
		out.put("tick", tick);
		List<Object> grounds = new ArrayList<>();
		for (GroundItem ground : GroundItemManager.getItems()) {
			grounds.add(Arrays.asList(ground.getId(), ground.getAmount(), ground.getCharge(), location(((core.game.node.Node) ground).getLocation()),
				label(ground.getDropper()), ground.getDropperUid(), ground.getTicks(), ground.getDecayTime(), ground.isRemainPrivate(),
				ground.isRemoved()));
		}
		out.put("grounds", grounds);
		List<Object> chunkItems = new ArrayList<>();
		for (GroundItem ground : chunk.getItems()) {
			chunkItems.add(item(ground));
		}
		out.put("chunk", chunkItems);
		List<Object> flags = new ArrayList<>();
		for (ItemUpdateFlag flag : chunk.flags) {
			flags.add(Arrays.asList(item(flag.item), flag.type));
		}
		out.put("flags", flags);
		List<Object> npcStates = new ArrayList<>();
		for (NPC npc : npcs) {
			Map<String, Object> row = impacts(npc);
			row.put("respawnTick", npc.getRespawnTick());
			row.put("respawning", npc.isRespawning);
			npcStates.add(row);
		}
		out.put("npcs", npcStates);
		List<Object> playerStates = new ArrayList<>();
		for (Player player : players) {
			Map<String, Object> row = impacts(player);
			row.put("lsp", player.lootSharePoints);
			playerStates.add(row);
		}
		out.put("players", playerStates);
		out.put("queued", runner.getCurrentPulses().length);
		out.put("rng", Arrays.asList(String.valueOf(probe(RandomFunction.RANDOM)), String.valueOf(probe((Random) collectionsRandom.get(null))),
			String.valueOf(threadSeed.getLong(Thread.currentThread()))));
		return out;
	}

	static Field field(Class<?> owner, String name) throws Exception {
		Field field = owner.getDeclaredField(name);
		field.setAccessible(true);
		return field;
	}

	public static void main(String[] args) throws Exception {
		ThreadLocalRandom.current();
		threadSeed = field(Thread.class, "threadLocalRandomSeed");
		collectionsRandom = field(Collections.class, "r");
		randomState = field(Random.class, "seed");

		PrintStream output = new PrintStream(System.out, false, "UTF-8");
		// Pulse.update and PulseRunner print a caught exception's stack trace:
		// record that it happened, and where among the other calls.
		System.setErr(new PrintStream(new ByteArrayOutputStream(), true, "UTF-8") {
			@Override
			public void println(Object value) {
				if (value instanceof Throwable) {
					emit("stackTrace", category((Throwable) value));
				}
			}
		});

		try (BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
			String line;
			while ((line = reader.readLine()) != null) {
				input = (Map) new JSONParser().parse(line);
				events = new ArrayList<>();
				reset();
				List<Object> steps = new ArrayList<>();
				for (Object entry : (List) input.get("actions")) {
					events = new ArrayList<>();
					Object value = null;
					String error = null;
					try {
						value = act((Map) entry);
					} catch (Throwable e) {
						error = category(e);
					}
					Map<String, Object> step = new TreeMap<>();
					step.put("value", value);
					step.put("error", error);
					step.put("events", events);
					events = new ArrayList<>();
					step.put("state", state());
					if (!events.isEmpty()) {
						throw new IllegalStateException("the snapshot made service calls: " + events);
					}
					steps.add(step);
				}
				input.put("expected", steps);
				output.println(JSONValue.toJSONString(input));
			}
		}
		output.flush();
	}
}
