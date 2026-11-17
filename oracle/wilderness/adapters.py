"""Adapters for the wilderness and dark zone oracle.

The zones oracle's adapters (oracle/zones/adapters.py) are the base: Region,
RegionManager, RegionChunk, the cache, Location, the pulse runner's world,
the packet classes and the rest are taken from there unchanged. The entity
classes are replaced here by recording adapters with the members
WildernessZone, DarkZone, LightSource and BrawlingGloves call, and the two
placeholder zone classes of that oracle are dropped: the real WildernessZone
and DarkZone are compiled instead. Every adapter records through
oracle.Support.emit and answers from the case. `/* ORIGINAL_* */` markers are
replaced by text taken from the pinned sources when the oracle is generated.
"""
import importlib.util
import pathlib

HERE = pathlib.Path(__file__).resolve().parent


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = HERE.parent / 'zones' / 'adapters.py'
ADAPTERS = dict(_load('zones_base_adapters', BASE).ADAPTERS)
for placeholder in ['core/game/world/map/zone/impl/WildernessZone.java', 'core/game/world/map/zone/impl/DarkZone.java']:
    del ADAPTERS[placeholder]


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/game/node/Node.java', '''
package core.game.node;

import core.game.node.entity.npc.NPC;
import core.game.node.item.Item;
import core.game.world.map.Location;

/** The members of Node that the zone classes call. */
public class Node {
	public Location getLocation() {
		return null;
	}

	public Item asItem() {
		return (Item) this;
	}

	public NPC asNpc() {
		return (NPC) this;
	}

	public boolean isActive() {
		return true;
	}

	public int size() {
		return 1;
	}
}
''')

adapter('core/game/node/item/Item.java', '''
package core.game.node.item;

import core.game.node.Node;
import oracle.Support;

/** An item id and amount; its name is a service of the case. */
public class Item extends Node {
	private final int id;
	private final int amount;

	public Item(int id) {
		this(id, 1);
	}

	public Item(int id, int amount) {
		this.id = id;
		this.amount = amount;
	}

	public int getId() {
		return id;
	}

	public int getAmount() {
		return amount;
	}

	public String getName() {
		String name = Support.itemName(id);
		Support.emit("itemGetName", id);
		return name;
	}
}
''')

adapter('core/cache/def/impl/ItemDefinition.java', '''
package core.cache.def.impl;

/** Only BrawlingGloves' static initializer reaches it; nothing is recorded. */
public class ItemDefinition {
	private final int id;

	private ItemDefinition(int id) {
		this.id = id;
	}

	public static ItemDefinition forId(int id) {
		return new ItemDefinition(id);
	}

	public String getName() {
		return "definition " + id;
	}
}
''')

adapter('core/game/node/entity/skill/Skills.java', '''
package core.game.node.entity.skill;

/** The skill slot constants, original text. */
public class Skills {
	/* ORIGINAL_SKILL_IDS */
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.event.EventHook;
import core.game.node.Node;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneMonitor;
import core.game.world.map.zone.ZoneRestriction;
import java.util.HashMap;
import java.util.Map;
import oracle.Support;

/**
 * Recording entity. Its state is the case's attribute map; the monitor is a
 * real ZoneMonitor. isTeleBlocked is the original text.
 */
public class Entity extends Node {
	private final int slot;
	private final Map<String, Object> state;
	private final ZoneMonitor monitor = new ZoneMonitor(this);
	private final Map<Class<?>, Object> extensions = new HashMap<>();
	public Timers timers = new Timers();

	public Entity(int slot, Map<String, Object> state) {
		this.slot = slot;
		this.state = state;
	}

	public int slot() {
		return slot;
	}

	public Map<String, Object> state() {
		return state;
	}

	public class Timers {
		public Object getTimer(String name) {
			boolean present = Support.flag(state, "teleTimer", false);
			Support.emit("timer", slot, name);
			return present ? new Object() : null;
		}
	}

	public class Locks {
		public boolean isTeleportLocked() {
			boolean locked = Support.flag(state, "teleportLocked", false);
			Support.emit("teleportLocked", slot);
			return locked;
		}
	}

	public class CombatPulse {
		public void attack(Node victim) {
			Support.emit("attack", slot, Support.targetLabel(victim));
		}
	}

