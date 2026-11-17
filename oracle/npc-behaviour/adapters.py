"""Recording stand-ins for the npc-behaviour oracle.

Every class here is an adapter, not the original. Markers of the form
/* ORIGINAL_NAME */ are replaced at generation time with original text taken
from the pinned sources (see generate.py `markers`). Calls that stand for an
outside system record themselves through oracle.Support.emit; the answers come
from the case. Plain field getters are not recorded.
"""

E = 'oracle.Support'

ADAPTERS = {}

ADAPTERS['core/ServerConstants.java'] = '''package core;
// MAX_PLAYERS carries the pinned default value.
public class ServerConstants { /* ORIGINAL_MAX_PLAYERS */ }
'''

ADAPTERS['core/tools/RandomFunction.java'] = '''package core.tools;
// Selected original methods, verbatim; RANDOM is assignable so a case can seed it.
public class RandomFunction { public static java.util.Random RANDOM;
/* ORIGINAL_RANDOM_METHODS */
}
'''

ADAPTERS['core/game/world/GameWorld.java'] = '''package core.game.world;
import core.game.interaction.MovementPulse;
public class GameWorld {
    public static int getTicks() { return oracle.Support.ticks(); }
    public static class Pulser { public void submit(MovementPulse p) { oracle.Support.emit("submit_return", p.id); } }
    public static Pulser getPulser() { return new Pulser(); }
    public static class Settings { public boolean getWild_pvp_enabled() { oracle.Support.emit("wild_pvp_enabled"); return Boolean.TRUE.equals(oracle.Support.answer("wild_pvp_enabled", false)); } }
    public static Settings getSettings() { oracle.Support.emit("settings"); return Boolean.TRUE.equals(oracle.Support.answer("settings", true)) ? new Settings() : null; }
}
'''

ADAPTERS['core/game/world/repository/Repository.java'] = '''package core.game.world.repository;
import core.game.node.entity.npc.NPC;
public class Repository { public static void removeRenderableNPC(NPC n) { oracle.Support.emit("remove_renderable", oracle.Support.ix(n)); } }
'''

ADAPTERS['core/game/world/map/RegionChunk.java'] = '''package core.game.world.map;
import core.game.node.entity.npc.NPC;
public class RegionChunk {
    final Location location;
    public RegionChunk(Location location) { this.location = location; }
    public void remove(NPC n) { oracle.Support.emit("chunk_remove", oracle.Support.ix(n), oracle.Support.coord(location)); }
    public void add(NPC n) { oracle.Support.emit("chunk_add", oracle.Support.ix(n), oracle.Support.coord(location)); }
}
'''

ADAPTERS['core/game/world/map/Region.java'] = '''package core.game.world.map;
import core.game.node.entity.player.Player;
import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.TimeUnit;
// isTolerated is the original text; its clock is the case clock.
public class Region {
    static class System { static long currentTimeMillis() { return oracle.Support.millis(); } }
    public final int id;
    private final Map<String, Long> tolerances = new HashMap<>();
    public Region(int id) { this.id = id; }
    public boolean isActive() { oracle.Support.emit("region_active", id); return Boolean.TRUE.equals(oracle.Support.answer("region_active:" + id, true)); }
    public Map<String, Long> getTolerances() { return tolerances; }
    /* ORIGINAL_IS_TOLERATED */
}
'''

ADAPTERS['core/game/world/map/build/DynamicRegion.java'] = '''package core.game.world.map.build;
public class DynamicRegion extends core.game.world.map.Region { public DynamicRegion(int id) { super(id); } }
'''

ADAPTERS['core/game/world/map/RegionManager.java'] = '''package core.game.world.map;
import core.game.node.entity.player.Player;
import java.util.List;
public class RegionManager {
    public static Region forId(int id) { oracle.Support.emit("region_for_id", id); return oracle.Support.region(id); }
    public static List<Player> getLocalPlayers(Location l, int radius) { oracle.Support.emit("local_players", oracle.Support.coord(l), radius); return oracle.Support.localPlayers(); }
    public static int getClippingFlag(int z, int x, int y) { oracle.Support.emit("clipping_flag", z, x, y); return ((Number) oracle.Support.answer("clipping_flag", 0L)).intValue(); }
}
'''

