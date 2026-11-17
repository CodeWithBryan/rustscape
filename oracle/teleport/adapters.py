"""Stand-ins for everything the unchanged teleport classes reach outside themselves.

Compiled unchanged next to these: TeleportManager.java, TeleportPulse.java,
Pulse.java, Animator.java, Animation.java, Graphics.java, EntityFlag.kt,
Clocks.kt, ActionLocks.java, Lock.java, LockElapse.java, WalkingQueue.java,
Point.java, Direction.java, ClipMaskSupplier.java, ZoneRestriction.java and
Audio.kt. `/* ORIGINAL_* */` markers are filled with original text at
generation time (generate.py, `markers`). Every recorded call goes through
oracle.Support.emit, which also injects the failures of the sweep and fires
the case's hooks.
"""

ADAPTERS = {}

ADAPTERS['core/ServerConstants.java'] = '''package core;
// Imported by TeleportManager, never used.
public final class ServerConstants {}
'''

ADAPTERS['core/tools/Log.java'] = '''package core.tools;
public enum Log { FINE, INFO, WARN, ERR }
'''

ADAPTERS['core/game/world/GameWorld.java'] = '''package core.game.world;
import core.game.system.task.Pulse;
// getTicks records each read of the world clock; the pulser records the submission and queues the pulse for the harness runner.
public final class GameWorld {
    public static int getTicks() { return oracle.Support.ticks(); }
    public static final class Pulser {
        public void submit(Pulse pulse) { oracle.Support.submit(pulse); }
    }
    private static final Pulser PULSER = new Pulser();
    public static Pulser getPulser() { return PULSER; }
}
'''

ADAPTERS['core/game/node/Node.java'] = '''package core.game.node;
import core.game.node.item.Item;
// The two Node members reached here: Pulse.hasInactiveNode and ZoneMonitor's asItem.
public abstract class Node {
    public boolean isActive() { return true; }
    public Item asItem() { return (Item) this; }
}
'''

ADAPTERS['core/game/node/item/Item.java'] = '''package core.game.node.item;
public class Item extends core.game.node.Node {
    private final int id;
    public Item(int id) { this.id = id; }
    public int getId() { return id; }
}
'''

ADAPTERS['core/game/node/item/GroundItem.java'] = '''package core.game.node.item;
import core.game.world.map.Location;
// Route-drawing items of WalkingQueue; never created by these cases.
public class GroundItem {
    public GroundItem(Item item, Location location, core.game.node.entity.player.Player player) { throw new IllegalStateException("route items are not driven"); }
    public Location getLocation() { return null; }
}
'''

ADAPTERS['core/game/node/scenery/Scenery.java'] = '''package core.game.node.scenery;
// Animation's object field type.
public class Scenery {}
'''

ADAPTERS['core/cache/def/impl/AnimationDefinition.java'] = '''package core.cache.def.impl;
// forId records the lookup and answers from the case (no cache); getDuration and getDurationTicks are original text.
public class AnimationDefinition {
    public int[] durations;
    public static final AnimationDefinition forId(int emoteId) { return oracle.Support.definition(emoteId); }
    /* ORIGINAL_DEFINITION_DURATIONS */
}
'''

ADAPTERS['core/game/world/update/UpdateMasks.java'] = '''package core.game.world.update;
import core.game.node.entity.Entity;
import core.game.world.update.flag.EntityFlag;
// register records the flag and its context; the result is always true (Animator ignores it).
public class UpdateMasks {
    private final Entity entity;
    public UpdateMasks(Entity entity) { this.entity = entity; }
    public boolean register(EntityFlag flag, Object context) { oracle.Support.register(entity, flag, context); return true; }
}
'''

ADAPTERS['core/game/world/update/flag/chunk/GraphicUpdateFlag.java'] = '''package core.game.world.update.flag.chunk;
// Graphics.send is never called here.
public class GraphicUpdateFlag {
    public GraphicUpdateFlag(core.game.world.update.flag.context.Graphics graphic, core.game.world.map.Location location) {}
}
'''

