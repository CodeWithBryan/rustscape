"""Stand-ins for everything the unchanged timer classes reach outside themselves.

Each entry is one source file compiled next to the unchanged RSTimer,
PersistTimer, TimerFlag, TimerManager, TimerRegistry, the fourteen classes of
core/game/system/timer/impl, SpellBookManager, Event, EventHook, EquipmentSlot,
Privilege, Globals and TickUtils. `/* ORIGINAL_* */` markers are filled with
original text at generation time (generate.py, `markers`). Every recorded call
goes through oracle.Support.emit, which also injects the failures of the sweep.
Stand-ins for paths the corpus does not drive throw.
"""

ADAPTERS = {}

ADAPTERS['core/tools/RandomFunction.java'] = '''package core.tools;
import java.util.Random;
// The selected original methods, verbatim; RANDOM is the case's seeded, recording generator.
public final class RandomFunction {
    public static Random RANDOM = new Random(0);
    /* ORIGINAL_RANDOM_METHODS */
}
'''

ADAPTERS['core/tools/Log.kt'] = '''package core.tools
// The original Log constants (SystemLogger.kt).
/* ORIGINAL_LOG_ENUM */
'''

ADAPTERS['core/game/node/Node.java'] = '''package core.game.node;
import core.game.node.entity.player.Player;
import core.game.node.entity.npc.NPC;
import core.game.world.map.Location;
// A node of the case: its name and location are recorded reads.
public abstract class Node {
    public String getName() { oracle.Support.emit("name", oracle.Support.ix(this)); return oracle.Support.name(this); }
    public Location getLocation() { oracle.Support.emit("location", oracle.Support.ix(this)); return oracle.Support.location(this); }
    public Player asPlayer() { return (Player) this; }
    public NPC asNpc() { return (NPC) this; }
}
'''

ADAPTERS['core/game/world/map/Location.kt'] = '''package core.game.world.map
// Coordinates and the original toString.
class Location(val x: Int, val y: Int, val z: Int) {
    /* ORIGINAL_LOCATION_TO_STRING */
}
'''

ADAPTERS['core/game/node/entity/Scripts.java'] = '''package core.game.node.entity;
// Entity.scripts: the delay read and removeWeakScripts are recorded.
public class Scripts {
    private final Entity entity;
    public Scripts(Entity entity) { this.entity = entity; }
    public int getDelay() { oracle.Support.emit("delay", oracle.Support.ix(entity)); return oracle.Support.number(entity, "delay", 0); }
    public void removeWeakScripts() { oracle.Support.emit("remove_weak_scripts", oracle.Support.ix(entity)); }
}
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.event.Event;
import core.game.event.EventHook;
import core.game.node.Node;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.skill.Skills;
import core.game.system.timer.TimerManager;
import core.game.world.map.zone.ZoneMonitor;
import java.util.ArrayList;
import java.util.HashMap;
// The original hook map with dispatch, unhook and hook (each call recorded, then the original body);
// attributes are the case's map, each call recorded.
public abstract class Entity extends Node {
    public final Scripts scripts = new Scripts(this);
    /* ORIGINAL_HOOKS_FIELD */
    public TimerManager timers = new TimerManager(this);
    private final Skills skills = new Skills(this);
    private final ImpactHandler impactHandler = new ImpactHandler(this);
    private final ZoneMonitor zoneMonitor = new ZoneMonitor(this);
    private final Properties properties = new Properties(this);
    public final HashMap<String, Object> attributes = new HashMap<>();
    public Skills getSkills() { return skills; }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public ZoneMonitor getZoneMonitor() { return zoneMonitor; }
    public Properties getProperties() { return properties; }
    public HashMap<Class<?>, ArrayList<EventHook>> hookMap() { return hooks; }

    public void dispatch(Event event) { oracle.Support.emit("dispatch", oracle.Support.ix(this), oracle.Support.describe(event)); originalDispatch(event); }
    public void unhook(EventHook hook) { oracle.Support.emit("unhook", oracle.Support.ix(this), oracle.Support.hookLabel(hook)); originalUnhook(hook); }
    public void hook(Class<?> event, EventHook hook) {
        oracle.Support.emit("hook", oracle.Support.ix(this), event.getSimpleName(), oracle.Support.hookLabel(hook));
        originalHook(event, hook);
    }
    /* ORIGINAL_HOOK_METHODS */

    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        oracle.Support.emit("get_attribute", oracle.Support.ix(this), key, oracle.Support.value(fail));
        Object v = attributes.get(key);
        return v == null ? fail : (T) v;
    }
    public void setAttribute(String key, Object value) {
        oracle.Support.emit("set_attribute", oracle.Support.ix(this), key, oracle.Support.value(value));
        attributes.put(key, value);
    }
    public void removeAttribute(String key) { oracle.Support.emit("remove_attribute", oracle.Support.ix(this), key); attributes.remove(key); }
    @SuppressWarnings("unchecked")
    public <T> T getExtension(Class<?> clazz) {
        oracle.Support.emit("extension", oracle.Support.ix(this), clazz.getSimpleName());
        return (T) oracle.Support.extension(this, clazz);
    }
    public boolean isPoisonImmune() { oracle.Support.emit("poison_immune", oracle.Support.ix(this)); return oracle.Support.flag(this, "immune", false); }
}
'''