ADAPTERS['core/game/world/map/zone/impl/WildernessZone.java'] = '''package core.game.world.map.zone.impl;
import core.game.node.entity.Entity;
public class WildernessZone { public static boolean isInZone(Entity e) { return oracle.Support.bool("wilderness_in_zone", e, false); } }
'''

ADAPTERS['core/game/world/map/path/Pathfinder.java'] = '''package core.game.world.map.path;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
public class Pathfinder {
    public static final Pathfinder SMART = new Pathfinder(), DUMB = new Pathfinder();
    // The safespot path: find and walk are separate calls.
    public static Path find(Entity mover, Location destination) { oracle.Support.emit("path_find", oracle.Support.ix(mover), oracle.Support.coord(destination)); return new Path(null, false); }
    // Wandering: the tick kernel's walk(location, water) is one call, made when the path walks.
    public static Path find(Entity mover, Location destination, boolean near, Pathfinder finder) { return new Path(destination, false); }
    public static Path findWater(Entity mover, Location destination, boolean near, Pathfinder finder) { return new Path(destination, true); }
}
'''

ADAPTERS['core/game/world/map/path/Path.java'] = '''package core.game.world.map.path;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
public class Path {
    final Location wander; final boolean water;
    Path(Location wander, boolean water) { this.wander = wander; this.water = water; }
    public void walk(Entity mover) {
        if (wander == null) oracle.Support.emit("path_walk", oracle.Support.ix(mover));
        else oracle.Support.emit("wander", oracle.Support.ix(mover), oracle.Support.coord(wander), water);
    }
}
'''

ADAPTERS['core/game/interaction/MovementPulse.java'] = '''package core.game.interaction;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
import core.game.world.map.path.Pathfinder;
public class MovementPulse {
    public final int id; final Entity mover;
    public MovementPulse(Entity mover, Location destination, Pathfinder finder) { this.mover = mover; id = ++oracle.Support.nextPulse; oracle.Support.emit("construct_return", id, oracle.Support.coord(destination)); }
    public MovementPulse(Entity mover, int id) { this.mover = mover; this.id = id; }
    public boolean pulse() { return true; }
    public boolean isRunning() { oracle.Support.emit("return_running", id); return Boolean.TRUE.equals(oracle.Support.answer("return_running:" + id, true)); }
    public void stop() { oracle.Support.emit("pulse_stop", oracle.Support.ix(mover), id); }
}
'''

ADAPTERS['core/game/node/item/Item.java'] = '''package core.game.node.item;
public class Item { final int id; public Item(int id) { this.id = id; } public int getId() { return id; } }
'''

ADAPTERS['core/game/container/impl/EquipmentContainer.java'] = '''package core.game.container.impl;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
public class EquipmentContainer {
    /* ORIGINAL_EQUIPMENT_SLOTS */
    final Player owner;
    public EquipmentContainer(Player owner) { this.owner = owner; }
    public Item getNew(int slot) { oracle.Support.emit("equipment_new", oracle.Support.ix(owner), slot); return new Item(((Number) oracle.Support.answer("equipment_new:" + oracle.Support.ix(owner), 0L)).intValue()); }
}
'''

ADAPTERS['core/game/node/entity/skill/Skills.java'] = '''package core.game.node.entity.skill;
import core.game.node.entity.Entity;
public class Skills {
    /* ORIGINAL_SKILL_CONSTANTS */
    final Entity owner;
    public Skills(Entity owner) { this.owner = owner; }
    public int getLevel(int slot) { oracle.Support.emit("level", oracle.Support.ix(owner), slot); return ((Number) oracle.Support.answer("level:" + oracle.Support.ix(owner), 1L)).intValue(); }
    public void restore() { oracle.Support.emit("skills_restore", oracle.Support.ix(owner)); }
}
'''

