package oracle;

import content.data.tables.AllotmentSeedDropTable;
import content.data.tables.CELEMinorTable;
import content.data.tables.GemDropTable;
import content.data.tables.HerbDropTable;
import content.data.tables.RareDropTable;
import content.data.tables.RareSeedDropTable;
import content.data.tables.UncommonSeedDropTable;
import content.global.activity.ttrail.ClueLevel;
import content.global.activity.ttrail.ClueScrollPlugin;
import core.ServerConstants;
import core.api.StartupListener;
import core.api.utils.NPCDropTable;
import core.api.utils.WeightBasedTable;
import core.api.utils.WeightedItem;
import core.cache.def.impl.NPCDefinition;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.npc.drop.DropFrequency;
import core.game.node.entity.npc.drop.NPCDropTables;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.node.item.WeightedChanceItem;
import core.game.system.config.DropTableParser;
import core.tools.Log;
import core.tools.RandomFunction;
import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.FileNotFoundException;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
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
 * Drives the unchanged drop-table classes. Every case seeds the three generators
 * the original draws from (RandomFunction.RANDOM, the generator private to
 * Collections.shuffle(List), and the calling thread's ThreadLocalRandom) and
 * reports, per action, how far each one advanced.
 */
public final class Support {
	static final long MULTIPLIER = 0x5DEECE66DL;
	static final long MASK = (1L << 48) - 1;
	static final long GAMMA = 0x9e3779b97f4a7c15L;
	static final String[] SHARED = {"RDT", "CELEDT", "USDT", "HDT", "GDT", "RSDT", "ASDT"};

	static Field threadSeed;
	static Field collectionsRandom;
	static Field randomState;
	static Field charmDrops;
	static Field tertiaryDrops;
	static Field rareTable;
	static final Map<String, Field> chanceFields = new HashMap<>();
	static ByteArrayOutputStream errors;

	static String folder;
	static String pinnedConfig;
	static String pinnedShared;

	static Map input;
	static Map action;
	static List<Object> events;
	static Map<Integer, NPCDefinition> definitions;
	static Map<String, WeightBasedTable> tables;

	public static final class Failure extends RuntimeException {
	}

	// ---- services called by the adapters ----

	public static NPCDefinition forId(int id) {
		emit("forId", id);
		return lookup(id);
	}

	static NPCDefinition lookup(int id) {
		Map options = (Map) input.getOrDefault("definitions", Collections.emptyMap());
		if (longs(options.get("throwing")).contains((long) id)) {
			throw new Failure();
		}
		if (longs(options.get("missing")).contains((long) id)) {
			return null;
		}
		NPCDefinition definition = definitions.get(id);
		if (definition == null) {
			definition = new NPCDefinition(id);
			Map names = (Map) options.getOrDefault("names", Collections.emptyMap());
			String key = Integer.toString(id);
			definition.name = names.containsKey(key) ? (String) names.get(key) : "NPC " + id;
			Map combat = (Map) options.getOrDefault("combat", Collections.emptyMap());
			definition.combatLevel = combat.containsKey(key) ? ((Number) combat.get(key)).intValue() : 0;
			if (longs(options.get("nullTables")).contains((long) id)) {
				definition.setDropTables(null);
			}
			definitions.put(id, definition);
		}
		return definition;
	}

	public static void log(Class<?> origin, Log type, String message) {
		int end = message.length();
		for (int i = 0; i < message.length(); i++) {
			char c = message.charAt(i);
			if (c == '\r' || c == '\n') {
				end = i;
				break;
			}
		}
		// Only the first line: the rest of an exception text is JVM stack frames.
		emit("log", origin.getSimpleName(), type.name(), message.substring(0, end));
	}

	public static boolean removesNothings(Player player) {
		emit("removesNothings");
		return player.wealth;
	}

	public static boolean inEquipment(Player player, int id, int amount) {
		emit("inEquipment", id, amount);
		return player.equipped.contains((long) id);
	}

	public static int fogCharges(Player player, int charges) {
		emit("fogCharges", charges);
		return charges;
	}

	public static void rolled(NPC self, Entity killer, ArrayList<Item> drops) {
		emit("rolled", self.id, items(drops));
		Map behavior = (Map) action.get("behavior");
		if (behavior == null) {
			return;
		}
		if (Boolean.TRUE.equals(behavior.get("removeFirst")) && !drops.isEmpty()) {
			drops.remove(0);
		}
		if (behavior.get("add") != null) {
			List add = (List) behavior.get("add");
			drops.add(new Item(integer(add.get(0)), integer(add.get(1))));
		}
		if (Boolean.TRUE.equals(behavior.get("fail"))) {
			throw new Failure();
		}
	}

	public static boolean hasSpaceFor(Player player, Item[] items) {
		emit("hasSpaceFor", items(Arrays.asList(items)));
		return player.hasSpace;
	}

	public static boolean isFull(Player player) {
		emit("isFull");
		return player.full;
	}

	public static String itemName(int id) {
		Map names = (Map) action.getOrDefault("names", Collections.emptyMap());
		String key = Integer.toString(id);
		return names.containsKey(key) ? (String) names.get(key) : "Item " + id;
	}

	// ---- recording ----

	static void emit(String kind, Object... values) {
		stackTraces();
		List<Object> row = new ArrayList<>();
		row.add(kind);
		row.addAll(Arrays.asList(values));
		events.add(row);
	}

	/** An original catch block that printed a stack trace leaves one "stackTrace" event. */
	static void stackTraces() {
		System.err.flush();
		if (errors.size() == 0) {
			return;
		}
		String text = new String(errors.toByteArray(), StandardCharsets.UTF_8);
		errors.reset();
		String found = "Unknown";
		for (String line : text.split("\\r?\\n")) {
			String name = line.split("[:;\\s]", 2)[0];
			if (name.matches("[\\w.$]+(Exception|Error)")) {
				found = name.endsWith("NumberFormatException") ? "Number"
					: name.endsWith("SAXParseException") ? "Xml"
					: name.endsWith("FileNotFoundException") ? "File"
					: name.endsWith("IllegalArgumentException") ? "Argument"
					: name;
				break;
			}
		}
		List<Object> row = new ArrayList<>();
		row.add("stackTrace");
		row.add(found);
		events.add(row);
	}

	static String category(Throwable e) {
		if (e instanceof Failure) return "Binding";
		if (e instanceof NumberFormatException) return "Number";
		if (e instanceof NullPointerException) return "Null";
		if (e instanceof ClassCastException) return "Type";
		if (e instanceof org.json.simple.parser.ParseException) return "Json";
		if (e instanceof FileNotFoundException) return "File";
		if (e instanceof IllegalArgumentException) return "Argument";
		if (e instanceof IndexOutOfBoundsException) return "Index";
		if (e instanceof ArithmeticException) return "Arithmetic";
		if (e instanceof StackOverflowError) return "Depth";
		return e.getClass().getName();
	}

	static List<Object> items(List<Item> items) {
		List<Object> out = new ArrayList<>();
		for (Item item : items) {
			out.add(item == null ? null : Arrays.asList(item.getId(), item.getAmount()));
		}
		return out;
	}

	static List<Object> weighted(List<WeightedItem> items) {
		List<Object> out = new ArrayList<>();
		for (WeightedItem item : items) {
			out.add(Arrays.asList(item.getId(), item.getMinAmt(), item.getMaxAmt(),
				Double.doubleToLongBits(item.getWeight()), item.getGuaranteed()));
		}
		return out;
	}

	static Map<String, Object> snapshot(WeightBasedTable table) throws Exception {
		Map<String, Object> out = new TreeMap<>();
		out.put("items", weighted(table));
		out.put("total", Double.doubleToLongBits(table.getTotalWeight()));
		out.put("guaranteed", weighted(table.getGuaranteedItems()));
		if (table instanceof NPCDropTable) {
			out.put("charms", snapshot((WeightBasedTable) charmDrops.get(table)));
			out.put("tertiary", snapshot((WeightBasedTable) tertiaryDrops.get(table)));
		}
		return out;
	}

	static Object sharedSnapshot(String name) throws Exception {
		if (name.equals("RDT")) {
			return snapshot((WeightBasedTable) rareTable.get(null));
		}
		List<Object> out = new ArrayList<>();
		for (Object entry : (List) chanceFields.get(name).get(null)) {
			WeightedChanceItem item = (WeightedChanceItem) entry;
			out.add(Arrays.asList(chance(item, "id"), chance(item, "minimum_amount"), chance(item, "maximum_amount"), item.weight));
		}
		return out;
	}

	static int chance(WeightedChanceItem item, String name) throws Exception {
		Field field = WeightedChanceItem.class.getDeclaredField(name);
		field.setAccessible(true);
		return field.getInt(item);
	}

	// ---- generator state ----

	static long randomState(Random random) throws Exception {
		return ((AtomicLong) randomState.get(random)).get();
	}

	static long[] states() throws Exception {
		return new long[] {
			randomState(RandomFunction.RANDOM),
			randomState((Random) collectionsRandom.get(null)),
			threadSeed.getLong(Thread.currentThread()),
		};
	}

	static long steps(long from, long to, boolean lcg) {
		long count = 0;
		while (from != to) {
			from = lcg ? (from * MULTIPLIER + 0xBL) & MASK : from + GAMMA;
			if (++count > 50_000_000L) {
				throw new IllegalStateException("generator position not found");
			}
		}
		return count;
	}

	static void seed(Map seeds) throws Exception {
		if (seeds.get("random") != null) {
			RandomFunction.RANDOM.setSeed(((Number) seeds.get("random")).longValue());
		}
		if (seeds.get("shuffle") != null) {
			collectionsRandom.set(null, new Random(((Number) seeds.get("shuffle")).longValue()));
		}
		if (seeds.get("tlr") != null) {
			threadSeed.setLong(Thread.currentThread(), ((Number) seeds.get("tlr")).longValue());
		}
	}

	// ---- case inputs ----

	static List<Long> longs(Object value) {
		List<Long> out = new ArrayList<>();
		if (value != null) {
			for (Object entry : (List) value) {
				out.add(((Number) entry).longValue());
			}
		}
		return out;
	}

	static int integer(Object value) {
		return ((Number) value).intValue();
	}

	static WeightedItem weightedItem(List row, int offset) {
		return new WeightedItem(integer(row.get(offset)), integer(row.get(offset + 1)), integer(row.get(offset + 2)),
			Double.longBitsToDouble(((Number) row.get(offset + 3)).longValue()), (Boolean) row.get(offset + 4));
	}

	static Entity receiver(Object value) {
		if (value == null) {
			return null;
		}
		Map spec = (Map) value;
		if (!"player".equals(spec.get("kind"))) {
			return new Entity();
		}
		Player player = new Player();
		player.wealth = Boolean.TRUE.equals(spec.get("wealth"));
		player.equipped = longs(spec.get("equipped"));
		player.hasSpace = Boolean.TRUE.equals(spec.get("hasSpace"));
		player.full = Boolean.TRUE.equals(spec.get("full"));
		return player;
	}

	static void clues(String level, Object ids) {
		if (ids == null) {
			ClueScrollPlugin.ORGANIZED.remove(ClueLevel.valueOf(level));
			return;
		}
		List<ClueScrollPlugin> list = new ArrayList<>(20);
		for (long id : longs(ids)) {
			list.add(new ClueScrollPlugin((int) id));
		}
		ClueScrollPlugin.ORGANIZED.put(ClueLevel.valueOf(level), list);
	}

	static String file(String name, Map a, String pinned) throws Exception {
		String path = folder + "/" + name;
		String text = Boolean.TRUE.equals(a.get("pinned")) ? pinned : (String) a.get("text");
		Files.write(Paths.get(path), text.getBytes(StandardCharsets.UTF_8));
		return path;
	}

	static void setSharedPath(String name, String path) {
		switch (name) {
			case "RDT": ServerConstants.RDT_DATA_PATH = path; break;
			case "CELEDT": ServerConstants.CELEDT_DATA_PATH = path; break;
			case "USDT": ServerConstants.USDT_DATA_PATH = path; break;
			case "HDT": ServerConstants.HDT_DATA_PATH = path; break;
			case "GDT": ServerConstants.GDT_DATA_PATH = path; break;
			case "RSDT": ServerConstants.RSDT_DATA_PATH = path; break;
			case "ASDT": ServerConstants.ASDT_DATA_PATH = path; break;
			default: throw new IllegalArgumentException(name);
		}
	}

	static void parseShared(String name, String path) {
		switch (name) {
			case "RDT": RareDropTable.Companion.parse(path); break;
			case "CELEDT": CELEMinorTable.parse(path); break;
			case "USDT": UncommonSeedDropTable.parse(path); break;
			case "HDT": HerbDropTable.parse(path); break;
			case "GDT": GemDropTable.parse(path); break;
			case "RSDT": RareSeedDropTable.parse(path); break;
			case "ASDT": AllotmentSeedDropTable.parse(path); break;
			default: throw new IllegalArgumentException(name);
		}
	}

	static StartupListener listener(String name) throws Exception {
		switch (name) {
			case "RDT": return new RareDropTable();
			case "CELEDT": return new CELEMinorTable();
			case "USDT": return new UncommonSeedDropTable();
			case "HDT": return new HerbDropTable();
			case "GDT": return new GemDropTable();
			case "RSDT": return new RareSeedDropTable();
			case "ASDT": return new AllotmentSeedDropTable();
			default: throw new IllegalArgumentException(name);
		}
	}

	static Item retrieve(String name, Entity receiver) {
		switch (name) {
			case "RDT": return RareDropTable.Companion.retrieve(receiver);
			case "CELEDT": return CELEMinorTable.retrieve();
			case "USDT": return UncommonSeedDropTable.retrieve();
			case "HDT": return HerbDropTable.retrieve();
			case "GDT": return GemDropTable.retrieve();
			case "RSDT": return RareSeedDropTable.retrieve();
			case "ASDT": return AllotmentSeedDropTable.retrieve();
			default: throw new IllegalArgumentException(name);
		}
	}

	static WeightBasedTable insert(WeightBasedTable table, String name, double weight) {
		switch (name) {
			case "EasyClue": return table.insertEasyClue(weight);
			case "MediumClue": return table.insertMediumClue(weight);
			case "HardClue": return table.insertHardClue(weight);
			case "RDTRoll": return table.insertRDTRoll(weight);
			case "CELEDTRoll": return table.insertCELEDTRoll(weight);
			case "SEEDDTRoll": return table.insertSEEDDTRoll(weight);
			case "HERBDTRoll": return table.insertHERBDTRoll(weight);
			case "GDTRoll": return table.insertGDTRoll(weight);
			case "RSDTRoll": return table.insertRSDTRoll(weight);
			case "ASDTRoll": return table.insertASDTRoll(weight);
			default: throw new IllegalArgumentException(name);
		}
	}

	// ---- actions ----

	static Object act(Map a) throws Exception {
		String op = (String) a.get("op");
		switch (op) {
			case "seed":
				seed(a);
				return null;
			case "load": {
				if (Boolean.TRUE.equals(a.get("missingFile"))) {
					ServerConstants.Companion.setCONFIG_PATH(folder + "/missing-");
				} else {
					file("drop_tables.json", a, pinnedConfig);
					ServerConstants.Companion.setCONFIG_PATH(folder + "/");
				}
				new DropTableParser().load();
				return null;
			}
			case "tables": {
				// Distinct table objects in ascending NPC id order of first use.
				List<Object> snapshots = new ArrayList<>();
				List<NPCDropTable> seen = new ArrayList<>();
				List<Object> npcs = new ArrayList<>();
				for (Map.Entry<Integer, NPCDefinition> entry : new TreeMap<>(definitions).entrySet()) {
					NPCDropTables holder = entry.getValue().getDropTables();
					Object index = null;
					if (holder != null) {
						int found = -1;
						for (int i = 0; i < seen.size(); i++) {
							if (seen.get(i) == holder.table) {
								found = i;
							}
						}
						if (found < 0) {
							found = seen.size();
							seen.add(holder.table);
							snapshots.add(snapshot(holder.table));
						}
						index = found;
					}
					npcs.add(Arrays.asList(entry.getKey(), index));
				}
				Map<String, Object> out = new TreeMap<>();
				out.put("tables", snapshots);
				out.put("npcs", npcs);
				return out;
			}
			case "roll": {
				int id = integer(a.get("npc"));
				return items(lookup(id).getDropTables().roll(new NPC(id), receiver(a.get("receiver")), integer(a.get("times"))));
			}
			case "table": {
				String kind = (String) a.get("kind");
				List adds = (List) a.get("adds");
				WeightBasedTable table;
				if (kind.equals("created")) {
					WeightedItem[] all = new WeightedItem[adds.size()];
					for (int i = 0; i < all.length; i++) {
						all[i] = weightedItem((List) adds.get(i), 1);
					}
					table = WeightBasedTable.create(all);
				} else {
					table = kind.equals("npc") ? new NPCDropTable() : new WeightBasedTable();
					for (Object entry : adds) {
						List row = (List) entry;
						switch ((String) row.get(0)) {
							case "main": table.add(weightedItem(row, 1)); break;
							case "charm": ((NPCDropTable) table).addToCharms(weightedItem(row, 1)); break;
							case "tertiary": ((NPCDropTable) table).addToTertiary(weightedItem(row, 1)); break;
							case "insert": insert(table, (String) row.get(1), Double.longBitsToDouble(((Number) row.get(2)).longValue())); break;
							default: throw new IllegalArgumentException((String) row.get(0));
						}
					}
				}
				tables.put((String) a.get("name"), table);
				return snapshot(table);
			}
			case "assign": {
				lookup(integer(a.get("npc"))).getDropTables().table = (NPCDropTable) tables.get(a.get("table"));
				return null;
			}
			case "rollTable": {
				WeightBasedTable table = tables.get(a.get("table"));
				Entity receiver = receiver(a.get("receiver"));
				return items(a.get("times") == null ? table.roll(receiver) : table.roll(receiver, integer(a.get("times"))));
			}
			case "convert": {
				ArrayList<WeightedItem> list = new ArrayList<>();
				for (Object row : (List) a.get("items")) {
					list.add(weightedItem((List) row, 0));
				}
				return items(tables.get(a.get("table")).convertWeightedItems(list, receiver(a.get("receiver"))));
			}
			case "excluding": {
				List<Long> ids = longs(a.get("ids"));
				int[] raw = new int[ids.size()];
				for (int i = 0; i < raw.length; i++) {
					raw[i] = ids.get(i).intValue();
				}
				WeightBasedTable out = tables.get(a.get("table")).excluding(raw);
				tables.put((String) a.get("as"), out);
				return snapshot(out);
			}
			case "canRoll":
				return tables.get(a.get("table")).canRoll((Player) receiver(a.get("receiver")));
			case "describe":
				return tables.get(a.get("table")).toString();
			case "snapshot":
				return snapshot(tables.get(a.get("table")));
			case "shared": {
				String name = (String) a.get("table");
				parseShared(name, file(name + ".xml", a, Boolean.TRUE.equals(a.get("pinned")) ? pinned(name) : null));
				stackTraces();
				return sharedSnapshot(name);
			}
			case "startup": {
				String name = (String) a.get("table");
				String state = (String) a.get("path");
				String path = null;
				if (state.equals("present")) {
					path = file(name + ".xml", a, Boolean.TRUE.equals(a.get("pinned")) ? pinned(name) : null);
				} else if (state.equals("missing")) {
					path = folder + "/missing-" + name + ".xml";
				}
				setSharedPath(name, path);
				listener(name).startup();
				stackTraces();
				return sharedSnapshot(name);
			}
			case "rareAdd": {
				// The rare table is private and its parser never makes a guaranteed
				// entry; this reaches its own add through reflection.
				((WeightBasedTable) rareTable.get(null)).add(weightedItem((List) a.get("item"), 0));
				return sharedSnapshot("RDT");
			}
			case "sharedSnapshot":
				return sharedSnapshot((String) a.get("table"));
			case "retrieve": {
				Item item = retrieve((String) a.get("table"), receiver(a.get("receiver")));
				return item == null ? null : Arrays.asList(item.getId(), item.getAmount());
			}
			case "clues":
				clues((String) a.get("level"), a.get("ids"));
				return null;
			case "clue": {
				Item item = ClueScrollPlugin.getClue(ClueLevel.valueOf((String) a.get("level")));
				return item == null ? null : Arrays.asList(item.getId(), item.getAmount());
			}
			case "random": {
				switch ((String) a.get("fn")) {
					case "range": return RandomFunction.random(integer(a.get("a")), integer(a.get("b")));
					case "max": return RandomFunction.random(integer(a.get("n")));
					case "double": return Double.doubleToLongBits(RandomFunction.randomDouble(Double.longBitsToDouble(((Number) a.get("bits")).longValue())));
					case "bool": return RandomFunction.nextBool();
					default: throw new IllegalArgumentException((String) a.get("fn"));
				}
			}
			case "stabilizer": {
				NPCDefinition definition = new NPCDefinition(-1);
				definition.combatLevel = integer(a.get("combat"));
				return Double.doubleToLongBits(new NPCDropTables(definition).getStabilizerRatio());
			}
			case "modRate": {
				NPCDropTables holder = new NPCDropTables(new NPCDefinition(-1));
				List<Object> out = new ArrayList<>();
				out.add(Double.doubleToLongBits(holder.getModRate()));
				holder.setModRate(Double.longBitsToDouble(((Number) a.get("bits")).longValue()));
				out.add(Double.doubleToLongBits(holder.getModRate()));
				return out;
			}
			case "constants": {
				Map<String, Object> out = new TreeMap<>();
				List<Object> rates = new ArrayList<>();
				for (int rate : NPCDropTables.DROP_RATES) rates.add(rate);
				out.put("dropRates", rates);
				List<Object> message = new ArrayList<>();
				for (int id : NPCDropTables.MESSAGE_NPCS) message.add(id);
				out.put("messageNpcs", message);
				List<Object> frequencies = new ArrayList<>();
				for (DropFrequency frequency : DropFrequency.values()) {
					frequencies.add(Arrays.asList(frequency.name(), frequency.ordinal(), DropFrequency.rate(frequency)));
				}
				out.put("frequencies", frequencies);
				Map<String, Object> slots = new TreeMap<>();
				slots.put("RDT", WeightBasedTable.SLOT_RDT);
				slots.put("CLUE_EASY", WeightBasedTable.Companion.getSLOT_CLUE_EASY());
				slots.put("CLUE_MEDIUM", WeightBasedTable.Companion.getSLOT_CLUE_MEDIUM());
				slots.put("CLUE_HARD", WeightBasedTable.Companion.getSLOT_CLUE_HARD());
				slots.put("CELEDT", WeightBasedTable.Companion.getSLOT_CELEDT());
				slots.put("USDT", WeightBasedTable.Companion.getSLOT_USDT());
				slots.put("HDT", WeightBasedTable.Companion.getSLOT_HDT());
				slots.put("GDT", WeightBasedTable.Companion.getSLOT_GDT());
				slots.put("RSDT", WeightBasedTable.Companion.getSLOT_RSDT());
				slots.put("ASDT", WeightBasedTable.Companion.getSLOT_ASDT());
				out.put("slots", slots);
				List<Object> clueSlots = new ArrayList<>();
				for (int id : WeightBasedTable.Companion.getCLUE_SCROLL_SLOTS()) clueSlots.add(id);
				out.put("clueSlots", clueSlots);
				List<Object> sharedSlots = new ArrayList<>();
				sharedSlots.add(HerbDropTable.SLOT_ITEM_ID);
				sharedSlots.add(GemDropTable.SLOT_ITEM_ID);
				sharedSlots.add(CELEMinorTable.SLOT_ITEM_ID);
				sharedSlots.add(UncommonSeedDropTable.SLOT_ITEM_ID);
				sharedSlots.add(RareSeedDropTable.SLOT_ITEM_ID);
				sharedSlots.add(AllotmentSeedDropTable.SLOT_ITEM_ID);
				out.put("sharedSlotIds", sharedSlots);
				return out;
			}
			default:
				throw new IllegalArgumentException(op);
		}
	}

	static String pinned(String name) throws Exception {
		return new String(Files.readAllBytes(Paths.get(pinnedShared, name + ".xml")), StandardCharsets.UTF_8);
	}

	static void reset() throws Exception {
		definitions = new HashMap<>();
		tables = new HashMap<>();
		WeightBasedTable rare = (WeightBasedTable) rareTable.get(null);
		rare.clear();
		rare.setTotalWeight(0.0);
		rare.getGuaranteedItems().clear();
		for (Field field : chanceFields.values()) {
			((List) field.get(null)).clear();
		}
		ClueScrollPlugin.ORGANIZED.clear();
		ServerConstants.Companion.setCONFIG_PATH(null);
		for (String name : SHARED) {
			setSharedPath(name, null);
		}
		Map clues = (Map) input.getOrDefault("clues", Collections.emptyMap());
		for (Object level : clues.keySet()) {
			clues((String) level, clues.get(level));
		}
		Map<String, Object> seeds = new HashMap<>();
		seeds.put("random", 530L);
		seeds.put("shuffle", 531L);
		seeds.put("tlr", 532L);
		seeds.putAll((Map) input.getOrDefault("seeds", Collections.emptyMap()));
		seed(seeds);
	}

	static Field field(Class<?> owner, String name) throws Exception {
		Field field = owner.getDeclaredField(name);
		field.setAccessible(true);
		return field;
	}

	public static void main(String[] args) throws Exception {
		folder = args[0];
		Path directory = Paths.get(folder);
		Files.createDirectories(directory);
		pinnedConfig = new String(Files.readAllBytes(Paths.get(args[1])), StandardCharsets.UTF_8);
		pinnedShared = args[2];

		ThreadLocalRandom.current();
		threadSeed = field(Thread.class, "threadLocalRandomSeed");
		collectionsRandom = field(Collections.class, "r");
		randomState = field(Random.class, "seed");
		charmDrops = field(NPCDropTable.class, "charmDrops");
		tertiaryDrops = field(NPCDropTable.class, "tertiaryDrops");
		rareTable = field(RareDropTable.class, "TABLE");
		chanceFields.put("CELEDT", field(CELEMinorTable.class, "TABLE"));
		chanceFields.put("USDT", field(UncommonSeedDropTable.class, "TABLE"));
		chanceFields.put("HDT", field(HerbDropTable.class, "TABLE"));
		chanceFields.put("GDT", field(GemDropTable.class, "TABLE"));
		chanceFields.put("RSDT", field(RareSeedDropTable.class, "TABLE"));
		chanceFields.put("ASDT", field(AllotmentSeedDropTable.class, "TABLE"));

		PrintStream output = new PrintStream(System.out, false, "UTF-8");
		errors = new ByteArrayOutputStream();
		System.setErr(new PrintStream(errors, true, "UTF-8"));

		try (BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
			String line;
			while ((line = reader.readLine()) != null) {
				input = (Map) new JSONParser().parse(line);
				events = new ArrayList<>();
				action = Collections.emptyMap();
				reset();
				List<Object> steps = new ArrayList<>();
				for (Object entry : (List) input.get("actions")) {
					action = (Map) entry;
					events = new ArrayList<>();
					long[] before = states();
					Object value = null;
					String error = null;
					try {
						value = act(action);
					} catch (Throwable e) {
						error = category(e);
					}
					stackTraces();
					long[] after = states();
					Map<String, Object> step = new TreeMap<>();
					step.put("value", value);
					step.put("error", error);
					step.put("events", events);
					// "seed" replaces generators, so its distance is not a draw count.
					step.put("draws", "seed".equals(action.get("op")) ? null : Arrays.asList(
						steps(before[0], after[0], true), steps(before[1], after[1], true), steps(before[2], after[2], false)));
					steps.add(step);
				}
				Map<String, Object> expected = new TreeMap<>();
				expected.put("steps", steps);
				// The next value of each generator pins where the case left it.
				expected.put("probe", Arrays.asList(RandomFunction.RANDOM.nextLong(),
					((Random) collectionsRandom.get(null)).nextLong(), ThreadLocalRandom.current().nextLong()));
				input.put("expected", expected);
				output.println(JSONValue.toJSONString(input));
			}
		}
		output.flush();
	}
}