	public class Properties {
		public boolean isMultiZone() {
			boolean multi = Support.flag(state, "multiZone", false);
			Support.emit("multiZone", slot);
			return multi;
		}

		public void setMultiZone(boolean multi) {
			state.put("multiZone", multi);
			Support.emit("setMultiZone", slot, multi);
		}

		public boolean isNPCWalkable() {
			boolean walkable = Support.flag(state, "npcWalkable", false);
			Support.emit("npcWalkable", slot);
			return walkable;
		}

		public int getCurrentCombatLevel() {
			int level = Support.num(state, "combat", 3);
			Support.emit("combatLevel", slot);
			return level;
		}

		public CombatPulse getCombatPulse() {
			return new CombatPulse();
		}
	}

	@Override
	public Location getLocation() {
		Location location = Support.loc(state.get("location"));
		Support.emit("location", slot);
		return location;
	}

	@Override
	public boolean isActive() {
		boolean active = Support.flag(state, "active", true);
		Support.emit("active", slot);
		return active;
	}

	@Override
	public int size() {
		int size = Support.num(state, "size", 1);
		Support.emit("size", slot);
		return size;
	}

	public int getId() {
		int id = Support.num(state, "npcId", 0);
		Support.emit("id", slot);
		return id;
	}

	public String getUsername() {
		String name = state.containsKey("username") ? (String) state.get("username") : "user" + slot;
		Support.emit("username", slot);
		return name;
	}

	public ZoneMonitor getZoneMonitor() {
		return monitor;
	}

	public Locks getLocks() {
		return new Locks();
	}

	public Properties getProperties() {
		return new Properties();
	}

	public ImpactHandler getImpactHandler() {
		return new ImpactHandler(this);
	}

	public Player asPlayer() {
		return (Player) this;
	}

	public boolean isPlayer() {
		return this instanceof Player;
	}

	public void updateLocation(Location last) {
		Support.emit("entityUpdate", slot, Support.coord(last));
	}

	public boolean continueAttack(Entity target, CombatStyle style, boolean message) {
		boolean answer = Support.flag(state, "continueAttack", true);
		Support.emit("entityContinueAttack", slot, target.slot(), style == null ? null : style.name(), message);
		return answer;
	}

	public boolean isIgnoreMultiBoundaries(Entity victim) {
		boolean answer = Support.flag(state, "ignoreMulti", false);
		Support.emit("entityIgnoreMulti", slot, victim.slot());
		return answer;
	}

	public void hook(Class<?> event, EventHook hook) {
		Support.emit("hook", slot, event.getSimpleName(), Support.zoneLabel(hook));
	}

	public void unhook(EventHook hook) {
		Support.emit("unhook", slot, Support.zoneLabel(hook));
	}

	@SuppressWarnings("unchecked")
	public <T> T getExtension(Class<?> clazz) {
		Object extension = extensions.get(clazz);
		Support.emit("getExtension", slot, clazz.getSimpleName());
		return (T) extension;
	}

	public void addExtension(Class<?> clazz, Object object) {
		extensions.put(clazz, object);
		Support.emit("addExtension", slot, clazz.getSimpleName(), Support.pulseLabel(object));
	}

	public Object extension(Class<?> clazz) {
		return extensions.get(clazz);
	}

	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key, T fail) {
		Object stored = null;
		if (key.equals("combat-attacker") && state.get("attacker") != null) {
			stored = Support.entity(state.get("attacker"));
		} else if (key.equals("combat-time") && state.get("combatTime") != null) {
			stored = ((Number) state.get("combatTime")).longValue();
		} else if (key.equals("deepwild-value-risk") && state.get("risk") != null) {
			stored = ((Number) state.get("risk")).longValue();
		}
		Support.emit("attribute", slot, key);
		return stored == null ? fail : (T) stored;
	}

	/* ORIGINAL_IS_TELE_BLOCKED */
}
''')

adapter('core/game/node/entity/combat/ImpactHandler.java', '''
package core.game.node.entity.combat;

import core.game.node.entity.Entity;
import oracle.Support;

/** Recording impact handler: only manualHit is reached. */
public class ImpactHandler {
	private final Entity entity;