ADAPTERS['core/cache/def/impl/NPCDefinition.java'] = '''package core.cache.def.impl;
import core.game.node.entity.npc.NPC;
import java.util.HashMap;
import java.util.Map;
public class NPCDefinition {
    final NPC owner;
    public NPCDefinition(NPC owner) { this.owner = owner; }
    @SuppressWarnings("unchecked")
    public <T> T getConfiguration(String key, T fallback) {
        oracle.Support.emit("config", oracle.Support.ix(owner), key);
        Object value = oracle.Support.answer("config:" + oracle.Support.ix(owner) + ":" + key, null);
        if (value == null) return fallback;
        if (value instanceof Number) return (T) Integer.valueOf(((Number) value).intValue());
        return (T) value;
    }
    public int getCombatLevel() { oracle.Support.emit("definition_combat_level", oracle.Support.ix(owner)); return ((Number) oracle.Support.answer("definition_combat_level:" + oracle.Support.ix(owner), 1L)).intValue(); }
    public boolean hasAction(String action) { oracle.Support.emit("has_action", oracle.Support.ix(owner), action); return Boolean.TRUE.equals(oracle.Support.answer("has_action:" + oracle.Support.ix(owner) + ":" + action, true)); }
    public Map<String, Object> getHandlers() {
        return new HashMap<String, Object>() {
            @Override public Object getOrDefault(Object key, Object fallback) { oracle.Support.emit("water_npc", oracle.Support.ix(owner)); return oracle.Support.answer("water_npc:" + oracle.Support.ix(owner), false); }
        };
    }
}
'''

ADAPTERS['core/game/system/config/NPCConfigParser.java'] = '''package core.game.system.config;
public class NPCConfigParser { /* ORIGINAL_CONFIG_KEYS */ }
'''

ADAPTERS['core/game/node/entity/combat/CombatStyle.java'] = '''package core.game.node.entity.combat;
public enum CombatStyle {
    MELEE, RANGE, MAGIC;
    final CombatSwingHandler handler = new CombatSwingHandler(name());
    public CombatSwingHandler getSwingHandler() { return handler; }
}
'''

ADAPTERS['core/game/node/entity/combat/CombatSwingHandler.java'] = '''package core.game.node.entity.combat;
public class CombatSwingHandler { public final String tag; public CombatSwingHandler(String tag) { this.tag = tag; } }
'''

