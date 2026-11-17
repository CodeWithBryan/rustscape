"""Stand-ins for everything the unchanged consumable classes reach outside themselves.

Each entry is one source file compiled next to the unchanged core/game/consumable classes,
content/data/consumables/Consumables.java with every effect class, ConsumableListener.kt,
Script.kt, Clocks.kt, IntType.kt, Pulse.java and TickUtils.kt. `/* ORIGINAL_* */` markers are
filled with original text at generation time (generate.py, `markers`). Every recorded call goes
through oracle.Support.emit, which also injects the failures of the sweep. Stand-ins for paths the
corpus does not drive throw IllegalStateException("not driven").
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

ADAPTERS['core/plugin/Plugin.java'] = '''package core.plugin;
// Imported (unused) by Consumable.java.
public interface Plugin<T> {}
'''

ADAPTERS['core/game/node/Node.java'] = '''package core.game.node;
// A node of the case: an id; isActive is Pulse's node check (the consumable pulses have none).
public abstract class Node {
    public int getId() { throw new IllegalStateException("not driven"); }
    public boolean isActive() { throw new IllegalStateException("not driven"); }
    public core.game.node.entity.player.Player asPlayer() { return (core.game.node.entity.player.Player) this; }
}
'''

ADAPTERS['oracle/OtherNode.java'] = '''package oracle;
// A node that is not an Item (an NPC or object with an item's id).
public class OtherNode extends core.game.node.Node {
    private final int id;
    public OtherNode(int id) { this.id = id; }
    @Override public int getId() { return id; }
}
'''

ADAPTERS['core/game/node/item/Item.java'] = '''package core.game.node.item;
// The original idHash (id << 16 | charge, charge 1000) and getId's mask; the name is the
// definition name the constructor reads (the case's table, "null" for an id without one).
public class Item extends core.game.node.Node {
    private final int idHash;
    private int amount;
    public int index = -1;
    private final String name;
    public Item(int id) { this(id, 1, 1000); }
    public Item(int id, int amount) { this(id, amount, 1000); }
    public Item(int id, int amount, int charge) {
        this.name = oracle.Support.itemName(id);
        this.idHash = id << 16 | charge;
        this.amount = amount;
    }
    @Override public int getId() { return idHash >> 16 & 0xFFFF; }
    public int getAmount() { return amount; }
    public int getSlot() { return index; }
    public String getName() { return name; }
    /* ORIGINAL_ITEM_TO_STRING */
}
'''

ADAPTERS['core/game/world/map/Location.kt'] = '''package core.game.world.map
// Coordinates; TroubleBrewingRumEffect builds one with the two-argument constructor.
class Location(val x: Int, val y: Int, val z: Int) {
    constructor(x: Int, y: Int) : this(x, y, 0)
}
'''

ADAPTERS['core/game/world/update/flag/context/Animation.java'] = '''package core.game.world.update.flag.context;
// An animation id; Entity.animate records it.
public class Animation {
    private final int id;
    public Animation(int id) { this.id = id; }
    public int getId() { return id; }
}
'''

ADAPTERS['core/game/node/entity/player/link/audio/Audio.kt'] = '''package core.game.node.entity.player.link.audio
// Potion's static SOUND (never played).
class Audio @JvmOverloads constructor(val id: Int, val delay: Int = 0, val loops: Int = 1, val radius: Int = 8)
'''

ADAPTERS['core/game/world/GameWorld.kt'] = '''package core.game.world
// GameWorld.ticks: every read is recorded.
object GameWorld {
    @JvmStatic
    val ticks: Int
        get() = oracle.Support.ticks()
}
'''

ADAPTERS['core/game/node/entity/combat/ImpactHandler.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
// The original HitsplatType enum; manualHit is recorded.
public class ImpactHandler {
    private final Entity entity;
    public ImpactHandler(Entity entity) { this.entity = entity; }
    public void manualHit(Entity source, int hit, HitsplatType type) {
        oracle.Support.emit("manual_hit", oracle.Support.ix(source), hit, type == null ? null : type.name());
    }
    /* ORIGINAL_HITSPLAT */
}
'''

ADAPTERS['core/game/node/entity/combat/CombatPulse.java'] = '''package core.game.node.entity.combat;
// delayNextAttack is recorded.
public class CombatPulse {
    public void delayNextAttack(int ticks) { oracle.Support.emit("delay_next_attack", ticks); }
}
'''

ADAPTERS['core/game/node/entity/Properties.java'] = '''package core.game.node.entity;
import core.game.node.entity.combat.CombatPulse;
public class Properties {
    private final CombatPulse combatPulse = new CombatPulse();
    public CombatPulse getCombatPulse() { return combatPulse; }
}
'''

ADAPTERS['core/game/node/entity/impl/PulseManager.java'] = '''package core.game.node.entity.impl;
import core.game.system.task.Pulse;
// run(pulse) is recorded with the pulse's label and delay; the case runs pulse() itself.
public class PulseManager {
    public void run(Pulse pulse) { oracle.Support.runPulse(pulse); }
}
'''

ADAPTERS['core/game/interaction/ScriptProcessor.kt'] = '''package core.game.interaction
import core.game.node.entity.Entity
// The three members ContentAPI's queueScript, stopExecuting and clearScripts reach; each is recorded.
class ScriptProcessor(val entity: Entity) {
    fun getActiveScript(): Script<*>? {
        oracle.Support.emit("active_script")
        return oracle.Support.activeScript()
    }
    fun addToQueue(script: Script<*>, strength: QueueStrength) {
        oracle.Support.emit("add_to_queue", strength.name, script.nextExecution, script.persist)
        oracle.Support.queued.add(script)
    }
    fun reset() { oracle.Support.emit("reset_scripts") }
}
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.interaction.ScriptProcessor;
import core.game.node.Node;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.PulseManager;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import java.util.HashMap;
// The original public clocks array and scripts field; attributes are the case's map; every member call is recorded.
public abstract class Entity extends Node {
    public final int[] clocks = new int[10];
    public ScriptProcessor scripts = new ScriptProcessor(this);
    private final Skills skills = new Skills(this);
    private final ImpactHandler impactHandler = new ImpactHandler(this);
    private final Properties properties = new Properties();
    private final PulseManager pulseManager = new PulseManager();
    public final HashMap<String, Object> attributes = new HashMap<>();
    public Skills getSkills() { return skills; }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public Properties getProperties() { return properties; }
    public PulseManager getPulseManager() { return pulseManager; }

    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        oracle.Support.emit("get_attribute", key, oracle.Support.value(fail));
        Object v = attributes.get(key);
        return v == null ? fail : (T) v;
    }
    public void setAttribute(String key, Object value) {
        oracle.Support.emit("set_attribute", key, oracle.Support.value(value));
        attributes.put(key, value);
    }
    public boolean animate(Animation animation) {
        oracle.Support.emit("animate", animation == null ? null : animation.getId());
        return true;
    }
    public boolean sendChat(String string) { oracle.Support.emit("chat", string); return true; }
    public void teleport(Location location) {
        oracle.Support.emit("teleport", location == null ? null : java.util.Arrays.asList(location.getX(), location.getY(), location.getZ()));
    }
    public <T> T getExtension(Class<?> clazz) {
        oracle.Support.emit("extension", clazz.getSimpleName());
        return null;
    }
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
// The original constants, fields and the members the consumables use, verbatim.
public final class Skills {
    /* ORIGINAL_SKILL_CONSTANTS */
    /* ORIGINAL_NUM_SKILLS */
    private final Entity entity;
    public final int[] staticLevels = new int[24];
    public final int[] dynamicLevels = new int[24];
    public int lifepoints = 10;
    public int lifepointsIncrease = 0;
    public boolean lifepointsUpdate = false;
    /* ORIGINAL_PRAYER_FIELD */
    public Skills(Entity entity) { this.entity = entity; }
    public void setPrayer(double value) { prayerPoints = value; }
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
    public static void send(Class<?> type, SkillContext context) { oracle.Support.emit("skill_packet", context.slot); }
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
        oracle.Support.emit("perk", perk.name());
        return oracle.Support.flag("sponge");
    }
}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
import content.global.skill.summoning.familiar.FamiliarManager;
import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.Settings;
import core.game.node.entity.player.link.diary.AchievementDiaryManager;
// The case's player; containers, packet dispatch, settings, diary and familiar managers are stand-ins.
public class Player extends Entity {
    private final Container inventory = new Container("inventory", 28);
    private final Container equipment = new Container("equipment", 14);
    private final PacketDispatch packetDispatch = new PacketDispatch(this);
    private final Settings settings = new Settings(this);
    private final AchievementDiaryManager diaries = new AchievementDiaryManager();
    private final FamiliarManager familiarManager = new FamiliarManager();
    public Container getInventory() { return inventory; }
    public Container getEquipment() { return equipment; }
    public Container getBank() { throw new IllegalStateException("not driven"); }
    public Container getBankSecondary() { throw new IllegalStateException("not driven"); }
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public Settings getSettings() { return settings; }
    public AchievementDiaryManager getAchievementDiaryManager() { return diaries; }
    public FamiliarManager getFamiliarManager() { return familiarManager; }
}
'''

ADAPTERS['core/game/container/Container.java'] = '''package core.game.container;
import core.game.node.item.Item;
// A container of the case: its slots; replace, remove, the slot remove and contains are recorded.
public class Container {
    public final String label;
    public final Item[] items;
    public Container(String label, int size) { this.label = label; this.items = new Item[size]; }
    public Item replace(Item item, int slot) {
        oracle.Support.emit("replace", label, oracle.Support.describe(item), slot);
        if (item != null && item.getAmount() < 1) item = null;
        Item old = items[slot];
        items[slot] = item;
        if (item != null) item.index = slot;
        return old;
    }
    public boolean remove(Item item) {
        oracle.Support.emit("remove", label, oracle.Support.describe(item));
        for (int i = 0; i < items.length; i++) {
            if (items[i] != null && items[i].getId() == item.getId()) return take(i, item);
        }
        return false;
    }
    public boolean remove(Item item, int slot, boolean fireListener) {
        oracle.Support.emit("remove_slot", label, oracle.Support.describe(item), slot, fireListener);
        Item old = items[slot];
        if (old == null || old.getId() != item.getId() || old.getAmount() < item.getAmount()) return false;
        return take(slot, item);
    }
    private boolean take(int slot, Item item) {
        Item old = items[slot];
        if (item.getAmount() >= old.getAmount()) items[slot] = null;
        else { Item rest = new Item(old.getId(), old.getAmount() - item.getAmount()); rest.index = slot; items[slot] = rest; }
        return true;
    }
    public boolean contains(int id, int amount) {
        oracle.Support.emit("contains", label, id, amount);
        int total = 0;
        for (Item i : items) if (i != null && i.getId() == id) total += i.getAmount();
        return total >= amount;
    }
}
'''

ADAPTERS['core/game/node/entity/player/link/PacketDispatch.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
// sendMessage and sendRunEnergy are recorded.
public class PacketDispatch {
    private final Player player;
    public PacketDispatch(Player player) { this.player = player; }
    public void sendMessage(String message) { oracle.Support.emit("packet_message", message); }
    public void sendRunEnergy() { oracle.Support.emit("run_energy_packet"); }
}
'''

ADAPTERS['core/game/node/entity/player/link/Settings.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
// The original run energy field and updateRunEnergy, verbatim.
public class Settings {
    private final Player player;
    /* ORIGINAL_RUN_ENERGY_FIELD */
    public Settings(Player player) { this.player = player; }
    public void setRunEnergy(double value) { runEnergy = value; }
    public double getEnergy() { return runEnergy; }
    /* ORIGINAL_SETTINGS_METHODS */
}
'''

ADAPTERS['core/game/node/entity/player/link/diary/DiaryType.java'] = '''package core.game.node.entity.player.link.diary;
// The original constant names, in declaration order.
public enum DiaryType { /* ORIGINAL_DIARY_TYPES */ }
'''

ADAPTERS['core/game/node/entity/player/link/diary/AchievementDiaryManager.java'] = '''package core.game.node.entity.player.link.diary;
import core.game.node.entity.player.Player;
// finishTask is recorded.
public class AchievementDiaryManager {
    public void finishTask(Player player, DiaryType type, int level, int index) {
        oracle.Support.emit("finish_task", type == null ? null : type.name(), level, index);
    }
}
'''

ADAPTERS['content/global/skill/summoning/familiar/FamiliarManager.java'] = '''package content.global.skill.summoning.familiar;
// The familiar read is recorded and answered from the case.
public class FamiliarManager {
    private final Familiar familiar = new Familiar();
    public Familiar getFamiliar() {
        oracle.Support.emit("familiar");
        return oracle.Support.flag("familiar") ? familiar : null;
    }
    public boolean hasFamiliar() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['content/global/skill/summoning/familiar/Familiar.java'] = '''package content.global.skill.summoning.familiar;
// updateSpecialPoints is recorded.
public class Familiar {
    public void updateSpecialPoints(int diff) { oracle.Support.emit("special_points", diff); }
    public boolean isBurdenBeast() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['content/global/skill/summoning/familiar/BurdenBeast.java'] = '''package content.global.skill.summoning.familiar;
// The beast-of-burden container (Container.BoB) is never driven.
public class BurdenBeast extends Familiar {
    public core.game.container.Container getContainer() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/player/info/PlayerMonitor.kt'] = '''package core.game.node.entity.player.info
import core.game.node.entity.player.Player
// The original LogType constants; log is recorded.
object PlayerMonitor {
    @JvmStatic fun log(player: Player, type: LogType, details: String) { oracle.Support.emit("player_log", type.name, details) }
}
/* ORIGINAL_LOG_TYPE */
'''

ADAPTERS['core/game/system/timer/RSTimer.kt'] = '''package core.game.system.timer
// A timer the case's spawnTimer answers: its identifier and arguments.
open class RSTimer(val identifier: String, val args: List<Any?>)
'''

ADAPTERS['core/game/system/timer/impl/Disease.kt'] = '''package core.game.system.timer.impl
import core.game.system.timer.RSTimer
// The original Disease field the cure reads and writes.
class Disease : RSTimer("disease", emptyList()) {
    var hitsLeft = 25
}
'''

ADAPTERS['core/game/interaction/InteractionListener.kt'] = '''package core.game.interaction
import core.game.node.Node
import core.game.node.entity.player.Player
// The registration member ConsumableListener uses; it is recorded and the handler kept.
interface InteractionListener {
    fun defineListeners()
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.emit("on", type.name, option.toList())
        oracle.Support.handler = handler
    }
}
'''

ADAPTERS['core/api/ContentAPI.kt'] = '''@file:JvmName("ContentAPIKt")
package core.api
import content.global.skill.summoning.familiar.BurdenBeast
import core.game.interaction.QueueStrength
import core.game.interaction.QueuedScript
import core.game.interaction.Clocks
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.entity.player.info.LogType
import core.game.node.entity.player.info.PlayerMonitor
import core.game.node.item.Item
import core.game.system.timer.RSTimer
import core.game.world.GameWorld
import core.game.world.map.Location
import core.tools.Log
import kotlin.math.absoluteValue
// The slot, item, level, prayer, option, attack-delay, script and tick functions are original text; the rest record.

/* ORIGINAL_CONTENT_API */

@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: Location? = null, radius: Int = 8) {
    oracle.Support.emit("audio", id, delay, loops, location?.let { listOf(it.x, it.y, it.z) }, radius)
}

fun sendMessage(player: Player, message: String) { oracle.Support.emit("message", message) }

fun log(origin: Class<*>, type: Log, message: String) { oracle.Support.emit("log", type.name, message) }

fun spawnTimer(identifier: String, vararg args: Any): RSTimer? {
    oracle.Support.emit("spawn_timer", identifier, args.map { oracle.Support.value(it) })
    return oracle.Support.spawn(identifier, args.toList())
}

fun registerTimer(entity: Entity, timer: RSTimer?) {
    oracle.Support.emit("register_timer", oracle.Support.label(timer))
    if (timer != null) oracle.Support.timers.add(timer)
}

fun removeTimer(entity: Entity, identifier: String) {
    oracle.Support.emit("remove_timer", identifier)
    oracle.Support.timers.removeIf { it.identifier == identifier }
}

inline fun <reified T : RSTimer> getTimer(entity: Entity): T? {
    oracle.Support.emit("get_timer", T::class.java.simpleName)
    return oracle.Support.timers.firstOrNull { it is T } as T?
}

inline fun <reified T : RSTimer> removeTimer(entity: Entity) {
    oracle.Support.emit("remove_timer_class", T::class.java.simpleName)
    oracle.Support.timers.removeIf { it is T }
}
'''