	public enum HitsplatType {
		NORMAL
	}

	public ImpactHandler(Entity entity) {
		this.entity = entity;
	}

	public void manualHit(Entity source, int hit, HitsplatType type) {
		Support.emit("manualHit", entity.slot(), source == null ? null : source.slot(), hit, type.name());
	}
}
''')

adapter('core/game/node/entity/player/link/SkullManager.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;
import oracle.Support;

/** Recording skull manager over the player's state; the constant is original text. */
public final class SkullManager {
	/* ORIGINAL_DEEP_WILD_THRESHOLD */

	private final Player player;

	public SkullManager(Player player) {
		this.player = player;
	}

	public boolean isWilderness() {
		boolean wilderness = Support.flag(player.state(), "wilderness", false);
		Support.emit("wilderness", player.slot());
		return wilderness;
	}

	public void setWilderness(boolean wilderness) {
		player.state().put("wilderness", wilderness);
		Support.emit("setWilderness", player.slot(), wilderness);
	}

	public int getLevel() {
		int level = Support.num(player.state(), "skullLevel", 0);
		Support.emit("skullLevel", player.slot());
		return level;
	}

	public void setLevel(int level) {
		player.state().put("skullLevel", (long) level);
		Support.emit("setSkullLevel", player.slot(), level);
	}

	public boolean isWildernessDisabled() {
		boolean disabled = Support.flag(player.state(), "wildDisabled", false);
		Support.emit("wildernessDisabled", player.slot());
		return disabled;
	}

	public boolean isDeepWilderness() {
		boolean deep = Support.flag(player.state(), "deep", false);
		Support.emit("deepWilderness", player.slot());
		return deep;
	}
}
''')