ADAPTERS['core/game/node/entity/combat/DeathTask.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
public class DeathTask { public static boolean isDead(Entity e) { return oracle.Support.bool("is_dead", e, false); } }
'''

ADAPTERS['core/game/node/entity/combat/BattleState.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
// Plain fields; neutralizeHits is the original text.
public class BattleState {
    private Entity entity, victim; private int estimatedHit, secondaryHit = -1, poisonDamage = -1; private CombatStyle style;
    public BattleState(Entity entity, Entity victim) { this.entity = entity; this.victim = victim; }
    public Entity getAttacker() { return entity; }
    public Entity getVictim() { return victim; }
    public int getEstimatedHit() { return estimatedHit; }
    public void setEstimatedHit(int v) { estimatedHit = v; }
    public int getSecondaryHit() { return secondaryHit; }
    public void setSecondaryHit(int v) { secondaryHit = v; }
    public int getPoisonDamage() { return poisonDamage; }
    public void setPoisonDamage(int v) { poisonDamage = v; }
    public CombatStyle getStyle() { return style; }
    public void setStyle(CombatStyle v) { style = v; }
    /* ORIGINAL_NEUTRALIZE */
}
'''

ADAPTERS['core/game/node/entity/combat/ImpactHandler.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
public class ImpactHandler {
    public static enum HitsplatType { MISS, NORMAL, POISON, DISEASE, NORMAL_1, VENOM; }
    final Entity owner;
    public ImpactHandler(Entity owner) { this.owner = owner; }
    public void setDisabledTicks(int ticks) { oracle.Support.emit("impact_disabled", oracle.Support.ix(owner), ticks); }
    public void manualHit(Entity source, int hit, HitsplatType type) { oracle.Support.emit("manual_hit", oracle.Support.ix(owner), oracle.Support.ix(source), hit, type.name()); }
    public void handleRecoilEffect(Entity attacker, int hit) { oracle.Support.emit("handle_recoil_effect", oracle.Support.ix(owner), oracle.Support.ix(attacker), hit); }
}
'''

ADAPTERS['core/game/node/entity/player/info/Rights.java'] = '''package core.game.node.entity.player.info;
public enum Rights { REGULAR_PLAYER, PLAYER_MODERATOR, ADMINISTRATOR }
'''

ADAPTERS['content/global/skill/slayer/Tasks.java'] = '''package content.global.skill.slayer;
public class Tasks { public final int levelReq; public Tasks(int levelReq) { this.levelReq = levelReq; } }
'''

ADAPTERS['content/global/skill/summoning/familiar/Familiar.java'] = '''package content.global.skill.summoning.familiar;
import core.game.node.entity.Entity;
import core.game.node.entity.npc.NPC;
import core.game.world.map.Location;
public class Familiar extends NPC {
    public Familiar(int id, Location location) { super(id, location); }
    public Entity getOwner() { oracle.Support.emit("familiar_owner", oracle.Support.ix(this)); Object o = oracle.Support.answer("familiar_owner:" + oracle.Support.ix(this), null); return o == null ? null : oracle.Support.entity(((Number) o).intValue()); }
}
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.DeathTask;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.Skills;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import core.game.world.map.path.Pathfinder;
import java.util.*;
// Stand-ins for the entity's sub-objects; the members named in generate.py are the original text.
public class Entity {
    static class System { static long currentTimeMillis() { return oracle.Support.millis(); } }
    public static final Set<String> TICK_KEYS = new HashSet<>(Arrays.asList("no-spawn-return", "return-to-spawn", "return-to-spawn-pulse", "lastForceTalk"));
    protected Location location;
    protected boolean invisible;
    public final Properties properties = new Properties();
    public final Skills skills = new Skills(this);
    public final Timers timers = new Timers();
    public final Map<String, Object> attributes = new HashMap<>();
    final Locks locks = new Locks();
    final ImpactHandler impactHandler = new ImpactHandler(this);
    final ZoneMonitor zoneMonitor = new ZoneMonitor();
    final Queue walkingQueue = new Queue();
    final Pulses pulseManager = new Pulses();
    final Masks updateMasks = new Masks();
    public Integer me() { return oracle.Support.ix(this); }

    public class CombatPulse {
        public boolean attacking; public Entity victim; public int nextAttack, lastSentAttack, lastReceivedAttack; public CombatStyle style = CombatStyle.MELEE;
        public boolean isAttacking() { oracle.Support.emit("is_attacking", me()); return attacking; }
        public boolean isInCombat() { return oracle.Support.bool("is_in_combat", Entity.this, false); }
        public Entity getVictim() { oracle.Support.emit("combat_victim", me()); return victim; }
        public void setVictim(Entity v) { oracle.Support.emit("combat_set_victim", me(), oracle.Support.ix(v)); victim = v; }
        public void attack(Entity v) { oracle.Support.emit("combat_attack", me(), oracle.Support.ix(v)); if (v != null && oracle.Support.sticky()) { attacking = true; victim = v; } }
        public void stop() { oracle.Support.emit("combat_stop", me()); attacking = false; victim = null; }
        public int getNextAttack() { oracle.Support.emit("next_attack", me()); return nextAttack; }
        public CombatStyle getStyle() { oracle.Support.emit("combat_style", me()); return style; }
        public void setLastReceivedAttack(int v) { oracle.Support.emit("set_last_received_attack", me(), v); lastReceivedAttack = v; }
        public int getLastSentAttack() { oracle.Support.emit("last_sent_attack", me()); return lastSentAttack; }
        public void setLastSentAttack(int v) { oracle.Support.emit("set_last_sent_attack", me(), v); lastSentAttack = v; }
    }
    public class Properties {
        public final CombatPulse combat = new CombatPulse(); public Location spawnLocation;
        public CombatPulse getCombatPulse() { return combat; }
        public Location getSpawnLocation() { return spawnLocation; }
        public boolean isMultiZone() { return oracle.Support.bool("is_multi_zone", Entity.this, false); }
        public int getCurrentCombatLevel() { return oracle.Support.integer("current_combat_level", Entity.this, 3); }
        public boolean isRetaliating() { return oracle.Support.bool("is_retaliating", Entity.this, true); }
        public void setNPCWalkable(boolean v) { oracle.Support.emit("set_npc_walkable", me(), v); }
        public void setCombatTimeOut(int v) { oracle.Support.emit("set_combat_time_out", me(), v); }
    }
    public class Locks {
        public boolean isInteractionLocked() { return oracle.Support.bool("interaction_locked", Entity.this, false); }
        public boolean isMovementLocked() { return oracle.Support.bool("movement_locked", Entity.this, false); }
        public void lockMovement(int ticks) { oracle.Support.emit("lock_movement", me(), ticks); }
        public void unlockMovement() { oracle.Support.emit("unlock_movement", me()); }
    }
    public class Timers { public void removeTimer(String name) { oracle.Support.emit("remove_timer", me(), name); } }
    public class ZoneMonitor {
        public boolean continueAttack(Entity target, CombatStyle style, boolean message) { oracle.Support.emit("zone_continue_attack", me(), oracle.Support.ix(target), style == null ? null : style.name(), message); return Boolean.TRUE.equals(oracle.Support.answer("zone_continue_attack:" + me(), true)); }
        public boolean isPvPable(Entity attacker, Entity victim, CombatStyle style, boolean message) { oracle.Support.emit("zone_is_pvpable", me(), oracle.Support.ix(attacker), oracle.Support.ix(victim), style == null ? null : style.name(), message); return Boolean.TRUE.equals(oracle.Support.answer("zone_is_pvpable:" + me(), false)); }
    }
    public class Queue {
        public void reset() { oracle.Support.emit("walking_reset", me()); }
        public boolean hasPath() { return oracle.Support.bool("walking_has_path", Entity.this, false); }
    }
    public class Pulses {
        public boolean hasPulseRunning() { return oracle.Support.bool("has_pulse_running", Entity.this, false); }
        public void clear() { oracle.Support.emit("pulse_manager_clear", me()); }
        public boolean isMovingPulse() { return oracle.Support.bool("pulse_manager_is_moving", Entity.this, false); }
    }
    public class Masks { public void reset() { oracle.Support.emit("masks_reset", me()); } }

    public Location getLocation() { return location; }
    public void setLocation(Location l) { location = l; }
    public Properties getProperties() { return properties; }
    public Locks getLocks() { return locks; }
    public Skills getSkills() { return skills; }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public ZoneMonitor getZoneMonitor() { return zoneMonitor; }
    public Queue getWalkingQueue() { return walkingQueue; }
    public Pulses getPulseManager() { return pulseManager; }
    public Masks getUpdateMasks() { return updateMasks; }
    public Player asPlayer() { return (Player) this; }
    public boolean invisibleField() { return invisible; }
    public boolean isActive() { return oracle.Support.bool("is_active", this, true); }
    public boolean face(Entity e) { oracle.Support.emit("face", me(), oracle.Support.ix(e)); return true; }
    public void teleport(Location l) { oracle.Support.emit("teleport", me(), oracle.Support.coord(l)); }
    public void tick() { oracle.Support.emit("entity_tick", me()); }
    public boolean canMove(Location l) { oracle.Support.emit("can_move", me(), oracle.Support.coord(l)); return Boolean.TRUE.equals(oracle.Support.answer("can_move:" + me(), true)); }
    public boolean sendChat(String s) { oracle.Support.emit("send_chat", me(), s); return true; }
    public void moveStep() { oracle.Support.emit("move_step", me()); }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key) { if (!TICK_KEYS.contains(key)) oracle.Support.emit("get_attribute", me(), key); return (T) attributes.get(key); }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fallback) { if (!TICK_KEYS.contains(key)) oracle.Support.emit("get_attribute", me(), key); Object v = attributes.get(key); return v == null ? fallback : (T) v; }
    // An attribute write is the entity's own state: it happens, then it is recorded (as in the npc-tick oracle).
    public void setAttribute(String key, Object value) { attributes.put(key, value); oracle.Support.emit("set_attribute", me(), key); }
    public void removeAttribute(String key) { attributes.remove(key); oracle.Support.emit("remove_attribute", me(), key); }

    /* ORIGINAL_ENTITY_METHODS */
}
'''

ADAPTERS['core/game/node/entity/npc/NPC.java'] = '''package core.game.node.entity.npc;
import content.global.skill.slayer.Tasks;
import content.global.skill.summoning.familiar.Familiar;
import core.api.utils.Vector;
import core.cache.def.impl.NPCDefinition;
import core.game.interaction.MovementPulse;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.node.entity.npc.agg.AggressiveBehavior;
import core.game.node.entity.npc.agg.AggressiveHandler;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.Skills;
import core.game.system.config.NPCConfigParser;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import core.game.world.map.build.DynamicRegion;
import core.game.world.map.path.Pathfinder;
import core.game.world.repository.Repository;
import core.tools.RandomFunction;
// The fields of NPC.java; the members named in generate.py are the original text.
public class NPC extends Entity {
    private int id;
    private NPCDefinition definition;
    public boolean walks;
    public boolean aggressive;
    protected Location[] movementPath;
    protected int walkRadius = 11;
    protected int movementIndex;
    public int respawnTick = -1;
    protected boolean pathBoundMovement;
    protected Player dialoguePlayer;
    protected AggressiveHandler aggressiveHandler;
    protected int nextWalk;
    public Tasks task;
    public boolean neverWalks;
    public String forceTalk;
    public NPCBehavior behavior;
    public boolean isRespawning = false;

