"""Stand-ins for everything the unchanged spell classes reach outside themselves.

Each entry is one source file compiled next to the unchanged MagicSpell,
CombatSpell, DefaultCombatSpell, SpellType, Runes, CombinationRune,
MagicStaff, SpellBlocks, SpellBookManager, SpellUtils, Audio and the combat
spell classes of the standard and ancient books. `/* ORIGINAL_* */` markers
are filled with original text at generation time (generate.py, `markers`).
Every recorded call goes through oracle.Support.emit, which also injects the
failures of the sweep. Stand-ins for paths the corpus does not drive throw.
"""

ADAPTERS = {}

ADAPTERS['core/tools/RandomFunction.java'] = '''package core.tools;
// The selected original method, verbatim; it draws Math.random, which the harness replaces.
public class RandomFunction {
    /* ORIGINAL_GET_RANDOM */
}
'''

ADAPTERS['core/plugin/Plugin.java'] = '''package core.plugin;
// The two abstract members of the original interface.
public interface Plugin<T> {
    Plugin<T> newInstance(T arg) throws Throwable;
    Object fireEvent(String identifier, Object... args);
}
'''

ADAPTERS['core/plugin/Initializable.java'] = '''package core.plugin;
public @interface Initializable {}
'''

ADAPTERS['core/game/world/GameWorld.java'] = '''package core.game.world;
// Recording tick and development-mode reads.
public final class GameWorld {
    public static final class Settings {
        public boolean isDevMode() { oracle.Support.emit("dev_mode"); return oracle.Support.devMode(); }
    }
    private static final Settings SETTINGS = new Settings();
    public static int getTicks() { oracle.Support.emit("ticks"); return oracle.Support.ticks(); }
    public static Settings getSettings() { return SETTINGS; }
}
'''

ADAPTERS['core/game/world/map/Location.kt'] = '''package core.game.world.map
import kotlin.math.*
// The original packed coordinates and withinDistance/getDistance, verbatim.
class Location(x: Int, y: Int, z: Int) {
    var hash = LocationHash(x, y, (z % 4 + 4) % 4)
    val x: Int get() = hash.x
    val y: Int get() = hash.y
    val z: Int get() = hash.z
    /* ORIGINAL_WITHIN_DISTANCE */
    companion object {
        @JvmStatic @JvmOverloads fun create(x: Int, y: Int, z: Int = 0): Location = Location(x, y, z)
        /* ORIGINAL_GET_DISTANCE */
    }
    @JvmInline /* ORIGINAL_LOCATION_HASH */
}
'''

ADAPTERS['core/game/world/update/flag/context/Graphics.java'] = '''package core.game.world.update.flag.context;
// The original constructor defaults; only the three fields are read here.
public class Graphics {
    public final int id, height, delay;
    public Graphics(int id) { this(id, 0, 0); }
    public Graphics(int id, int height) { this(id, height, 0); }
    public Graphics(int id, int height, int delay) { this.id = id; this.height = height; this.delay = delay; }
    public int getId() { return id; }
    public int getHeight() { return height; }
}
'''

ADAPTERS['core/game/world/update/flag/context/Animation.java'] = '''package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
// The original constructor defaults; only the three fields are read here.
public class Animation {
    public final int id, delay;
    public final Priority priority;
    public Animation(int id) { this(id, 0, Priority.MID); }
    public static Animation create(int id) { return new Animation(id, 0, Priority.MID); }
    public Animation(int id, Priority priority) { this(id, 0, priority); }
    public Animation(int id, int delay) { this(id, delay, Priority.MID); }
    public Animation(int id, int delay, Priority priority) { this.id = id; this.delay = delay; this.priority = priority; }
    public int getId() { return id; }
}
'''