adapter('core/game/component/Component.java', '''
package core.game.component;

import core.game.node.entity.player.Player;
import oracle.Support;

/** Recording component: open and close are the base members DarkZone's overlay calls through super. */
public class Component {
	protected final int id;

	public Component(int id) {
		this.id = id;
	}

	public int getId() {
		return id;
	}

	public void open(Player player) {
		Support.emit("componentOpen", player.slot(), Support.componentLabel(this));
	}

	public boolean close(Player player) {
		Support.emit("componentClose", player.slot(), Support.componentLabel(this));
		Boolean answer = Support.answer("componentClose");
		return answer == null ? true : answer;
	}
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import content.global.skill.summoning.familiar.Familiar;
import core.game.component.Component;
import core.game.interaction.Option;
import core.game.node.entity.Entity;
import core.game.node.entity.player.info.Rights;
import core.game.node.entity.player.link.SkullManager;
import core.game.node.entity.player.link.TeleportManager;
import core.game.node.item.Item;
import core.game.world.map.Location;
import java.util.List;
import java.util.Map;
import oracle.Support;

/**
 * Recording player: every sub-object is a service of the case. The
 * interface manager keeps one overlay field; its openOverlay, closeOverlay
 * and close(Component) call the components' own close/open in the order of
 * the original manager (its definition-type repair and packets are outside
 * this oracle).
 */
public class Player extends Entity {
	private Component overlay;

	public Player(int slot, Map<String, Object> state) {
		super(slot, state);
	}

	public boolean isArtificial() {
		boolean artificial = Support.flag(state(), "artificial", false);
		Support.emit("artificial", slot());
		return artificial;
	}

	public void sendMessage(String message) {
		Support.emit("playerMessage", slot(), message);
	}

	public String getName() {
		return "p" + slot();
	}

	public Component overlay() {
		return overlay;
	}

	public class Dispatch {
		public void sendMessage(String message) {
			Support.emit("packetMessage", slot(), message);
		}

		public void sendInterfaceConfig(int interfaceId, int child, boolean hide) {
			Support.emit("interfaceConfig", slot(), interfaceId, child, hide);
		}

		public void sendString(String text, int interfaceId, int child) {
			Support.emit("sendString", slot(), text, interfaceId, child);
		}
	}

	public class Music {
		public boolean isPlaying() {
			boolean playing = Support.flag(state(), "playing", false);
			Support.emit("musicPlaying", slot());
			return playing;
		}

		public void playDefault() {
			Support.emit("playDefault", slot());
		}

		public void unlock(int id) {
			Support.emit("unlock", slot(), id, null);
		}

		public void unlock(int id, boolean play) {
			Support.emit("unlock", slot(), id, play);
		}
	}

	public class Box {
		private final String which;

		Box(String which) {
			this.which = which;
		}

		public int getAmount(Item item) {
			int amount = Support.num(state(), "rum", 0);
			Support.emit("inventoryAmount", slot(), item.getId());
			return amount;
		}

		public boolean remove(Item item) {
			Support.emit("inventoryRemove", slot(), item.getId(), item.getAmount());
			return true;
		}

		public Item[] toArray() {
			List ids = (List) Support.get(state(), which, java.util.Collections.emptyList());
			Item[] items = new Item[ids.size()];
			for (int i = 0; i < items.length; i++) {
				items[i] = ids.get(i) == null ? null : new Item(Support.num(ids.get(i)));
			}
			Support.emit("toArray", slot(), which);
			return items;
		}

		public boolean containsItem(Item item) {
			throw new UnsupportedOperationException("cleanItems is outside this oracle");
		}
	}

	public class Details {
		public Rights getRights() {
			Rights rights = Rights.valueOf((String) Support.get(state(), "rights", "REGULAR_PLAYER"));
			Support.emit("rights", slot());
			return rights;
		}
	}

	public class Interfaces {
		public boolean isResizable() {
			boolean resizable = Support.flag(state(), "resizable", false);
			Support.emit("resizable", slot());
			return resizable;
		}

		public int getWindowPaneId() {
			int pane = Support.num(state(), "windowPane", 548);
			Support.emit("windowPane", slot());
			return pane;
		}

		public void restoreTabs() {
			Support.emit("restoreTabs", slot());
		}

		public void openOverlay(Component component) {
			Support.emit("openOverlay", slot(), Support.componentLabel(component));
			if (overlay != null && !overlay.close(Player.this)) {
				return;
			}
			overlay = component;
			overlay.open(Player.this);
		}

		public void closeOverlay() {
			Support.emit("closeOverlay", slot());
			if (overlay != null && close(overlay)) {
				overlay = null;
			}
		}

		public boolean close(Component component) {
			Support.emit("closeComponent", slot(), Support.componentLabel(component));
			return component.close(Player.this);
		}

		public Component getOverlay() {
			Support.emit("getOverlay", slot());
			return overlay;
		}
	}

	public class Teleporter {
		public boolean send(Location location, TeleportManager.TeleportType type) {
			Support.emit("teleporterSend", slot(), Support.coord(location), type.name());
			return true;
		}
	}

	public class Familiars {
		public boolean hasFamiliar() {
			boolean has = Support.get(state(), "familiar", null) != null || Support.flag(state(), "hasFamiliar", false);
			Support.emit("hasFamiliar", slot());
			return has;
		}

		public boolean hasPet() {
			boolean pet = Support.flag(state(), "pet", false);
			Support.emit("hasPet", slot());
			return pet;
		}

		public Familiar getFamiliar() {
			Familiar familiar = (Familiar) Support.entity(state().get("familiar"));
			Support.emit("getFamiliar", slot());
			return familiar;
		}
	}

	public class Appearance {
		public void sync() {
			Support.emit("sync", slot());
		}
	}

	public class Interaction {
		public void set(Option option) {
			Support.emit("optionSet", slot(), option.getName());
		}

		public void remove(Option option) {
			Support.emit("optionRemove", slot(), option.getName());
		}
	}

	public SkullManager getSkullManager() {
		return new SkullManager(this);
	}

	public Dispatch getPacketDispatch() {
		return new Dispatch();
	}

	public Music getMusicPlayer() {
		return new Music();
	}

	public Box getInventory() {
		return new Box("inventory");
	}

	public Box getEquipment() {
		return new Box("equipment");
	}

	public Box getBank() {
		return new Box("bank");
	}

	public Details getDetails() {
		return new Details();
	}

	public Interfaces getInterfaceManager() {
		return new Interfaces();
	}

	public Teleporter getTeleporter() {
		return new Teleporter();
	}

	public Familiars getFamiliarManager() {
		return new Familiars();
	}

	public Appearance getAppearance() {
		return new Appearance();
	}

	public Interaction getInteraction() {
		return new Interaction();
	}
}
''')