    public NPC(int id, Location location) {
        this.id = id;
        this.location = location;
        this.definition = new NPCDefinition(this);
        this.properties.spawnLocation = location;
        this.behavior = NPCBehavior.forId(id);
    }
    public int getId() { return id; }
    public void setWalks(boolean walks) { this.walks = walks; }
    public NPC getShownNPC(Player player) { oracle.Support.emit("shown_npc", oracle.Support.ix(this), oracle.Support.ix(player)); Object o = oracle.Support.answer("shown_npc:" + oracle.Support.ix(this), null); return o == null ? this : (NPC) oracle.Support.entity(((Number) o).intValue()); }
    public void clear() { oracle.Support.emit("npc_clear", oracle.Support.ix(this)); }
    public NPC transform(int id) { oracle.Support.emit("transform", oracle.Support.ix(this), id); return this; }
    public void reTransform() { oracle.Support.emit("re_transform", oracle.Support.ix(this)); }
    public Tasks getTask() { return task; }

    /* ORIGINAL_NPC_METHODS */
}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
import core.game.container.impl.EquipmentContainer;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.ImpactHandler.HitsplatType;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.info.Rights;
import core.game.world.GameWorld;
// The members named in generate.py are the original text.
public class Player extends Entity {
    final String name;
    public final SkullManager skullManager = new SkullManager();
    public final Degrader degrader = new Degrader();
    final EquipmentContainer equipment = new EquipmentContainer(this);
    public Player(String name) { this.name = name; }
    public String getName() { oracle.Support.emit("name", me()); return name; }
    public Rights getRights() { oracle.Support.emit("rights", me()); Object o = oracle.Support.answer("rights:" + me(), "REGULAR_PLAYER"); return o == null ? null : Rights.valueOf((String) o); }
    public SkullManager getSkullManager() { return skullManager; }
    public EquipmentContainer getEquipment() { return equipment; }
    public PacketDispatch getPacketDispatch() { return new PacketDispatch(); }
    public Iface getInterfaceManager() { return new Iface(); }
    public class Iface { public boolean hasChatbox() { return oracle.Support.bool("dialogue_chatbox", Player.this, true); } }
    @Override public boolean isInvisible() { return oracle.Support.bool("player_invisible", this, false); }
    public class SkullManager {
        public boolean isWilderness() { return oracle.Support.bool("skull_wilderness", Player.this, false); }
        public boolean hasWildernessProtection() { return oracle.Support.bool("wilderness_protection", Player.this, false); }
    }
    public class Degrader { public void checkArmourDegrades(Player p) { oracle.Support.emit("check_armour_degrades", oracle.Support.ix(p)); } }
    public class PacketDispatch { public void sendMessage(String text) { oracle.Support.emit("send_message", me(), text); } }