ADAPTERS['core/game/world/map/RegionManager.java'] = '''package core.game.world.map;
import core.game.node.entity.Entity;
import core.game.node.item.GroundItem;
// move records the chunk move (a service) and then runs the original's last statement,
// entity.zoneMonitor.updateLocation(entity.walkingQueue.footPrint), whose zone walk is a recorded service here.
public final class RegionManager {
    public static void move(Entity entity, Location src, Location dst) {
        oracle.Support.emit("region_move", oracle.Support.ix(entity), oracle.Support.coord(src), oracle.Support.coord(dst));
        entity.getZoneMonitor().updateLocation(entity.getWalkingQueue().getFootPrint());
    }
    public static Chunk getRegionChunk(Location l) { throw new IllegalStateException("chunks are not driven"); }
    public static int getClippingFlag(int z, int x, int y) { return 0; }
    public static class Chunk {
        public void add(GroundItem i) {}
        public void remove(GroundItem i) {}
        public void flag(core.game.world.update.flag.chunk.GraphicUpdateFlag f) {}
    }
}
'''

ADAPTERS['core/game/world/map/Location.kt'] = '''package core.game.world.map
// The original packed coordinates, constructors, setters, region getters and the static create overloads, verbatim.
/* ORIGINAL_LOCATION_HEADER */
    /* ORIGINAL_LOCATION_COORDINATES */
    /* ORIGINAL_LOCATION_REGION */
    /* ORIGINAL_LOCATION_TRANSFORM */
    companion object {
        /* ORIGINAL_LOCATION_CREATE */
    }
    @JvmInline /* ORIGINAL_LOCATION_HASH */
}
'''

ADAPTERS['content/global/skill/skillcapeperks/SkillcapePerks.java'] = '''package content.global.skill.skillcapeperks;
// WalkingQueue's marathon perk; teleports never run, so it is never asked.
public enum SkillcapePerks {
    MARATHON_RUNNER;
    public static boolean isActive(SkillcapePerks perk, core.game.node.entity.player.Player player) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/skill/Skills.java'] = '''package core.game.node.entity.skill;
public final class Skills { public static final int AGILITY = 16; }
'''

ADAPTERS['core/game/interaction/ScriptProcessor.java'] = '''package core.game.interaction;
import core.game.node.entity.Entity;
// setDelay records the write; the delay is the processor field.
public class ScriptProcessor {
    private final Entity entity;
    public int delay;
    public ScriptProcessor(Entity entity) { this.entity = entity; }
    public void setDelay(int delay) { oracle.Support.emit("script_delay", oracle.Support.ix(entity), delay); this.delay = delay; }
    public int getDelay() { return delay; }
}
'''

ADAPTERS['core/game/system/timer/RSTimer.java'] = '''package core.game.system.timer;
public class RSTimer {}
'''

ADAPTERS['core/game/system/timer/TimerManager.java'] = '''package core.game.system.timer;
import core.game.node.entity.Entity;
// getTimer records the lookup and answers from the case's active timer names.
public class TimerManager {
    private final Entity entity;
    public TimerManager(Entity entity) { this.entity = entity; }
    public RSTimer getTimer(String identifier) { return oracle.Support.timer(entity, identifier); }
}
'''

ADAPTERS['core/game/node/entity/impl/GameAttributes.java'] = '''package core.game.node.entity.impl;
import java.util.*;
// The original attribute members, verbatim, over a map that records every get, put and remove
// (WalkingQueue's "routedraw" read is answered without a record: the native walking queue keeps it as a field).
public final class GameAttributes {
    private final Map<String, Object> attributes;
    private final List<String> savedAttributes = new ArrayList<>(250);
    public GameAttributes(core.game.node.entity.Entity entity) { attributes = new oracle.Support.Attributes(entity); }
    public Map<String, Object> getAttributes() { return attributes; }
    /* ORIGINAL_GAME_ATTRIBUTES */
}
'''

ADAPTERS['core/game/node/entity/impl/Properties.java'] = '''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
// The teleport members of Properties, verbatim.
public class Properties {
    private final Entity entity;
    private boolean teleporting;
    private Location teleportLocation;
    public Properties(Entity entity) { this.entity = entity; }
    /* ORIGINAL_PROPERTIES */
}
'''

ADAPTERS['core/game/node/entity/impl/PulseManager.java'] = '''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
// clear() is a recorded service here; the pulses corpus verifies its body.
public final class PulseManager {
    private final Entity entity;
    public PulseManager(Entity entity) { this.entity = entity; }
    public void clear() { oracle.Support.emit("clear_pulses", oracle.Support.ix(entity)); }
}
'''

ADAPTERS['core/game/node/entity/combat/ImpactHandler.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import core.game.world.GameWorld;
// The disabled-ticks field and its original setter.
public class ImpactHandler {
    private final Entity entity;
    private int disabledTicks;
    public ImpactHandler(Entity entity) { this.entity = entity; }
    public int getDisabledTicks() { return disabledTicks; }
    /* ORIGINAL_SET_DISABLED_TICKS */
}
'''