ADAPTERS['core/game/node/entity/impl/Animator.java'] = '''package core.game.node.entity.impl;
import core.game.world.update.flag.context.Animation;
// The original Priority constants; forceAnimation is recorded.
public class Animator {
    private final core.game.node.entity.Entity entity;
    public Animator(core.game.node.entity.Entity entity) { this.entity = entity; }
    public static enum Priority { /* ORIGINAL_PRIORITIES */ }
    public boolean forceAnimation(Animation animation) {
        oracle.Support.emit("force_animation", oracle.Support.ix(entity), oracle.Support.anim(animation));
        return true;
    }
}
'''

ADAPTERS['core/game/node/entity/impl/Projectile.java'] = '''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
// The template's constructor fields; transform/send and magic are recorded. The label names the
// static field (or constructor) that holds the template, assigned by the harness after registration.
public class Projectile {
    public final int projectileId, startHeight, endHeight, startDelay, speed, angle, distance;
    public String label = "unlabelled";
    private Object[] transform;
    private Projectile(int projectileId, int startHeight, int endHeight, int startDelay, int speed, int angle, int distance) {
        this.projectileId = projectileId; this.startHeight = startHeight; this.endHeight = endHeight; this.startDelay = startDelay;
        this.speed = speed; this.angle = angle; this.distance = distance;
    }
    public static Projectile create(Entity source, Entity victim, int projectileId, int startHeight, int endHeight, int startDelay, int speed, int angle, int distance) {
        if (source != null || victim != null) throw new IllegalStateException("not driven");
        return new Projectile(projectileId, startHeight, endHeight, startDelay, speed, angle, distance);
    }
    public static Projectile magic(Entity source, Entity victim, int projectileId, int startHeight, int endHeight, int startDelay, int angle) {
        oracle.Support.emit("projectile_magic", oracle.Support.ix(source), oracle.Support.ix(victim), projectileId, startHeight, endHeight, startDelay, angle);
        Projectile p = new Projectile(projectileId, startHeight, endHeight, startDelay, -1, angle, -1);
        p.label = "magic";
        return p;
    }
    public Projectile transform(Entity source, Entity victim, boolean npc, int baseSpeed, int modifiedSpeed) {
        transform = new Object[] { oracle.Support.ix(source), oracle.Support.ix(victim), null, npc, baseSpeed, modifiedSpeed };
        return this;
    }
    public Projectile transform(Entity source, Location l, boolean npc, int baseSpeed, int modifiedSpeed) {
        transform = new Object[] { oracle.Support.ix(source), null, oracle.Support.coord(l), npc, baseSpeed, modifiedSpeed };
        return this;
    }
    public void send() {
        Object[] t = transform;
        oracle.Support.emit("projectile", label, t[0], t[1], t[2], t[3], t[4], t[5]);
    }
}
'''

ADAPTERS['core/game/node/Node.java'] = '''package core.game.node;
import core.game.node.entity.player.Player;
import core.game.node.entity.npc.NPC;
import core.game.world.map.Location;
// A node of the case: its name and location are recorded reads.
public abstract class Node {
    public String name;
    public Location location;
    public String getName() { oracle.Support.emit("name", oracle.Support.ix(this)); return name; }
    public Location getLocation() { oracle.Support.emit("location", oracle.Support.ix(this)); return location; }
    public Player asPlayer() { return (Player) this; }
    public NPC asNpc() { return (NPC) this; }
}
'''

ADAPTERS['oracle/TargetNode.java'] = '''package oracle;
// A node of the case that is not an entity (scenery, a ground tile).
public class TargetNode extends core.game.node.Node {}
'''

ADAPTERS['core/game/node/item/Item.java'] = '''package core.game.node.item;
import core.game.node.Node;
// Id and amount; the name is the definition name the case supplies (not a recorded call).
public class Item extends Node {
    private final int id;
    private final int amount;
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { this.id = id; this.amount = amount; }
    public int getId() { return id; }
    public int getAmount() { return amount; }
    @Override public String getName() { return oracle.Support.itemName(id); }
}
'''