    /* ORIGINAL_PLAYER_METHODS */
}
'''

ADAPTERS['core/game/node/entity/npc/RecordingNpc.java'] = '''package core.game.node.entity.npc;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
// A content subclass: every overridable member it replaces records and answers from the case.
public class RecordingNpc extends NPC {
    public RecordingNpc(int id, Location location) { super(id, location); }
    boolean hook(String name, Entity other, boolean fallback) { oracle.Support.emit("class_hook", name, oracle.Support.ix(this), oracle.Support.ix(other)); return Boolean.TRUE.equals(oracle.Support.answer("class_hook:" + name, fallback)); }
    @Override public boolean canSelectTarget(Entity target) { return hook("canSelectTarget", target, true); }
    @Override public boolean isIgnoreAttackRestrictions(Entity victim) { return hook("isIgnoreAttackRestrictions", victim, false); }
    @Override protected void onRespawn() { hook("onRespawn", null, false); }
}
'''

ADAPTERS['core/game/node/entity/npc/RecordingBehavior.kt'] = '''package core.game.node.entity.npc
import core.game.node.entity.Entity
import core.game.node.entity.combat.BattleState
import core.game.node.entity.combat.CombatStyle
import core.game.node.entity.combat.CombatSwingHandler
import core.game.node.item.Item
// A content behaviour: every hook records and answers from the case.
class RecordingBehavior(val tag: String) : NPCBehavior() {
    fun hook(name: String, vararg args: Any?): Any? {
        oracle.Support.emit("behaviour_hook", tag, name, *args.map { oracle.Support.repr(it) }.toTypedArray())
        return oracle.Support.answer("behaviour_hook:" + name, null)
    }
    override fun tick(self: NPC): Boolean = hook("tick", self) != false
    override fun beforeDamageReceived(self: NPC, attacker: Entity, state: BattleState) { hook("beforeDamageReceived", self, attacker); oracle.Support.scriptState(state) }
    override fun afterDamageReceived(self: NPC, attacker: Entity, state: BattleState) { hook("afterDamageReceived", self, attacker); oracle.Support.scriptState(state) }
    override fun beforeAttackFinalized(self: NPC, victim: Entity, state: BattleState) { hook("beforeAttackFinalized", self, victim) }
    override fun onRemoval(self: NPC) { hook("onRemoval", self) }
    override fun onCreation(self: NPC) { hook("onCreation", self) }
    override fun onRespawn(self: NPC) { hook("onRespawn", self) }
    override fun onDeathStarted(self: NPC, killer: Entity) { hook("onDeathStarted", self, killer) }
    override fun onDeathFinished(self: NPC, killer: Entity) { hook("onDeathFinished", self, killer) }
    override fun onDropTableRolled(self: NPC, killer: Entity, drops: ArrayList<Item>) { hook("onDropTableRolled", self, killer) }
    override fun canBeAttackedBy(self: NPC, attacker: Entity, style: CombatStyle, shouldSendMessage: Boolean): Boolean = hook("canBeAttackedBy", self, attacker, style.name, shouldSendMessage) != false
    override fun shouldIgnoreMultiRestrictions(self: NPC, victim: Entity): Boolean = hook("shouldIgnoreMultiRestrictions", self, victim) == true
    override fun getSwingHandlerOverride(self: NPC, original: CombatSwingHandler): CombatSwingHandler {
        val answer = hook("getSwingHandlerOverride", self, original.tag)
        return if (answer is String) CombatStyle.valueOf(answer).swingHandler else original
    }
    override fun getXpMultiplier(self: NPC, attacker: Entity): Double { hook("getXpMultiplier", self, attacker); return 2.5 }
}
'''

ADAPTERS['core/game/world/map/Location.kt'] = '''package core.game.world.map
import core.api.utils.Vector
import kotlin.math.*
// The original packed coordinates and the members these paths read, verbatim; the chunk is a stand-in.
class Location(x: Int, y: Int, z: Int) {
    var hash = LocationHash(x, y, (z % 4 + 4) % 4)
    val x: Int get() = hash.x
    val y: Int get() = hash.y
    val z: Int get() = hash.z
    val chunk: RegionChunk get() = RegionChunk(this)
    /* ORIGINAL_LOCATION_MEMBERS */
    companion object {
        @JvmStatic @JvmOverloads fun create(x: Int, y: Int, z: Int = 0): Location = Location(x, y, z)
        /* ORIGINAL_GET_DISTANCE */
    }
    @JvmInline /* ORIGINAL_LOCATION_HASH */
}
'''

ADAPTERS['core/api/utils/Vector.kt'] = '''package core.api.utils
import core.game.world.map.Location
import kotlin.math.*
// The members NPC.getMovementDestination reads, verbatim.
class Vector(val x: Double, val y: Double) {
    /* ORIGINAL_VECTOR_MEMBERS */
    companion object {
        /* ORIGINAL_BETWEEN_LOCS */
    }
}
'''