ADAPTERS['core/game/world/map/zone/MapZone.java'] = '''package core.game.world.map.zone;
import core.game.node.Node;
import core.game.node.entity.Entity;
// The restriction field with the original isRestricted; teleport records and answers from the case.
public class MapZone {
    public final int index;
    private int restriction;
    public MapZone(int index, int restriction) { this.index = index; this.restriction = restriction; }
    /* ORIGINAL_MAP_ZONE_RESTRICTED */
    public boolean teleport(Entity e, int type, Node node) { return oracle.Support.zoneTeleport(this, e, type, node); }
}
'''

ADAPTERS['core/game/world/map/zone/RegionZone.java'] = '''package core.game.world.map.zone;
public final class RegionZone {
    private final MapZone zone;
    public RegionZone(MapZone zone) { this.zone = zone; }
    public MapZone getZone() { return zone; }
}
'''

ADAPTERS['core/game/world/map/zone/ZoneMonitor.java'] = '''package core.game.world.map.zone;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import java.util.*;
import org.rs09.consts.Items;
// The jewellery set, isRestricted, teleport and canTeleportByJewellery are original text over the live zone list;
// updateLocation and move are recorded services (the zones corpus verifies their bodies).
public final class ZoneMonitor {
    /* ORIGINAL_JEWELLERY */
    private final Entity entity;
    private final List<RegionZone> zones = new ArrayList<>(20);
    public ZoneMonitor(Entity entity) { this.entity = entity; }
    public List<RegionZone> getZones() { return zones; }
    /* ORIGINAL_ZONE_MONITOR */
    public boolean updateLocation(Location last) { return oracle.Support.zoneUpdate(entity, last); }
    public boolean move(Location location, Location destination) { throw new IllegalStateException("walking steps are not driven"); }
}
'''

ADAPTERS['core/game/dialogue/DialogueInterpreter.java'] = '''package core.game.dialogue;
import core.game.node.entity.player.Player;
public final class DialogueInterpreter {
    private final Player player;
    public DialogueInterpreter(Player player) { this.player = player; }
    public boolean close() { oracle.Support.emit("close_dialogue", oracle.Support.ix(player)); return true; }
}
'''

