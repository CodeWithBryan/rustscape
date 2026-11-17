package oracle;

import content.global.skill.smithing.smelting.Bar;
import core.ServerConstants;
import core.game.component.Component;
import core.game.container.Container;
import core.game.container.ContainerListener;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.GlobalData;
import core.game.node.entity.player.link.HintIconManager;
import core.game.node.entity.player.link.SkullManager;
import core.game.node.entity.player.link.WarningMessages;
import core.game.node.entity.player.link.diary.AchievementDiary;
import core.game.node.entity.player.link.diary.AchievementDiaryManager;
import core.game.node.entity.player.link.diary.DiaryType;
import core.game.node.entity.player.link.request.assist.AssistSession;
import core.game.node.item.Item;
import core.game.system.timer.RSTimer;
import core.game.system.timer.TimerManager;
import core.game.world.map.Location;
import core.net.packet.IoBuffer;
import core.net.packet.context.HintIconContext;
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
import org.json.simple.JSONArray;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/**
 * Drives the unchanged SkullManager, HintIconManager (with HintIcon and
 * HintIconContext), WarningMessages, AssistSession, AchievementDiaryManager,
 * AchievementDiary, DiaryType and the Skulled timer from one JSON case per
 * line. Every adapter records through emit; a case can make an event run
 * further actions or fail.
 */
@SuppressWarnings({"unchecked", "rawtypes"})
public class Support {
	public static Map<String, Object> input;
	public static long clock;
	static TreeMap<Integer, Entity> nodes;
	static List<Object> events;
	static Map<String, Integer> calls;
	static Set<Integer> fired;
	static IdentityHashMap<Object, String> labels;
	static Map<String, Integer> counters;
	static List<AssistSession> sessions;
	static List<RSTimer> timers;

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

	public static String dbl(double value) {
		return Double.toString(value);
	}

	public static Entity entity(Object id) {
		return id == null ? null : nodes.get(num(id));
	}