adapter('core/game/node/entity/npc/NPC.java', '''
package core.game.node.entity.npc;

import core.game.node.entity.Entity;
import core.game.node.entity.npc.agg.AggressiveHandler;
import core.game.world.map.Location;
import java.util.Map;
import oracle.Support;

/** Recording NPC. */
public class NPC extends Entity {
	public NPC(int slot, Map<String, Object> state) {
		super(slot, state);
	}

	public class Definition {
		public boolean hasAttackOption() {
			boolean answer = Support.flag(state(), "attackOption", true);
			Support.emit("attackOption", slot());
			return answer;
		}

		public int getCombatLevel() {
			int level = Support.num(state(), "npcCombat", 1);
			Support.emit("npcCombat", slot());
			return level;
		}
	}

	public void setInvisible(boolean invisible) {
		state().put("invisible", invisible);
		Support.emit("setInvisible", slot(), invisible);
	}

	public boolean isInvisible() {
		boolean invisible = Support.flag(state(), "invisible", false);
		Support.emit("invisible", slot());
		return invisible;
	}

	public Definition getDefinition() {
		return new Definition();
	}

	public String getName() {
		String name = state().containsKey("name") ? (String) state().get("name") : "npc" + slot();
		Support.emit("npcName", slot());
		return name;
	}

	public boolean isAggressive() {
		boolean aggressive = Support.flag(state(), "aggressive", false);
		Support.emit("aggressive", slot());
		return aggressive;
	}

	public void setAggressiveHandler(AggressiveHandler handler) {
		Support.emit("aggressiveHandler", slot(), handler.behavior());
	}

	public Location getDropLocation() {
		Location location = Support.loc(state().get("drop"));
		Support.emit("dropLocation", slot());
		return location;
	}

	public void reTransform() {
		Support.emit("reTransform", slot());
	}

	public boolean shouldPreventStacking(Entity mover) {
		boolean answer = Support.flag(state(), "preventStacking", true);
		Support.emit("preventStacking", slot(), mover.slot());
		return answer;
	}

	public void onRegionInactivity() {
		Support.emit("regionInactivity", slot());
	}
}
''')

adapter('content/global/skill/summoning/familiar/Familiar.java', '''
package content.global.skill.summoning.familiar;

import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import java.util.Map;
import oracle.Support;

/** Recording familiar: an NPC with an owner. */
public class Familiar extends NPC {
	public Familiar(int slot, Map<String, Object> state) {
		super(slot, state);
	}

	public Player getOwner() {
		Player owner = (Player) Support.entity(state().get("owner"));
		Support.emit("owner", slot());
		return owner;
	}

	public void transform() {
		Support.emit("transform", slot());
	}

	public boolean isCombatFamiliar() {
		boolean combat = Support.flag(state(), "combatFamiliar", false);
		Support.emit("combatFamiliar", slot());
		return combat;
	}
}
''')

adapter('core/game/node/entity/npc/agg/AggressiveBehavior.java', '''
package core.game.node.entity.npc.agg;

/** The behaviour objects, named for the record. */
public class AggressiveBehavior {
	public static final AggressiveBehavior DEFAULT = new AggressiveBehavior("DEFAULT");
	public static final AggressiveBehavior WILDERNESS = new AggressiveBehavior("WILDERNESS");

	private final String name;

	private AggressiveBehavior(String name) {
		this.name = name;
	}

	public String name() {
		return name;
	}
}
''')

adapter('core/game/node/entity/npc/agg/AggressiveHandler.java', '''
package core.game.node.entity.npc.agg;

import core.game.node.entity.Entity;

/** The original constructor only stores its arguments. */
public class AggressiveHandler {
	private final Entity entity;
	private final AggressiveBehavior behavior;

	public AggressiveHandler(Entity entity, AggressiveBehavior behavior) {
		this.entity = entity;
		this.behavior = behavior;
	}

	public String behavior() {
		return behavior.name();
	}
}
''')