ADAPTERS['core/game/node/entity/Properties.java'] = '''package core.game.node.entity;
import core.game.node.entity.combat.spell.CombatSpell;
// The autocast spell read is recorded.
public class Properties {
    private final Entity entity;
    public Properties(Entity entity) { this.entity = entity; }
    public CombatSpell getAutocastSpell() {
        oracle.Support.emit("autocast", oracle.Support.ix(entity));
        return oracle.Support.flag(entity, "autocast", false) ? new CombatSpell() : null;
    }
}
'''

ADAPTERS['core/game/node/entity/combat/spell/CombatSpell.java'] = '''package core.game.node.entity.combat.spell;
public class CombatSpell {}
'''

ADAPTERS['core/game/node/entity/combat/spell/MagicSpell.java'] = '''package core.game.node.entity.combat.spell;
// SpellBook.register's argument; never driven.
public class MagicSpell { public void setSpellId(int id) { throw new IllegalStateException("not driven"); } }
'''

ADAPTERS['core/game/component/Component.java'] = '''package core.game.component;
public class Component {
    public final int id;
    public Component(int id) { this.id = id; }
}
'''

ADAPTERS['core/game/node/entity/combat/equipment/WeaponInterface.java'] = '''package core.game.node.entity.combat.equipment;
import core.game.component.Component;
import core.game.node.entity.player.Player;
// The weapon tab of the case; selectAutoSpell is recorded.
public class WeaponInterface extends Component {
    private final Player player;
    public WeaponInterface(Player player, int id) { super(id); this.player = player; }
    public void selectAutoSpell(int buttonId, boolean adjustAttackStyle, boolean forgetAutocast) {
        oracle.Support.emit("select_auto_spell", oracle.Support.ix(player), id, buttonId, adjustAttackStyle, forgetAutocast);
    }
}
'''