	public static Player player(Object id) {
		return (Player) entity(id);
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

	public static String itemName(int id) {
		Map names = (Map) get(input, "names", Collections.emptyMap());
		String key = Integer.toString(id);
		return names.containsKey(key) ? (String) names.get(key) : "Item " + id;
	}

	public static String npcName(int id) {
		Map names = (Map) get(input, "npcNames", Collections.emptyMap());
		String key = Integer.toString(id);
		return names.containsKey(key) ? (String) names.get(key) : "NPC " + id;
	}

	public static int alchemy(int id) {
		Map values = (Map) get(input, "alchemy", Collections.emptyMap());
		return num(values, Integer.toString(id), 0);
	}

	public static boolean registry() {
		return flag(input, "registry", true);
	}

	public static void track(RSTimer timer) {
		timers.add(timer);
	}

	static String name(Object object, String prefix) {
		String label = labels.get(object);
		if (label == null) {
			int n = counters.merge(prefix, 1, Integer::sum) - 1;
			label = prefix + n;
			labels.put(object, label);
		}
		return label;
	}

	/** A node's label: its slot, or "node" for a plain node. */
	public static Object label(Object node) {
		if (node == null) {
			return null;
		}
		if (node instanceof Entity) {
			return ((Entity) node).slot;
		}
		return "node";
	}

	public static Object value(Object v) {
		if (v == null || v instanceof Boolean || v instanceof Integer || v instanceof Long || v instanceof String) {
			return v;
		}
		if (v instanceof Double) {
			return dbl((Double) v);
		}
		if (v instanceof Byte || v instanceof Short) {
			return ((Number) v).intValue();
		}
		if (v instanceof Entity) {
			return Arrays.asList("entity", ((Entity) v).slot);
		}
		if (v instanceof AssistSession) {
			return name(v, "session");
		}
		if (v instanceof RSTimer) {
			return name(v, "timer");
		}
		if (v instanceof ContainerListener) {
			return name(v, "listener");
		}
		if (v instanceof double[]) {
			List<Object> out = new ArrayList<>();
			for (double d : (double[]) v) {
				out.add(dbl(d));
			}
			return out;
		}
		if (v instanceof boolean[]) {
			List<Object> out = new ArrayList<>();
			for (boolean d : (boolean[]) v) {
				out.add(d);
			}
			return out;
		}
		if (v instanceof DiaryType) {
			return ((DiaryType) v).name();
		}
		if (v instanceof AchievementDiary) {
			return Arrays.asList("diary", ((AchievementDiary) v).getType().name());
		}
		if (v instanceof Item[]) {
			List<Object> out = new ArrayList<>();
			for (Item item : (Item[]) v) {
				out.add(item == null ? null : Arrays.asList(item.getId(), item.getAmount()));
			}
			return out;
		}
		return v.getClass().getSimpleName();
	}

	public static void sent(Player owner, IoBuffer b) {
		byte[] a = Arrays.copyOf(b.array(), b.toByteBuffer().position());
		StringBuilder s = new StringBuilder();
		for (byte x : a) {
			s.append(String.format("%02x", x & 255));
		}
		emit("send", owner.slot, b.opcode(), s.toString());
	}

	static Field declared(Object owner, String name) {
		for (Class<?> c = owner.getClass(); c != null; c = c.getSuperclass()) {
			try {
				Field f = c.getDeclaredField(name);
				f.setAccessible(true);
				return f;
			} catch (NoSuchFieldException e) {
				// keep looking in the superclass
			}
		}
		throw new IllegalStateException(name);
	}

	static Object field(Object owner, String name) {
		try {
			return declared(owner, name).get(owner);
		} catch (ReflectiveOperationException e) {
			throw new IllegalStateException(e);
		}
	}

	static void setField(Object owner, String name, Object value) {
		try {
			declared(owner, name).set(owner, value);
		} catch (ReflectiveOperationException e) {
			throw new IllegalStateException(e);
		}
	}

	static Node target(Object value) {
		if (value == null) {
			return null;
		}
		Map map = (Map) value;
		if (map.containsKey("node")) {
			return entity(map.get("node"));
		}
		return new Node(loc(map.get("location")));
	}

	static List<Object> icon(HintIconContext c) {
		if (c == null) {
			return null;
		}
		Location l = c.getLocation();
		return Arrays.asList(label(c.getPlayer()), c.getSlot(), c.getTargetType(), c.getArrowId(), c.getIndex(), c.getModelId(),
			l == null ? null : Arrays.asList(l.getX(), l.getY(), l.getZ()), c.getHeight());
	}

	static DiaryType type(Object name) {
		return name == null ? null : DiaryType.valueOf((String) name);
	}

	static AssistSession session(Object label) {
		return label == null ? null : sessions.get(num(label));
	}

	static Item item(Object spec) {
		List l = (List) spec;
		return new Item(num(l.get(0)), num(l.get(1)));
	}

	static List<Object> strings(String[] array) {
		return array == null ? null : new ArrayList<>(Arrays.asList(array));
	}

	static Object table() {
		List<Object> out = new ArrayList<>();
		for (DiaryType t : DiaryType.values()) {
			List<Object> achievements = new ArrayList<>();
			for (String[] level : t.getAchievements()) {
				achievements.add(strings(level));
			}
			List<Object> rewards = new ArrayList<>();
			for (Item[] level : t.getRewards()) {
				rewards.add(value(level));
			}
			List<Object> npcs = new ArrayList<>();
			for (int npc : t.getNpcs()) {
				npcs.add(npc);
			}
			out.add(Arrays.asList(t.name(), t.getName(), t.getChild(), strings(t.getLevelNames()), achievements, rewards, t.getInfo(), npcs));
		}
		return out;
	}

	static ContainerListener deepListener(Player p) {
		return (ContainerListener) p.attributes.get("deepwild-value-listener");
	}

	static Container container(Player p, Object kind) {
		switch ((String) kind) {
		case "inventory": return p.getInventory();
		case "equipment": return p.getEquipment();
		default: return p.getBank();
		}
	}

	static Object skull(Player p, Map a, String op) {
		SkullManager s = p.getSkullManager();
		switch (op) {
		case "checkSkull": s.checkSkull(entity(a.get("other"))); return null;
		case "setSkullIcon": s.setSkullIcon(num(a.get("icon"))); return null;
		case "reset": s.reset(); return null;
		case "getPlayer": return value(s.getPlayer());
		case "getLevel": return s.getLevel();
		case "setLevel": s.setLevel(num(a.get("level"))); return null;
		case "isWilderness": return s.isWilderness();
		case "setWilderness": s.setWilderness(flag(a, "value", false)); return null;
		case "isSkullCheckDisabled": return s.isSkullCheckDisabled();
		case "setSkullCheckDisabled": s.setSkullCheckDisabled(flag(a, "value", false)); return null;
		case "isWildernessDisabled": return s.isWildernessDisabled();
		case "setWildernessDisabled": s.setWildernessDisabled(flag(a, "value", false)); return null;
		case "hasWildernessProtection": return s.hasWildernessProtection();
		case "isSkulled": return s.isSkulled();
		case "setSkulled": s.setSkulled(flag(a, "value", false)); return null;
		case "isDeepWilderness": return s.isDeepWilderness();
		case "setDeepWilderness": s.setDeepWilderness(flag(a, "value", false)); return null;
		case "updateDWSkullIcon": s.updateDWSkullIcon(); return null;
		case "removeDWSkullIcon": s.removeDWSkullIcon(); return null;
		case "listenerUpdate": deepListener(p).update(container(p, a.get("container")), null); return null;
		case "listenerRefresh": deepListener(p).refresh(container(p, a.get("container"))); return null;
		default: throw new IllegalArgumentException(op);
		}
	}

	static Object hint(Player p, Map a, String op) {
		Node t = target(a.get("target"));
		int arrow = num(a, "arrow", HintIconManager.DEFAULT_ARROW);
		int model = num(a, "model", HintIconManager.DEFAULT_MODEL);
		switch (op) {
		case "register": return HintIconManager.registerHintIcon(p, t);
		case "registerHeight": return HintIconManager.registerHeightHintIcon(p, t, num(a.get("height")));
		case "registerArrow": return HintIconManager.registerHintIcon(p, t, arrow);
		case "registerModel": return HintIconManager.registerHintIcon(p, t, arrow, model);
		case "registerSlot": return HintIconManager.registerHintIcon(p, t, arrow, model, num(a.get("slot")));
		case "registerSlotHeight": return HintIconManager.registerHintIcon(p, t, arrow, model, num(a.get("slot")), num(a.get("height")));
		case "registerFull":
			return HintIconManager.registerHintIcon(p, t, arrow, model, num(a.get("slot")), num(a.get("height")), num(a.get("type")));
		case "remove": HintIconManager.removeHintIcon(p, num(a.get("slot"))); return null;
		case "clear": p.getHintIconManager().clear(); return null;
		case "freeSlot": return p.getHintIconManager().freeSlot();
		case "getIcon": return icon(p.getHintIconManager().getIcon(num(a.get("slot"))));
		case "setTargetType": p.getHintIconManager().getIcon(num(a.get("slot"))).setTargetType(num(a.get("value"))); return null;
		case "setArrowId": p.getHintIconManager().getIcon(num(a.get("slot"))).setArrowId(num(a.get("value"))); return null;
		case "setHeight": p.getHintIconManager().getIcon(num(a.get("slot"))).setHeight(num(a.get("value"))); return null;
		case "constants":
			return Arrays.asList(HintIconManager.MAXIMUM_SIZE, HintIconManager.DEFAULT_ARROW, HintIconManager.DEFAULT_MODEL,
				HintIconManager.ARROW_CIRCLE_MODEL);
		default: throw new IllegalArgumentException(op);
		}
	}

	static Object warn(Player p, Map a, String op) {
		WarningMessages w = p.getWarningMessages();
		switch (op) {
		case "open": w.open(p); return null;
		case "getMessage": {
			WarningMessages.WarningMessage m = w.getMessage(num(a.get("index")));
			return Arrays.asList(m.getIndex(), ((List) field(w, "messages")).indexOf(m));
		}
		case "toggle": w.getMessage(num(a.get("index"))).toggle(player(a.get("other"))); return null;
		default: throw new IllegalArgumentException(op);
		}
	}

	static Object assist(Map a, String op) {
		AssistSession s = session(a.get("session"));
		Player p = player(a.get("node"));
		switch (op) {
		case "new": {
			AssistSession made = new AssistSession(p, player(a.get("partner")));
			sessions.add(made);
			return value(made);
		}
		case "newEmpty": {
			AssistSession made = new AssistSession();
			sessions.add(made);
			return value(made);
		}
		case "module": {
			AssistSession made = new AssistSession();
			sessions.add(made);
			made.open(p, player(a.get("target")));
			return value(made);
		}
		case "extend": {
			AssistSession.extend(p, player(a.get("partner")));
			Object made = p.extensions.get(AssistSession.class);
			if (made != null && !sessions.contains(made)) {
				sessions.add((AssistSession) made);
			}
			return null;
		}
		case "getExtension": {
			AssistSession found = AssistSession.getExtension(p);
			if (found != null && !sessions.contains(found)) {
				sessions.add(found);
			}
			return value(found);
		}
		case "open": s.open(); return null;
		case "close": return p.getInterfaceManager().close();
		case "closeEvent": return s.getCloseEvent().close(p, new Component(301));
		case "toggleIcon": s.toggleIcon(p, flag(a, "hide", false)); return null;
		case "addExperience": s.addExperience(num(a.get("skill")), ((Number) a.get("exp")).doubleValue()); return null;
		case "refresh": s.refresh(); return null;
		case "toggleButton": s.toggleButton((byte) num(a.get("id"))); return null;
		case "isToggled": return s.isToggled(num(a.get("skill")));
		case "getSkillIndex": return s.getSkillIndex(num(a.get("skill")));
		case "getSkills": return value(s.getSkills());
		case "getTimeLeft": return s.getTimeLeft();
		case "getTotalExperience": return dbl(s.getTotalExperience());
		case "getPlayer": return value(s.getPlayer());
		case "load": s.load(); return null;
		case "save": s.save(); return null;
		case "reset": s.reset(); return null;
		case "pulse": return s.pulse();
		case "translate":
			return s.translateExperience(p, num(a.get("slot")), ((Number) a.get("exp")).doubleValue(), ((Number) a.get("mod")).doubleValue());
		case "isRestricted": return s.isRestricted();
		case "getExp": return value(s.getExp());
		case "setExp": {
			List values = (List) a.get("values");
			double[] exp = new double[values.size()];
			for (int i = 0; i < exp.length; i++) {
				exp[i] = ((Number) values.get(i)).doubleValue();
			}
			s.setExp(exp);
			return null;
		}
		case "getPartener": return value(s.getPartener());
		case "setPartener": s.setPartener(p); return null;
		case "setGlobal": {
			GlobalData g = p.getSavedData().raw();
			if (a.containsKey("time")) {
				g.setAssistTime(((Number) a.get("time")).longValue());
			}
			if (a.containsKey("values")) {
				List values = (List) a.get("values");
				double[] exp = new double[values.size()];
				for (int i = 0; i < exp.length; i++) {
					exp[i] = ((Number) values.get(i)).doubleValue();
				}
				g.setAssistExperience(exp);
			}
			return null;
		}
		default: throw new IllegalArgumentException(op);
		}
	}

	static Object diary(Player p, Map a, String op) {
		AchievementDiaryManager m = p.getAchievementDiaryManager();
		DiaryType t = type(a.get("type"));
		int level = num(a, "level", 0);
		int index = num(a, "index", 0);
		switch (op) {
		case "parse": m.parse((JSONArray) a.get("data")); return null;
		case "openTab": m.openTab(); return null;
		case "updateTask": m.updateTask(player(get(a, "actor", a.get("node"))), t, level, index, flag(a, "complete", false)); return null;
		case "finishTask": m.finishTask(player(get(a, "actor", a.get("node"))), t, level, index); return null;
		case "hasCompletedTask": return m.hasCompletedTask(t, level, index);
		case "setStarted": m.setStarted(t, level); return null;
		case "setCompleted": m.setCompleted(t, level, index); return null;
		case "getDiary": return value(m.getDiary(t));
		case "getKaramjaGlove": return m.getKaramjaGlove();
		case "getArmour": return m.getArmour();
		case "checkMiningReward": return m.checkMiningReward(num(a.get("reward")));
		case "checkSmithReward": return m.checkSmithReward(a.get("bar") == null ? null : Bar.valueOf((String) a.get("bar")));
		case "hasGlove": return m.hasGlove();
		case "hasArmour": return m.hasArmour();
		case "isComplete": return m.isComplete(t);
		case "getPlayer": return value(m.getPlayer());
		case "getDiarys": {
			List<Object> out = new ArrayList<>();
			for (AchievementDiary d : m.getDiarys()) {
				out.add(value(d));
			}
			return out;
		}
		case "resetRewards": m.resetRewards(); return null;
		default: throw new IllegalArgumentException(op);
		}
	}

	static Object achievement(Player p, Map a, String op) {
		DiaryType t = type(a.get("type"));
		AchievementDiary d = p.getAchievementDiaryManager().getDiary(t);
		int level = num(a, "level", 0);
		int index = num(a, "index", 0);
		Player actor = player(get(a, "actor", a.get("node")));
		switch (op) {
		case "open": d.open(actor); return null;
		case "drawStatus": d.drawStatus(actor); return null;
		case "updateTask": d.updateTask(actor, level, index, flag(a, "complete", false)); return null;
		case "finishTask": d.finishTask(actor, level, index); return null;
		case "resetTask": d.resetTask(actor, level, index); return null;
		case "setLevelStarted": d.setLevelStarted(level); return null;
		case "setCompleted": d.setCompleted(level, index); return null;
		case "isStartedLevel": return d.isStarted(level);
		case "isStarted": return d.isStarted();
		case "isCompleteTask": return d.isComplete(level, index);
		case "isCompleteLevel": return d.isComplete(level);
		case "isCompleteCumulative": return d.isComplete(level, flag(a, "cumulative", false));
		case "isComplete": return d.isComplete();
		case "getLevel": return d.getLevel();
		case "getReward": return d.getReward();
		case "getLevelName": return d.getLevel(level);
		case "getStatus": return d.getStatus(level);
		case "setLevelRewarded": d.setLevelRewarded(level); return null;
		case "isLevelRewarded": return d.isLevelRewarded(level);
		case "getType": return value(d.getType());
		case "removeRewardsFor": return AchievementDiary.removeRewardsFor(p, t, level);
		case "addRewardsFor": return AchievementDiary.addRewardsFor(p, t, level);
		case "flagRewarded": return AchievementDiary.flagRewarded(p, t, level);
		case "canReplaceReward": return AchievementDiary.canReplaceReward(p, t, level);
		case "grantReplacement": return AchievementDiary.grantReplacement(p, t, level);
		case "hasCompletedLevel": return AchievementDiary.hasCompletedLevel(p, t, level);
		case "hasClaimedLevelRewards": return AchievementDiary.hasClaimedLevelRewards(p, t, level);
		case "canClaimLevelRewards": return AchievementDiary.canClaimLevelRewards(p, t, level);
		case "getRewards": return value(AchievementDiary.getRewards(t, level));
		default: throw new IllegalArgumentException(op);
		}
	}

	public static Object act(Map a) {
		String op = (String) a.get("op");
		Player p = a.get("node") instanceof Number && entity(a.get("node")) instanceof Player ? player(a.get("node")) : null;
		int dot = op.indexOf('.');
		String group = dot < 0 ? op : op.substring(0, dot);
		String member = dot < 0 ? "" : op.substring(dot + 1);
		switch (group) {
		case "skull": return skull(player(a.get("node")), a, member);
		case "hint": return hint(player(a.get("node")), a, member);
		case "warn": return warn(player(a.get("node")), a, member);
		case "assist": return assist(a, member);
		case "diary": return diary(player(a.get("node")), a, member);
		case "ad": return achievement(player(a.get("node")), a, member);
		case "setAttribute": entity(a.get("node")).attributes.put((String) a.get("key"), attribute(a.get("value"))); return null;
		case "clearCalls": calls.clear(); return null;
		case "clock": clock = ((Number) a.get("now")).longValue(); return null;
		case "deep": ServerConstants.ENHANCED_DEEP_WILDERNESS = flag(a, "value", false); return null;
		case "active": entity(a.get("node")).active = flag(a, "value", true); return null;
		case "move": setField(entity(a.get("node")), "location", loc(a.get("location"))); return null;
		case "invSet": {
			Item[] items = container(player(a.get("node")), a.get("container")).raw();
			items[num(a.get("slot"))] = a.get("item") == null ? null : item(a.get("item"));
			return null;
		}
		case "timerRun": return timers.get(num(a.get("timer"))).run(entity(a.get("node")));
		case "timerRemove": entity(a.get("node")).getTimers().removeTimer((String) a.get("identifier")); return null;
		case "timerProcess": entity(a.get("node")).getTimers().processTimers(); return null;
		case "skullIcons": {
			List<Object> out = new ArrayList<>();
			for (SkullManager.SkullIcon icon : SkullManager.SkullIcon.values()) {
				out.add(Arrays.asList(icon.name(), icon.id));
			}
			return out;
		}
		case "skullIconForId": return SkullManager.SkullIcon.forId(num(a.get("id"))).name();
		case "threshold": return SkullManager.DEEP_WILD_DROP_RISK_THRESHOLD;
		case "diaryTable": return table();
		case "forChild": return value(DiaryType.forChild(num(a.get("child"))));
		case "hasRewardEquipment": return type(a.get("type")).hasRewardEquipment(player(a.get("node")));
		case "diaryRewards": return value(type(a.get("type")).getRewards(num(a.get("level"))));
		case "diaryNpc": return type(a.get("type")).getNpc(num(a.get("level")));
		case "diaryAchievements": return strings(type(a.get("type")).getAchievements(num(a.get("level"))));
		case "completedLevels": return new ArrayList<>(AchievementDiary.completedLevels);
		case "barOrdinal": return Bar.valueOf((String) a.get("bar")).ordinal();
		default: throw new IllegalArgumentException(op);
		}
	}

	static Object attribute(Object spec) {
		if (spec instanceof Map) {
			Map m = (Map) spec;
			if (m.containsKey("long")) {
				return ((Number) m.get("long")).longValue();
			}
			if (m.containsKey("listener")) {
				return new ContainerListener() {
					@Override
					public void update(Container c, core.game.container.ContainerEvent event) {
						emit("foreignUpdate");
					}

					@Override
					public void refresh(Container c) {
						emit("foreignRefresh");
					}
				};
			}
		}
		if (spec instanceof Long) {
			return ((Long) spec).intValue();
		}
		return spec;
	}

	static String error(Throwable e) {
		while (e.getCause() != null) {
			e = e.getCause();
		}
		if (e instanceof Failure) return "Failure";
		if (e instanceof NullPointerException) return "Null";
		if (e instanceof ClassCastException) return "ClassCast";
		if (e instanceof ArrayIndexOutOfBoundsException) return "Index";
		if (e instanceof IndexOutOfBoundsException) return "Index";
		if (e instanceof ConcurrentModificationException) return "ConcurrentModification";
		if (e instanceof IllegalArgumentException) return "IllegalArgument";
		return e.getClass().getSimpleName();
	}

	static List<Object> timerList(List<RSTimer> list) {
		List<Object> out = new ArrayList<>();
		for (RSTimer t : list) {
			out.add(Arrays.asList(value(t), t.getIdentifier(), t.getRunInterval(), t.getNextExecution()));
		}
		return out;
	}

	static List<Object> items(Container c) {
		List<Object> out = new ArrayList<>();
		Item[] raw = c.raw();
		for (int i = 0; i < raw.length; i++) {
			if (raw[i] != null) {
				out.add(Arrays.asList(i, raw[i].getId(), raw[i].getAmount()));
			}
		}
		return out;
	}

	static Object describe(Player p) {
		Map row = new LinkedHashMap();
		SkullManager s = p.getSkullManager();
		List<Object> causes = new ArrayList<>();
		for (Player c : (List<Player>) field(s, "skullCauses")) {
			causes.add(label(c));
		}
		row.put("skull", Arrays.asList(s.isWilderness(), s.isWildernessDisabled(), s.getLevel(), causes, field(s, "skulled"), s.isSkullCheckDisabled(),
			s.isDeepWilderness()));
		List<Object> hints = new ArrayList<>();
		for (int i = 0; i < HintIconManager.MAXIMUM_SIZE; i++) {
			hints.add(icon(p.getHintIconManager().getIcon(i)));
		}
		row.put("hints", hints);
		Map attributes = new TreeMap();
		for (Map.Entry<String, Object> e : p.attributes.entrySet()) {
			attributes.put(e.getKey(), value(e.getValue()));
		}
		row.put("attributes", attributes);
		Map extensions = new TreeMap();
		for (Map.Entry<Class, Object> e : p.extensions.entrySet()) {
			extensions.put(e.getKey().getSimpleName(), value(e.getValue()));
		}
		row.put("extensions", extensions);
		Map containers = new LinkedHashMap();
		for (Container c : Arrays.asList(p.getInventory(), p.getEquipment(), p.getBank())) {
			List<Object> listeners = new ArrayList<>();
			for (ContainerListener l : c.rawListeners()) {
				listeners.add(value(l));
			}
			containers.put(c.kind(), Arrays.asList(items(c), listeners));
		}
		row.put("containers", containers);
		Component opened = p.getInterfaceManager().getOpened();
		row.put("opened", opened == null ? null : Arrays.asList(opened.getId(), opened.getCloseEvent() != null));
		GlobalData g = p.getSavedData().raw();
		Object alias = null;
		for (AssistSession session : sessions) {
			if (session.getExp() == g.getAssistExperience()) {
				alias = value(session);
				break;
			}
		}
		row.put("global", Arrays.asList(g.getAssistTime(), value(g.getAssistExperience()), alias));
		List<Object> diaries = new ArrayList<>();
		for (AchievementDiary d : p.getAchievementDiaryManager().getDiarys()) {
			List<Object> completed = new ArrayList<>();
			for (boolean[] level : d.getTaskCompleted()) {
				List<Object> done = new ArrayList<>();
				for (int i = 0; i < level.length; i++) {
					if (level[i]) {
						done.add(i);
					}
				}
				completed.add(done);
			}
			diaries.add(Arrays.asList(d.getType().name(), value(d.getLevelStarted()), value(d.getLevelRewarded()), completed));
		}
		row.put("diaries", diaries);
		List<Object> warnings = new ArrayList<>();
		for (Object m : (List) field(p.getWarningMessages(), "messages")) {
			warnings.add(((WarningMessages.WarningMessage) m).getIndex());
		}
		row.put("warnings", warnings);
		TimerManager tm = p.getTimers();
		row.put("timers", Arrays.asList(timerList(tm.getActiveTimers()), timerList(tm.getNewTimers()), timerList(tm.getToRemoveTimers())));
		return row;
	}

	public static Object evaluate(Map data) {
		input = data;
		clock = ((Number) get(data, "clock", 1000000000000L)).longValue();
		nodes = new TreeMap<>();
		events = new ArrayList<>();
		calls = new HashMap<>();
		fired = new HashSet<>();
		labels = new IdentityHashMap<>();
		counters = new HashMap<>();
		sessions = new ArrayList<>();
		timers = new ArrayList<>();
		AchievementDiary.completedLevels.clear();
		ServerConstants.ENHANCED_DEEP_WILDERNESS = flag(data, "enhanced", false);
		for (Object value : (List) get(data, "nodes", Collections.emptyList())) {
			Map spec = new HashMap((Map) value);
			int id = num(spec.get("id"));
			String kind = (String) get(spec, "kind", "player");
			nodes.put(id, kind.equals("player") ? new Player(id, spec) : new NPC(id, spec));
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
		Map players = new TreeMap();
		for (Entity n : nodes.values()) {
			if (n instanceof Player) {
				players.put("" + n.slot, describe((Player) n));
			}
		}
		out.put("players", players);
		List<Object> described = new ArrayList<>();
		for (AssistSession s : sessions) {
			Component component = (Component) field(s, "component");
			described.add(Arrays.asList(label(field(s, "player")), label(field(s, "partener")), value(field(s, "skills")), field(s, "restricted"),
				value(field(s, "exp")), field(s, "kill"), field(s, "time"), component.getId(), component.getCloseEvent() != null));
		}
		out.put("sessions", described);
		out.put("completedLevels", new ArrayList<>(AchievementDiary.completedLevels));
		List<Object> spawned = new ArrayList<>();
		for (RSTimer t : timers) {
			spawned.add(Arrays.asList(value(t), t.getIdentifier(), t.getRunInterval()));
		}
		out.put("timers", spawned);
		out.put("enhanced", ServerConstants.ENHANCED_DEEP_WILDERNESS);
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