ADAPTERS['core/cache/def/impl/ItemDefinition.java'] = '''package core.cache.def.impl;
// forId(id).getName(): the definition name the case supplies.
public class ItemDefinition {
    private final int id;
    private ItemDefinition(int id) { this.id = id; }
    public static ItemDefinition forId(int id) { return new ItemDefinition(id); }
    public String getName() { return oracle.Support.itemName(id); }
}
'''

ADAPTERS['core/game/container/Container.java'] = '''package core.game.container;
import core.game.node.item.Item;
// The inventory: recorded amount reads and removals over the case's slots.
public class Container {
    private final core.game.node.entity.Entity owner;
    public Container(core.game.node.entity.Entity owner) { this.owner = owner; }
    public int getAmount(int id) {
        oracle.Support.emit("inventory_amount", oracle.Support.ix(owner), id);
        return oracle.Support.inventoryAmount(owner, id);
    }
    public boolean remove(Item item) {
        oracle.Support.emit("inventory_remove", oracle.Support.ix(owner), item.getId(), item.getAmount());
        return oracle.Support.inventoryRemove(owner, item.getId(), item.getAmount());
    }
}
'''

ADAPTERS['core/game/container/impl/EquipmentContainer.java'] = '''package core.game.container.impl;
import core.game.node.item.Item;
// The worn items: recorded get/getNew reads; the slot constants are original text.
public class EquipmentContainer {
    /* ORIGINAL_SLOTS */
    private final core.game.node.entity.Entity owner;
    public EquipmentContainer(core.game.node.entity.Entity owner) { this.owner = owner; }
    public Item get(int slot) {
        oracle.Support.emit("equipment", oracle.Support.ix(owner), slot);
        Integer id = oracle.Support.worn(owner, slot);
        return id == null ? null : new Item(id);
    }
    public Item getNew(int slot) {
        oracle.Support.emit("equipment_new", oracle.Support.ix(owner), slot);
        Integer id = oracle.Support.worn(owner, slot);
        return id == null ? new Item(0) : new Item(id);
    }
}
'''

ADAPTERS['core/game/node/entity/skill/Skills.java'] = '''package core.game.node.entity.skill;
import core.game.node.entity.Entity;
// The original skill constants; every read and experience grant is recorded and answered from the case.
public final class Skills {
    /* ORIGINAL_SKILL_CONSTANTS */
    private final Entity entity;
    public Skills(Entity entity) { this.entity = entity; }
    public int getLevel(int slot) { oracle.Support.emit("level", oracle.Support.ix(entity), slot); return oracle.Support.level(entity, slot); }
    public int getLevel(int slot, boolean discardAssist) {
        oracle.Support.emit("level_with", oracle.Support.ix(entity), slot, discardAssist);
        return oracle.Support.level(entity, slot);
    }
    public int getStaticLevel(int slot) { throw new IllegalStateException("not driven"); }
    public void addExperience(int slot, double experience, boolean playerMod) {
        oracle.Support.emit("add_experience", oracle.Support.ix(entity), slot, oracle.Support.hex(experience), playerMod);
    }
    public void addExperience(int slot, double experience) {
        oracle.Support.emit("add_experience2", oracle.Support.ix(entity), slot, oracle.Support.hex(experience));
    }
    public void drainLevel(int skill, double drainPercentage, double maximumDrainPercentage) { throw new IllegalStateException("not driven"); }
    public int heal(int health) { throw new IllegalStateException("not driven"); }
    public void decrementPrayerPoints(double amount) { throw new IllegalStateException("not driven"); }
    public int updateLevel(int skill, int amount, int maximum) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/combat/CombatStyle.java'] = '''package core.game.node.entity.combat;