ADAPTERS['core/game/node/entity/combat/ImpactHandler.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
// The original HitsplatType enum; manualHit is recorded.
public class ImpactHandler {
    private final Entity entity;
    public ImpactHandler(Entity entity) { this.entity = entity; }
    public void manualHit(Entity source, int hit, HitsplatType type) {
        oracle.Support.emit("manual_hit", oracle.Support.ix(entity), oracle.Support.ix(source), hit, type == null ? null : type.name());
    }
    /* ORIGINAL_HITSPLAT */
}
'''

ADAPTERS['core/game/node/entity/skill/Skills.java'] = '''package core.game.node.entity.skill;
import content.global.skill.skillcapeperks.SkillcapePerks;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.request.assist.AssistSession;
import core.net.packet.PacketRepository;
import core.net.packet.context.SkillContext;
import core.net.packet.out.SkillLevel;
// The original constants, fields and the members the timers use, verbatim.
public final class Skills {
    /* ORIGINAL_SKILL_CONSTANTS */
    private final Entity entity;
    public final int[] staticLevels = new int[24];
    public final int[] dynamicLevels = new int[24];
    public int lifepoints = 10;
    public int lifepointsIncrease = 0;
    public boolean lifepointsUpdate = false;
    /* ORIGINAL_SKILL_FIELDS */
    public Skills(Entity entity) { this.entity = entity; }
    /* ORIGINAL_SKILLS_METHODS */
}
'''

ADAPTERS['core/game/node/entity/player/link/request/assist/AssistSession.java'] = '''package core.game.node.entity.player.link.request.assist;
import core.game.node.entity.player.Player;
// Never present in the corpus (getExtension answers null).
public class AssistSession {
    public Player getPlayer() { throw new IllegalStateException("not driven"); }
    public int getSkillIndex(int slot) { throw new IllegalStateException("not driven"); }
    public boolean isRestricted() { throw new IllegalStateException("not driven"); }
    public boolean[] getSkills() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/net/packet/PacketRepository.java'] = '''package core.net.packet;
import core.net.packet.context.SkillContext;
// Records the SkillLevel packets the skills members send.
public final class PacketRepository {
    public static void send(Class<?> type, SkillContext context) {
        oracle.Support.emit("skill_packet", oracle.Support.ix(context.player), context.slot);
    }
}
'''

ADAPTERS['core/net/packet/context/SkillContext.java'] = '''package core.net.packet.context;
import core.game.node.entity.player.Player;
public class SkillContext {
    public final Player player;
    public final int slot;
    public SkillContext(Player player, int slot) { this.player = player; this.slot = slot; }
}
'''

ADAPTERS['core/net/packet/out/SkillLevel.java'] = '''package core.net.packet.out;
public class SkillLevel {}
'''

ADAPTERS['content/global/skill/skillcapeperks/SkillcapePerks.java'] = '''package content.global.skill.skillcapeperks;
import core.game.node.entity.player.Player;
// isActive records the query and answers from the case.
public enum SkillcapePerks {
    DAMAGE_SPONG;
    public static boolean isActive(SkillcapePerks perk, Player player) {
        oracle.Support.emit("perk", oracle.Support.ix(player), perk.name());
        return oracle.Support.flag(player, "sponge", false);
    }
}
'''

ADAPTERS['core/game/node/entity/npc/NPC.java'] = '''package core.game.node.entity.npc;
import content.global.skill.slayer.Tasks;
import core.game.node.entity.Entity;
// The slayer task read is recorded.
public class NPC extends Entity {
    public Tasks getTask() {
        oracle.Support.emit("npc_task", oracle.Support.ix(this));
        Object level = oracle.Support.spec(this, "task");
        return level == null ? null : new Tasks(((Number) level).intValue());
    }
}
'''

ADAPTERS['content/global/skill/slayer/Tasks.java'] = '''package content.global.skill.slayer;
// A task with the case's level requirement.
public class Tasks {
    private final int levelReq;
    public Tasks(int levelReq) { this.levelReq = levelReq; }
    public int getLevelReq() { return levelReq; }
}
'''

ADAPTERS['content/global/skill/summoning/familiar/Familiar.java'] = '''package content.global.skill.summoning.familiar;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
// The owner read is recorded.
public class Familiar extends NPC {
    public Player getOwner() {
        oracle.Support.emit("familiar_owner", oracle.Support.ix(this));
        return (Player) oracle.Support.entityAt(this, "owner");
    }
}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
import core.game.container.impl.EquipmentContainer;
import core.game.node.entity.Entity;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.info.Rights;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.Settings;
import core.game.node.entity.player.link.SkullManager;
import core.game.node.entity.player.link.SpellBookManager;
import java.util.HashMap;
import kotlin.Unit;
import kotlin.jvm.functions.Function1;
// The original logout listener map and SpellBookManager field; every other member is recorded and answered from the case.
public class Player extends Entity {
    /* ORIGINAL_LOGOUT_LISTENERS */
    private final Settings settings = new Settings(this);
    private final SkullManager skullManager = new SkullManager(this);
    private final SpellBookManager spellBookManager = new SpellBookManager();
    private final InterfaceManager interfaceManager = new InterfaceManager(this);
    private final PlayerDetails details = new PlayerDetails(this);
    private final EquipmentContainer equipment = new EquipmentContainer(this);
    public Settings getSettings() { return settings; }
    public SkullManager getSkullManager() { return skullManager; }
    public SpellBookManager getSpellBookManager() { return spellBookManager; }
    public InterfaceManager getInterfaceManager() { return interfaceManager; }
    public PlayerDetails getDetails() { return details; }
    public EquipmentContainer getEquipment() { return equipment; }
    public boolean hasModalOpen() { oracle.Support.emit("modal", oracle.Support.ix(this)); return oracle.Support.flag(this, "modal", false); }
    public void debug(String text) { oracle.Support.emit("debug", oracle.Support.ix(this), text); }
    public boolean isArtificial() { oracle.Support.emit("artificial", oracle.Support.ix(this)); return oracle.Support.flag(this, "artificial", false); }
    public Rights getRights() {
        oracle.Support.emit("rights", oracle.Support.ix(this));
        return Rights.valueOf((String) oracle.Support.specOr(this, "rights", "REGULAR_PLAYER"));
    }
}
'''

ADAPTERS['core/game/node/entity/player/info/PlayerDetails.java'] = '''package core.game.node.entity.player.info;
import core.game.node.entity.player.Player;
// The uid read is recorded.
public class PlayerDetails {
    private final Player player;
    public PlayerDetails(Player player) { this.player = player; }
    public int getUid() { oracle.Support.emit("uid", oracle.Support.ix(player)); return oracle.Support.number(player, "uid", 0); }
}
'''

ADAPTERS['core/game/node/entity/player/info/Rights.java'] = '''package core.game.node.entity.player.info;
// The original constant names.
public enum Rights { /* ORIGINAL_RIGHTS */ }
'''

ADAPTERS['core/game/container/impl/EquipmentContainer.java'] = '''package core.game.container.impl;
import core.game.node.item.Item;
// Worn item reads are recorded.
public class EquipmentContainer {
    private final core.game.node.entity.Entity owner;
    public EquipmentContainer(core.game.node.entity.Entity owner) { this.owner = owner; }
    public Item get(int slot) {
        oracle.Support.emit("equipment", oracle.Support.ix(owner), slot);
        Integer id = oracle.Support.worn(owner, slot);
        return id == null ? null : new Item(id);
    }
}
'''

ADAPTERS['core/game/node/item/Item.java'] = '''package core.game.node.item;
public class Item {
    private final int id;
    private final int amount;
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { this.id = id; this.amount = amount; }
    public int getId() { return id; }
    public int getAmount() { return amount; }
}
'''

ADAPTERS['core/game/node/entity/player/link/Settings.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
import static core.api.ContentAPIKt.setVarp;
// The original special energy field and its two members, verbatim.
public class Settings {
    private final Player player;
    /* ORIGINAL_SPECIAL_FIELD */
    public Settings(Player player) { this.player = player; }
    public void set(int value) { specialEnergy = value; }
    /* ORIGINAL_SETTINGS_METHODS */
}
'''

ADAPTERS['core/game/node/entity/player/link/SkullManager.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
// The three members the Skulled timer calls are recorded.
public class SkullManager {
    private final Player player;
    public SkullManager(Player player) { this.player = player; }
    public void setSkullIcon(int icon) { oracle.Support.emit("skull_icon", oracle.Support.ix(player), icon); }
    public void setSkulled(boolean skulled) { oracle.Support.emit("set_skulled", oracle.Support.ix(player), skulled); }
    public void reset() { oracle.Support.emit("reset_skull", oracle.Support.ix(player)); }
}
'''

ADAPTERS['core/game/node/entity/player/link/InterfaceManager.java'] = '''package core.game.node.entity.player.link;
import core.game.component.Component;
import core.game.node.entity.player.Player;
// openTab is recorded with the component id.
public class InterfaceManager {
    private final Player player;
    public InterfaceManager(Player player) { this.player = player; }
    public void openTab(Component component) { oracle.Support.emit("open_tab", oracle.Support.ix(player), component.id); }
}
'''

ADAPTERS['core/game/node/entity/player/link/prayer/PrayerType.java'] = '''package core.game.node.entity.player.link.prayer;
// The original constant names, in declaration order.
public enum PrayerType { /* ORIGINAL_PRAYER_TYPES */ }
'''

ADAPTERS['core/game/world/map/zone/ZoneRestriction.java'] = '''package core.game.world.map.zone;
// The original constant names, in declaration order.
public enum ZoneRestriction { /* ORIGINAL_ZONE_RESTRICTIONS */ }
'''

ADAPTERS['core/game/world/map/zone/ZoneMonitor.java'] = '''package core.game.world.map.zone;
import core.game.node.entity.Entity;
// isRestricted is recorded and answered from the case.
public class ZoneMonitor {
    private final Entity entity;
    public ZoneMonitor(Entity entity) { this.entity = entity; }
    public boolean isRestricted(ZoneRestriction restriction) {
        oracle.Support.emit("zone_restricted", oracle.Support.ix(entity), restriction.name());
        return oracle.Support.flag(entity, "restricted", false);
    }
}
'''

ADAPTERS['core/game/world/repository/Repository.java'] = '''package core.game.world.repository;
import core.game.node.entity.player.Player;
// getPlayerByUid is recorded and answered from the case.
public final class Repository {
    public static Player getPlayerByUid(int uid) { oracle.Support.emit("player_by_uid", uid); return oracle.Support.byUid(uid); }
    public static Player getPlayerByName(String name) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/dialogue/DialoguePlugin.java'] = '''package core.game.dialogue;
// A closed dialogue of the case; getIds is recorded.
public class DialoguePlugin {
    public final String label;
    public DialoguePlugin(String label) { this.label = label; }
    public int[] getIds() { oracle.Support.emit("dialogue_ids", label); return oracle.Support.dialogueIds(label); }
}
'''

ADAPTERS['core/game/event/Events.kt'] = '''package core.game.event
import core.game.dialogue.DialoguePlugin
import core.game.node.Node
import core.game.node.entity.player.link.SpellBookManager.SpellBook
import core.game.node.entity.player.link.SpellBookManager.SpellbookChangeSource
import core.game.node.entity.player.link.prayer.PrayerType
// The original declarations of the five events the timers hook.
/* ORIGINAL_EVENTS */
'''

ADAPTERS['core/api/Event.kt'] = '''package core.api
import core.game.event.*
// The original constants of the five events the timers hook.
object Event {
    /* ORIGINAL_EVENT_CONSTANTS */
}
'''

ADAPTERS['core/api/Commands.kt'] = '''package core.api
import core.game.node.entity.player.Player
import core.game.system.command.Privilege
// The command members AntiMacro.defineCommands calls; the command body is not driven.
interface Commands {
    fun reject(player: Player, vararg message: String) { throw IllegalStateException("not driven") }
    fun notify(player: Player, message: String, logToConsole: Boolean = false) { throw IllegalStateException("not driven") }
    fun define(name: String, privilege: Privilege = Privilege.ADMIN, usage: String = "", description: String = "", handle: (Player, Array<String>) -> Unit) {
        throw IllegalStateException("not driven")
    }
    fun defineCommands()
}
'''

ADAPTERS['core/api/utils/WeightBasedTable.kt'] = '''package core.api.utils
import core.game.node.entity.Entity
import core.game.node.item.Item
// A random event's loot table: roll is recorded and answered from the case.
class WeightBasedTable(val label: String) {
    fun roll(receiver: Entity? = null): ArrayList<Item> {
        oracle.Support.emit("roll_loot", label, oracle.Support.ix(receiver))
        return oracle.Support.loot(label)
    }
}
'''

ADAPTERS['content/global/ame/RandomEventNPC.kt'] = '''package content.global.ame
import core.api.utils.WeightBasedTable
import core.game.node.entity.player.Player
import core.game.world.map.Location
// The members AntiMacro reaches: create, spawnLocation, init, terminate and loot.
open class RandomEventNPC(val label: String) {
    var loot: WeightBasedTable? = null
    var spawnLocation: Location? = null
        get() { oracle.Support.emit("spawn_location", label); return field }
    open fun create(player: Player, loot: WeightBasedTable? = null, type: String = ""): RandomEventNPC {
        oracle.Support.emit("create_event_npc", label, oracle.Support.ix(player), loot?.label, type)
        return oracle.Support.createNpc(this, loot)
    }
    open fun init() { oracle.Support.emit("init_npc", label) }
    open fun terminate() { oracle.Support.emit("terminate_npc", label) }
}
'''

ADAPTERS['content/global/ame/RandomEvents.kt'] = '''package content.global.ame
import core.api.utils.WeightBasedTable
// The original constant names in declaration order; loot and type are the original's (CERTER and
// RICK_TURPENTINE share the CERTER table, SURPRISE_EXAM has type "sexam"). The two pool draws are recorded.
enum class RandomEvents(val npc: RandomEventNPC, val loot: WeightBasedTable? = null, val type: String = "") {
    /* ORIGINAL_RANDOM_EVENTS */;
    companion object {
        fun getSkillBasedRandomEvent(skill: Int): RandomEvents? {
            oracle.Support.emit("skill_random", skill)
            return oracle.Support.skillRandom(skill)
        }
        fun getNonSkillRandom(): RandomEvents {
            oracle.Support.emit("non_skill_random")
            return oracle.Support.nonSkillRandom()
        }
    }
}
'''

ADAPTERS['core/api/ContentAPI.kt'] = '''@file:JvmName("ContentAPIKt")
package core.api
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.game.system.timer.RSTimer
import core.game.system.timer.TimerRegistry
import core.game.system.timer.impl.Poison
import core.game.system.timer.impl.PoisonImmunity
import core.game.world.map.Location
import core.tools.Log
// The timer, poison, level, equipment and attribute functions are original text; the rest record.

/* ORIGINAL_CONTENT_API */

fun getWorldTicks(): Int = oracle.Support.ticks()

fun sendMessage(player: Player, message: String) { oracle.Support.emit("message", oracle.Support.ix(player), message) }

@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: Location? = null, radius: Int = 8) {
    oracle.Support.emit("audio", oracle.Support.ix(player), id, delay, loops, location?.toString(), radius)
}

@JvmOverloads
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("varp", oracle.Support.ix(player), varpIndex, value, save)
}

@JvmOverloads
fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("varbit", oracle.Support.ix(player), varbitId, value, save)
}

fun log(origin: Class<*>, type: Log, message: String) { oracle.Support.emit("log", type.name, message) }

fun registerLogoutListener(player: Player, key: String, handler: (p: Player) -> Unit) {
    oracle.Support.emit("register_logout", oracle.Support.ix(player), key)
    player.logoutListeners[key] = handler
}

fun clearLogoutListener(player: Player, key: String) {
    oracle.Support.emit("clear_logout", oracle.Support.ix(player), key)
    player.logoutListeners.remove(key)
}
'''