adapter('core/game/interaction/Option.java', '''
package core.game.interaction;

/** An option name; _P_ATTACK is the original field. */
public class Option {
	/* ORIGINAL_P_ATTACK */

	private final String name;

	public Option(String name) {
		this.name = name;
	}

	public Option(String name, int index) {
		this.name = name;
	}

	public String getName() {
		return name;
	}
}
''')

adapter('core/game/interaction/Script.kt', '''
package core.game.interaction

/* ORIGINAL_QUEUE_STRENGTH */
''')

adapter('core/game/event/Events.kt', '''
package core.game.event

/* ORIGINAL_USE_WITH_EVENT */
''')

adapter('core/api/Event.kt', '''
package core.api

import core.game.event.UseWithEvent

/** The one event class DarkZone hooks. */
object Event {
    /* ORIGINAL_USED_WITH */
}
''')

adapter('content/global/skill/skillcapeperks/SkillcapePerks.kt', '''
package content.global.skill.skillcapeperks

import core.game.node.entity.player.Player

/** The one perk DarkZone reads; isActive is a service of the case. */
enum class SkillcapePerks {
    CONSTANT_GLOW;

    companion object {
        @JvmStatic
        fun isActive(perk: SkillcapePerks, player: Player): Boolean {
            val active = oracle.Support.flag(player.state(), "glow", false)
            oracle.Support.emit("perk", player.slot(), perk.name)
            return active
        }
    }
}
''')

adapter('core/tools/RandomFunction.java', '''
package core.tools;

import java.util.Random;

/** The shared generator and the members the zones call: original text. */
public class RandomFunction {
	/* ORIGINAL_RANDOM_FIELD */

	/* ORIGINAL_RANDOM_RANGE */

	/* ORIGINAL_ROLL */

	/* ORIGINAL_RANDOM_MAX */

	/* ORIGINAL_NORMAL_RAND_DIST */
}
''')

adapter('core/game/world/GameWorld.java', '''
package core.game.world;

import oracle.Support;

/** The pulse runner and the settings of the case. */
public class GameWorld {
	public static class Settings {
		public boolean getWild_pvp_enabled() {
			boolean enabled = Support.flag(Support.input, "pvp", true);
			Support.emit("wildPvp");
			return enabled;
		}
	}

	public static PulseRunner getPulser() {
		return Support.runner;
	}

	public static Settings getSettings() {
		return Support.flag(Support.input, "noSettings", false) ? null : new Settings();
	}
}
''')

adapter('core/game/world/repository/Repository.java', '''
package core.game.world.repository;

import core.game.node.entity.npc.NPC;
import oracle.Support;

/** Recording repository. */
public class Repository {
	public static void addRenderableNPC(NPC npc) {
		Support.emit("renderable", npc.slot());
	}

	public static void sendNews(String string) {
		Support.emit("news", string);
	}
}
''')

adapter('core/game/node/item/GroundItemManager.java', '''
package core.game.node.item;

import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import oracle.Support;

/** Recording ground items. */
public class GroundItemManager {
	public static void create(Item item, Location location, Player player) {
		Support.emit("groundItem", item.getId(), item.getAmount(), Support.coord(location), player.slot());
	}
}
''')

adapter('core/api/ContentAPI.kt', '''
package core.api

import core.game.interaction.QueueStrength
import core.game.node.entity.Entity
import core.tools.Log

/** Recording log service. */
fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", type.name, message)
}

/** Recording queueScript: the script is kept for the case to run. */
fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    oracle.Support.emit("queueScript", entity.slot(), delay, strength.name, persist)
    oracle.Support.scripts.add(script)
}

/** Recording stopExecuting: its answer is the case's. */
fun stopExecuting(entity: Entity): Boolean {
    oracle.Support.emit("stopExecuting", entity.slot())
    return oracle.Support.answer("stopExecuting") ?: true
}

/** Recording runTask: the task is kept for the case to run. */
fun runTask(entity: Entity, delay: Int = 0, repeatTimes: Int = 1, task: () -> Unit) {
    oracle.Support.emit("runTask", entity.slot(), delay, repeatTimes)
    oracle.Support.tasks.add(task)
}

/** Recording getItemName: the name is the case's. */
fun getItemName(id: Int): String {
    oracle.Support.emit("getItemName", id)
    return oracle.Support.itemName(id)
}
''')