ADAPTERS['core/game/node/entity/player/link/InterfaceManager.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
public final class InterfaceManager {
    private final Player player;
    public InterfaceManager(Player player) { this.player = player; }
    public boolean close() { oracle.Support.emit("close_interfaces", oracle.Support.ix(player)); return true; }
}
'''

ADAPTERS['core/game/node/entity/player/link/SpellBookManager.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
public final class SpellBookManager {
    private final Player player;
    public SpellBookManager(Player player) { this.player = player; }
    public int getSpellBook() { oracle.Support.emit("spell_book", oracle.Support.ix(player)); return oracle.Support.number(player, "spellbook", 192); }
}
'''

ADAPTERS['core/game/node/entity/player/link/SkullManager.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
public final class SkullManager {
    private final Player player;
    public SkullManager(Player player) { this.player = player; }
    public int getLevel() { oracle.Support.emit("skull_level", oracle.Support.ix(player)); return oracle.Support.number(player, "skull", 0); }
}
'''

ADAPTERS['core/game/event/Events.kt'] = '''package core.game.event
import core.game.node.entity.Entity
// The event interface and the two attribute events, verbatim.
interface Event {}
/* ORIGINAL_ATTRIBUTE_EVENTS */
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.event.AttributeRemoveEvent;
import core.game.event.AttributeSetEvent;
import core.game.event.Event;
import core.game.interaction.ScriptProcessor;
import core.game.node.Node;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.*;
import core.game.node.entity.impl.Properties;
import core.game.node.entity.lock.ActionLocks;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.TeleportManager;
import core.game.system.timer.TimerManager;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneMonitor;
import core.game.world.map.zone.ZoneRestriction;
import core.game.world.update.UpdateMasks;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
// The fields TeleportManager reaches, with the original Entity members inserted verbatim; dispatch records the event.
public abstract class Entity extends Node {
    public Location location;
    public Direction direction;
    public ScriptProcessor scripts = new ScriptProcessor(this);
    public final int[] clocks = new int[10];
    public TimerManager timers = new TimerManager(this);
    private final GameAttributes attributes = new GameAttributes(this);
    private final Properties properties = new Properties(this);
    private final ActionLocks locks = new ActionLocks();
    private final Animator animator = new Animator(this);
    private final UpdateMasks updateMasks = new UpdateMasks(this);
    private final ImpactHandler impactHandler = new ImpactHandler(this);
    private final PulseManager pulseManager = new PulseManager(this);
    private final ZoneMonitor zoneMonitor = new ZoneMonitor(this);
    private final TeleportManager teleporter = new TeleportManager(this);
    private WalkingQueue walkingQueue;
    public void init() { walkingQueue = new WalkingQueue(this); }
    public Location getLocation() { return location; }
    public void setLocation(Location location) { this.location = location; }
    public Direction getDirection() { return direction; }
    public void setDirection(Direction direction) { if (direction != null) this.direction = direction; }
    public GameAttributes getAttributes() { return attributes; }
    public Properties getProperties() { return properties; }
    public ActionLocks getLocks() { return locks; }
    public Animator getAnimator() { return animator; }
    public UpdateMasks getUpdateMasks() { return updateMasks; }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public PulseManager getPulseManager() { return pulseManager; }
    public ZoneMonitor getZoneMonitor() { return zoneMonitor; }
    public TeleportManager getTeleporter() { return teleporter; }
    public WalkingQueue getWalkingQueue() { return walkingQueue; }
    public void incrementAttribute(String key) { throw new IllegalStateException("not driven"); }
    public void dispatch(Event event) { oracle.Support.dispatch(this, event); }
    /* ORIGINAL_ENTITY_METHODS */
}
'''

ADAPTERS['core/game/node/entity/npc/NPC.java'] = '''package core.game.node.entity.npc;
public class NPC extends core.game.node.entity.Entity {}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
import core.game.dialogue.DialogueInterpreter;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.SkullManager;
import core.game.node.entity.player.link.SpellBookManager;
import core.game.world.map.Location;
// The player members TeleportManager and WalkingQueue reach; sendMessage records.
public class Player extends core.game.node.entity.Entity {
    private final DialogueInterpreter dialogueInterpreter = new DialogueInterpreter(this);
    private final InterfaceManager interfaceManager = new InterfaceManager(this);
    private final SpellBookManager spellBookManager = new SpellBookManager(this);
    private final SkullManager skullManager = new SkullManager(this);
    private final Settings settings = new Settings();
    private final Flags flags = new Flags();
    public DialogueInterpreter getDialogueInterpreter() { return dialogueInterpreter; }
    public InterfaceManager getInterfaceManager() { return interfaceManager; }
    public SpellBookManager getSpellBookManager() { return spellBookManager; }
    public SkullManager getSkullManager() { return skullManager; }
    public Settings getSettings() { return settings; }
    public Flags getPlayerFlags() { return flags; }
    public void sendMessage(String message) { oracle.Support.emit("message", oracle.Support.ix(this), message); }
    public static class Settings {
        public double getRunEnergy() { return 100.0; }
        public double getWeight() { return 0.0; }
        public boolean isRunToggled() { return false; }
        public void setRunToggled(boolean value) { throw new IllegalStateException("not driven"); }
        public void updateRunEnergy(double drain) { throw new IllegalStateException("not driven"); }
    }
    public static class Flags {
        public Location last;
        public boolean update;
        public Location getLastSceneGraph() { return last; }
        public void setUpdateSceneGraph(boolean value) { update = value; }
        public boolean isUpdateSceneGraph() { return update; }
    }
    public Skills getSkills() { throw new IllegalStateException("not driven"); }
    public class Skills { public int getLevel(int id) { return 1; } }
}
'''

ADAPTERS['core/api/ContentAPI.kt'] = '''package core.api
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.audio.Audio
import core.game.system.timer.RSTimer
import core.game.world.map.Location
import core.tools.Log
// hasTimerActive and getTimer are original text; the audio calls keep the original signatures and record their arguments.
/* ORIGINAL_HAS_TIMER_ACTIVE */

/* ORIGINAL_GET_TIMER */

/* ORIGINAL_PLAY_AUDIO_HEADER */
    oracle.Support.emit("audio", oracle.Support.ix(player), id, delay, loops, oracle.Support.coord(location), radius)
}

/* ORIGINAL_PLAY_GLOBAL_AUDIO_HEADER */
    oracle.Support.emit("global_audio", oracle.Support.coord(location), id, delay, loops, radius)
}

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", message)
}
'''