// MAGIC is the style the multi-target spells pass; getSwingHandler is only reached by an undriven effect.
public enum CombatStyle {
    MELEE, RANGE, MAGIC;
    public CombatSwingHandler getSwingHandler() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/combat/CombatSwingHandler.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
// The static EXPERIENCE_MOD the spell experience reads.
public abstract class CombatSwingHandler {
    public static double EXPERIENCE_MOD = 4.0;
    public boolean isAccurateImpact(Entity entity, Entity victim) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/combat/MultihitTargets.kt'] = '''@file:JvmName("MultihitTargetsKt")
package core.game.node.entity.combat
import core.game.node.entity.Entity
// Recording stand-in: the case lists the targets.
fun findMultihitTargets(target: Entity, attacker: Entity, combatStyle: CombatStyle): List<Entity> {
    oracle.Support.emit("multi_targets", oracle.Support.ix(target), oracle.Support.ix(attacker), combatStyle.name)
    return oracle.Support.multiTargets(target)
}
'''

ADAPTERS['core/game/node/entity/combat/BattleState.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.spell.CombatSpell;
// The fields the spell members read and write.
public class BattleState {
    public Entity entity, victim;
    public int estimatedHit;
    public boolean frozen;
    public CombatSpell spell;
    public CombatStyle style;
    public BattleState() {}
    public BattleState(Entity entity, Entity victim) { this.entity = entity; this.victim = victim; }
    public int getEstimatedHit() { return estimatedHit; }
    public void setEstimatedHit(int hit) { estimatedHit = hit; }
    public boolean isFrozen() { return frozen; }
    public void setFrozen(boolean frozen) { this.frozen = frozen; }
    public CombatSpell getSpell() { return spell; }
    public CombatStyle getStyle() { return style; }
}
'''

ADAPTERS['core/game/node/entity/combat/CombatPulse.java'] = '''package core.game.node.entity.combat;
import core.game.node.Node;
import core.game.node.entity.Entity;
// The members CombatSpell.cast calls; every call is recorded.
public class CombatPulse {
    private final Entity entity;
    public CombatPulse(Entity entity) { this.entity = entity; }
    public boolean isAttacking() { oracle.Support.emit("is_attacking", oracle.Support.ix(entity)); return oracle.Support.flag(entity, "attacking", false); }
    public void setVictim(Node victim) { oracle.Support.emit("set_victim", oracle.Support.ix(entity), oracle.Support.ix(victim)); }
    public void updateStyle() { oracle.Support.emit("update_style", oracle.Support.ix(entity)); }
    public void start() { oracle.Support.emit("pulse_start", oracle.Support.ix(entity)); }
    public void attack(Node victim) { oracle.Support.emit("pulse_attack", oracle.Support.ix(entity), oracle.Support.ix(victim)); }
}
'''

ADAPTERS['core/game/node/entity/combat/equipment/WeaponInterface.java'] = '''package core.game.node.entity.combat.equipment;
import core.game.node.entity.combat.spell.CombatSpell;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.SpellBookManager;
import org.rs09.consts.Components;
import static core.api.ContentAPIKt.*;
// The autocast members of the original class: the four spell tables, the style constants, the attribute keys and
// getAutospellId, selectAutoSpell and canAutocast are original text. The weapon interface kind is the case's;
// setAttackStyle is recorded.
public final class WeaponInterface {
    /* ORIGINAL_SPELL_TABLES */
    /* ORIGINAL_STYLE_CONSTANTS */
    /* ORIGINAL_AUTOCAST_KEYS */
    public enum WeaponInterfaces { STAFF, OTHER }
    public static class AttackStyle {
        private final int style;
        public AttackStyle(int style) { this.style = style; }
        public int getStyle() { return style; }
    }
    private final Player player;
    private final WeaponInterfaces current;
    public WeaponInterface(Player player, boolean staff) { this.player = player; this.current = staff ? WeaponInterfaces.STAFF : WeaponInterfaces.OTHER; }
    public boolean setAttackStyle(int button) { oracle.Support.emit("set_attack_style", oracle.Support.ix(player), button); return true; }

    /* ORIGINAL_AUTOCAST_METHODS */
}
'''

ADAPTERS['core/game/node/entity/Properties.java'] = '''package core.game.node.entity;
import core.game.node.entity.combat.CombatPulse;
import core.game.node.entity.combat.equipment.WeaponInterface;
import core.game.node.entity.combat.spell.CombatSpell;
import core.game.world.update.flag.context.Animation;
// Recording property reads and writes; the autocast spell is the case's field.
public class Properties {
    private final Entity entity;
    public CombatSpell autocastSpell;
    private final CombatPulse combatPulse;
    public Properties(Entity entity) { this.entity = entity; this.combatPulse = new CombatPulse(entity); }
    public CombatSpell getAutocastSpell() { oracle.Support.emit("autocast", oracle.Support.ix(entity)); return autocastSpell; }
    public void setAutocastSpell(CombatSpell spell) { oracle.Support.emit("set_autocast", oracle.Support.ix(entity), oracle.Support.label(spell)); autocastSpell = spell; }
    public void setSpell(CombatSpell spell) { oracle.Support.emit("set_spell", oracle.Support.ix(entity), oracle.Support.label(spell)); }
    public CombatPulse getCombatPulse() { return combatPulse; }
    public WeaponInterface.AttackStyle getAttackStyle() {
        oracle.Support.emit("attack_style", oracle.Support.ix(entity));
        Integer s = oracle.Support.attackStyle(entity);
        return s == null ? null : new WeaponInterface.AttackStyle(s);
    }
    public Animation getMagicAnimation() { oracle.Support.emit("magic_animation", oracle.Support.ix(entity)); return oracle.Support.magicAnimation(entity); }
    public Animation getAttackAnimation() { oracle.Support.emit("attack_animation", oracle.Support.ix(entity)); return oracle.Support.attackAnimation(entity); }
    public boolean isMultiZone() { oracle.Support.emit("multi_zone", oracle.Support.ix(entity)); return oracle.Support.flag(entity, "multi", false); }
    public int[] getBonuses() { oracle.Support.emit("bonuses", oracle.Support.ix(entity)); return oracle.Support.bonuses(entity); }
}
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.node.Node;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
// Recording Entity members; attributes are the case's map with GameAttributes' /save: key handling, each call recorded.
public abstract class Entity extends Node {
    private final Properties properties = new Properties(this);
    private final Skills skills = new Skills(this);
    private final Animator animator = new Animator(this);
    public final java.util.Map<String, Object> attributes = new java.util.HashMap<>();
    public Properties getProperties() { return properties; }
    public Skills getSkills() { return skills; }
    public Animator getAnimator() { return animator; }
    public int size() { throw new IllegalStateException("not driven"); }
    public void faceLocation(Location l) { oracle.Support.emit("face_location", oracle.Support.ix(this), oracle.Support.coord(l)); }
    public void faceTemporary(Entity e, int ticks) { oracle.Support.emit("face_temporary", oracle.Support.ix(this), oracle.Support.ix(e), ticks); }
    public void face(Entity e) { oracle.Support.emit("face", oracle.Support.ix(this), oracle.Support.ix(e)); }
    public boolean graphics(Graphics g) { oracle.Support.emit("graphics", oracle.Support.ix(this), oracle.Support.gfx(g)); return true; }
    public boolean animate(Animation a) { oracle.Support.emit("animate", oracle.Support.ix(this), oracle.Support.anim(a)); return true; }
    public void setAttribute(String key, Object value) {
        oracle.Support.emit("set_attribute", oracle.Support.ix(this), key, oracle.Support.value(value));
        attributes.put(key.startsWith("/save:") ? key.substring(6) : key, value);
    }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        oracle.Support.emit("get_attribute", oracle.Support.ix(this), key, oracle.Support.value(fail));
        Object v = attributes.get(key.replace("/save:", ""));
        return v == null ? fail : (T) v;
    }
    public void removeAttribute(String key) { oracle.Support.emit("remove_attribute", oracle.Support.ix(this), key); attributes.remove(key); }
    public core.game.world.map.zone.ZoneMonitor getZoneMonitor() { return new core.game.world.map.zone.ZoneMonitor(this); }
    public Locks getLocks() { throw new IllegalStateException("not driven"); }
    public WalkingQueue getWalkingQueue() { throw new IllegalStateException("not driven"); }
    public static class Locks { public boolean isMovementLocked() { return false; } public void lockMovement(int ticks) {} }
    public static class WalkingQueue { public void reset() {} }
}
'''

ADAPTERS['core/game/world/map/zone/ZoneMonitor.java'] = '''package core.game.world.map.zone;
import core.game.node.entity.Entity;
// isInZone is recorded and answered from the case.
public class ZoneMonitor {
    private final Entity entity;
    public ZoneMonitor(Entity entity) { this.entity = entity; }
    public boolean isInZone(String name) { oracle.Support.emit("in_zone", oracle.Support.ix(entity), name); return oracle.Support.inZone(entity, name); }
}
'''

ADAPTERS['core/game/node/entity/npc/NPC.java'] = '''package core.game.node.entity.npc;
import core.game.node.entity.Entity;
// Recording NPC reads: id, slayer task undead flag, definition combat graphics.
public class NPC extends Entity {
    public int id;
    public int getId() { oracle.Support.emit("npc_id", oracle.Support.ix(this)); return id; }
    public Task getTask() { oracle.Support.emit("npc_task", oracle.Support.ix(this)); return oracle.Support.task(this); }
    public Definition getDefinition() { return new Definition(this); }
    public static class Task {
        public final boolean undead;
        public Task(boolean undead) { this.undead = undead; }
    }
    public static class Definition {
        private final NPC npc;
        Definition(NPC npc) { this.npc = npc; }
        public core.game.world.update.flag.context.Graphics[] getCombatGraphics() {
            oracle.Support.emit("combat_graphics", oracle.Support.ix(npc));
            return oracle.Support.combatGraphics(npc);
        }
    }
}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
import core.game.container.Container;
import core.game.container.impl.EquipmentContainer;
import core.game.event.Event;
import core.game.node.entity.Entity;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.SpellBookManager;
// The original SpellBookManager field; everything else is recorded and answered from the case.
public class Player extends Entity {
    private final SpellBookManager spellBookManager = new SpellBookManager();
    private final PacketDispatch packetDispatch = new PacketDispatch(this);
    private final EquipmentContainer equipment = new EquipmentContainer(this);
    private final Container inventory = new Container(this);
    public SpellBookManager getSpellBookManager() { return spellBookManager; }
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public EquipmentContainer getEquipment() { return equipment; }
    public Container getInventory() { return inventory; }
    public void sendMessage(String message) { oracle.Support.emit("send_message", oracle.Support.ix(this), message); }
    public void dispatch(Event event) { oracle.Support.event(this, event); }
    public SavedData getSavedData() { return new SavedData(this); }
    public InterfaceManager getInterfaceManager() { return new InterfaceManager(this); }
    public Settings getSettings() { return new Settings(this); }
    public Prayer getPrayer() { throw new IllegalStateException("not driven"); }
    public static class Prayer { public boolean get(core.game.node.entity.player.link.prayer.PrayerType type) { return false; } }
    public static class SavedData {
        private final Player player;
        SavedData(Player player) { this.player = player; }
        public SavedData getActivityData() { return this; }
        public int[] getGodCasts() { oracle.Support.emit("god_casts", oracle.Support.ix(player)); return oracle.Support.godCasts(player); }
    }
    public static class InterfaceManager {
        private final Player player;
        InterfaceManager(Player player) { this.player = player; }
        public void openTab(core.game.component.Component component) { oracle.Support.emit("open_tab", oracle.Support.ix(player), component.id); }
    }
    public static class Settings {
        private final Player player;
        Settings(Player player) { this.player = player; }
        public int getAttackStyleIndex() { oracle.Support.emit("attack_style_index", oracle.Support.ix(player)); return oracle.Support.styleIndex(player); }
    }
}
'''

ADAPTERS['core/game/node/entity/player/link/PacketDispatch.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
// Messages and interface configs are recorded.
public class PacketDispatch {
    private final Player player;
    public PacketDispatch(Player player) { this.player = player; }
    public void sendMessage(String message) { oracle.Support.emit("message", oracle.Support.ix(player), message); }
    public void sendInterfaceConfig(int interfaceId, int childId, boolean hide) {
        oracle.Support.emit("interface_config", oracle.Support.ix(player), interfaceId, childId, hide);
    }
}
'''

ADAPTERS['core/game/component/Component.java'] = '''package core.game.component;
public class Component {
    public final int id;
    public Component(int id) { this.id = id; }
}
'''

ADAPTERS['core/game/event/Events.kt'] = '''package core.game.event
import core.game.node.Node
import core.game.node.entity.player.link.SpellBookManager.SpellBook
interface Event
/* ORIGINAL_SPELL_CAST_EVENT */
'''

ADAPTERS['core/game/node/entity/player/link/prayer/PrayerType.java'] = '''package core.game.node.entity.player.link.prayer;
// Imported by TeleblockSpell's undriven effect.
public enum PrayerType { PROTECT_FROM_MAGIC }
'''

ADAPTERS['content/region/kandarin/feldip/quest/zogreflesheaters/SkogreBehavior.kt'] = '''package content.region.kandarin.feldip.quest.zogreflesheaters
import org.rs09.consts.NPCs
// The original companion object, verbatim.
class SkogreBehavior {
    /* ORIGINAL_SKOGRE_IDS */
}
'''

ADAPTERS['content/region/kandarin/feldip/quest/zogreflesheaters/ZogreBehavior.kt'] = '''package content.region.kandarin.feldip.quest.zogreflesheaters
import org.rs09.consts.NPCs
// The original companion object, verbatim.
class ZogreBehavior {
    /* ORIGINAL_ZOGRE_IDS */
}
'''

ADAPTERS['core/api/ContentAPI.kt'] = '''@file:JvmName("ContentAPIKt")
package core.api
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.audio.Audio
import core.game.world.map.Location
// getAttribute, setAttribute, removeAttribute and removeAttributes are original text; the rest record and answer
// from the case. Timers, poison and RSTimer are only reached by undriven spell effects.
/* ORIGINAL_ATTRIBUTE_FUNCTIONS */

@JvmOverloads
fun playGlobalAudio(location: Location, id: Int, delay: Int = 0, loops: Int = 1, radius: Int = Audio.defaultAudioRadius) {
    oracle.Support.emit("global_audio", oracle.Support.coord(location), id, delay, loops, radius)
}

fun hasTimerActive(entity: Entity, identifier: String): Boolean {
    oracle.Support.emit("timer_active", oracle.Support.ix(entity), identifier)
    return oracle.Support.timer(entity, identifier)
}

@JvmOverloads
fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("set_varbit", oracle.Support.ix(player), varbitId, value, save)
}

fun setComponentVisibility(player: Player, iface: Int, child: Int, hide: Boolean) {
    oracle.Support.emit("component_visibility", oracle.Support.ix(player), iface, child, hide)
}

class RSTimer
fun registerTimer(entity: Entity, timer: RSTimer?) { throw IllegalStateException("not driven") }
fun spawnTimer(identifier: String, vararg args: Any): RSTimer? { throw IllegalStateException("not driven") }
fun applyPoison(entity: Entity, source: Entity, severity: Int) { throw IllegalStateException("not driven") }
'''
