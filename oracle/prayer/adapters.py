"""Stand-ins for everything the unchanged prayer classes reach outside themselves.

Each entry is one source file compiled next to the unchanged Prayer.java,
PrayerType.java, PrayerCategory.java, SkillBonus.java, Audio.kt and
PrayerTabInterface.kt. `/* ORIGINAL_* */` markers are filled with original text
at generation time (generate.py, `markers`). Every recorded call goes through
oracle.Support.emit, which also injects the failures of the sweep.
"""

E = 'oracle.Support'
ADAPTERS = {}

ADAPTERS['core/tools/RandomFunction.java'] = '''package core.tools;
// The selected original methods, verbatim; RANDOM is the case's recording generator.
public class RandomFunction {
    public static java.util.Random RANDOM;
    /* ORIGINAL_RANDOM_METHODS */
}
'''

ADAPTERS['core/tools/StringUtils.java'] = '''package core.tools;
// The original formatDisplayName, verbatim.
public final class StringUtils {
    /* ORIGINAL_FORMAT_DISPLAY_NAME */
}
'''

ADAPTERS['core/game/system/task/Pulse.java'] = '''package core.game.system.task;
// The members a submitted anonymous pulse uses; the harness fires queued pulses on request.
public abstract class Pulse {
    private final int delay;
    public Pulse(int delay) { this.delay = delay; }
    public int getDelay() { return delay; }
    public abstract boolean pulse();
}
'''

ADAPTERS['core/game/world/GameWorld.java'] = '''package core.game.world;
import core.game.system.task.Pulse;
// GameWorld.getPulser().submit records the submission and queues the pulse for the harness.
public final class GameWorld {
    public static final class Pulser {
        public void submit(Pulse pulse) { oracle.Support.emit("submit", pulse.getDelay()); oracle.Support.queue(pulse); }
    }
    private static final Pulser PULSER = new Pulser();
    public static Pulser getPulser() { return PULSER; }
}
'''

ADAPTERS['core/game/world/map/Location.kt'] = '''package core.game.world.map
import kotlin.math.*
// The original packed coordinates and the members Prayer reads, verbatim.
class Location(x: Int, y: Int, z: Int) {
    var hash = LocationHash(x, y, (z % 4 + 4) % 4)
    val x: Int get() = hash.x
    val y: Int get() = hash.y
    val z: Int get() = hash.z
    /* ORIGINAL_LOCATION_MEMBERS */
    companion object {
        @JvmStatic @JvmOverloads fun create(x: Int, y: Int, z: Int = 0): Location = Location(x, y, z)
        /* ORIGINAL_GET_DISTANCE */
    }
    @JvmInline /* ORIGINAL_LOCATION_HASH */
}
'''

ADAPTERS['core/game/world/map/RegionManager.java'] = '''package core.game.world.map;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import java.util.List;
// Recording local-entity queries answered from the case.
public final class RegionManager {
    public static List<NPC> getLocalNPCs(Location l, int distance) {
        oracle.Support.emit("local_npcs", oracle.Support.coord(l), distance);
        return oracle.Support.locals("localNpcs", NPC.class);
    }
    public static List<Player> getLocalPlayers(Location l, int distance) {
        oracle.Support.emit("local_players", oracle.Support.coord(l), distance);
        return oracle.Support.locals("localPlayers", Player.class);
    }
}
'''

ADAPTERS['core/game/world/update/flag/context/Graphics.java'] = '''package core.game.world.update.flag.context;
// Graphics.create(id): only the id is read here.
public class Graphics {
    public final int id;
    private Graphics(int id) { this.id = id; }
    public static Graphics create(int id) { return new Graphics(id); }
}
'''

ADAPTERS['core/game/node/entity/impl/Projectile.java'] = '''package core.game.node.entity.impl;
import core.game.world.map.Location;
// Projectile.create(...).send() records the projectile.
public class Projectile {
    private final Object[] values;
    private Projectile(Object... values) { this.values = values; }
    public static Projectile create(Location start, Location destination, int projectileId, int startHeight, int endHeight, int startDelay, int speed, int angle, int distance) {
        return new Projectile(oracle.Support.coord(start), oracle.Support.coord(destination), projectileId, startHeight, endHeight, startDelay, speed, angle, distance);
    }
    public void send() { oracle.Support.emit("projectile", values); }
}
'''

ADAPTERS['core/game/node/entity/combat/CombatStyle.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.player.link.prayer.PrayerType;
// The constants and protection prayers of the original constructor calls.
public enum CombatStyle {
    /* ORIGINAL_COMBAT_STYLES */;
    private final PrayerType protectionPrayer;
    CombatStyle(PrayerType protectionPrayer) { this.protectionPrayer = protectionPrayer; }
    public PrayerType getProtectionPrayer() { return protectionPrayer; }
}
'''

ADAPTERS['core/game/node/entity/combat/equipment/ArmourSet.java'] = '''package core.game.node.entity.combat.equipment;
// The constant names of the original enum.
public enum ArmourSet { /* ORIGINAL_ARMOUR_SETS */ }
'''

ADAPTERS['core/game/node/entity/combat/BattleState.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.equipment.ArmourSet;
// The four getters Entity.getFormattedHit reads.
public class BattleState {
    public Entity attacker, victim;
    public CombatStyle style;
    public ArmourSet armourEffect;
    public Entity getAttacker() { return attacker; }
    public Entity getVictim() { return victim; }
    public CombatStyle getStyle() { return style; }
    public ArmourSet getArmourEffect() { return armourEffect; }
}
'''

ADAPTERS['core/game/node/entity/combat/ImpactHandler.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
// manualHit records the hit; HitsplatType is the original enum.
public class ImpactHandler {
    private final Entity entity;
    public ImpactHandler(Entity entity) { this.entity = entity; }
    public Object manualHit(Entity source, int hit, HitsplatType type) {
        oracle.Support.emit("manual_hit", oracle.Support.ix(entity), oracle.Support.ix(source), hit, type.name());
        return null;
    }
    /* ORIGINAL_HITSPLAT */
}
'''

ADAPTERS['core/game/node/entity/Properties.java'] = '''package core.game.node.entity;
import core.game.node.entity.combat.CombatStyle;
// Recording bonus and multi-zone reads; the NPC protect style is a plain field.
public class Properties {
    private final Entity entity;
    public int[] bonuses;
    public CombatStyle protectStyle;
    public Properties(Entity entity) { this.entity = entity; }
    public int[] getBonuses() { oracle.Support.emit("bonuses", oracle.Support.ix(entity)); return bonuses; }
    public boolean isMultiZone() { oracle.Support.emit("multi_zone", oracle.Support.ix(entity)); return oracle.Support.flag(entity, "multi", false); }
    public CombatStyle getProtectStyle() { return protectStyle; }
}
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.combat.equipment.ArmourSet;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Graphics;
// Recording Entity members; getFormattedHit and isIgnoreProtection are original text.
public abstract class Entity {
    public Location location;
    public final Properties properties = new Properties(this);
    private final Skills skills = new Skills(this);
    private final ImpactHandler impactHandler = new ImpactHandler(this);
    public Location getLocation() { oracle.Support.emit("location", oracle.Support.ix(this)); return location; }
    public Properties getProperties() { return properties; }
    public Skills getSkills() { return skills; }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public Player asPlayer() { return (Player) this; }
    public boolean graphics(Graphics graphics) { oracle.Support.emit("graphics", oracle.Support.ix(this), graphics.id); return true; }
    public boolean isAttackable(Entity entity, CombatStyle style, boolean message) {
        oracle.Support.emit("is_attackable", oracle.Support.ix(this), oracle.Support.ix(entity), style.name(), message);
        return oracle.Support.flag(this, "attackable", true);
    }
    public void commenceDeath(Entity killer) { oracle.Support.emit("commence_death_super", oracle.Support.ix(this), oracle.Support.ix(killer)); }
    public void fullRestore() { oracle.Support.emit("full_restore_super", oracle.Support.ix(this)); }
    public abstract boolean hasProtectionPrayer(CombatStyle style);
    /* ORIGINAL_ENTITY_METHODS */
}
'''

ADAPTERS['core/game/node/entity/npc/NPC.java'] = '''package core.game.node.entity.npc;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatStyle;
// NPC.hasProtectionPrayer is original text.
public class NPC extends Entity {
    /* ORIGINAL_NPC_METHODS */
}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
import core.game.event.Event;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.Settings;
import core.game.node.entity.player.link.appearance.Appearance;
import core.game.node.entity.player.link.prayer.Prayer;
import core.game.node.entity.player.link.prayer.PrayerType;
import java.util.HashMap;
// The original varp maps and Prayer field; hasProtectionPrayer, commenceDeath and fullRestore are original text.
public class Player extends Entity {
    public HashMap<Integer, Integer> varpMap = new HashMap<>();
    public HashMap<Integer, Boolean> saveVarp = new HashMap<>();
    public final HashMap<Integer, Integer> varbits = new HashMap<>();
    private final Prayer prayer = new Prayer(this);
    private final PacketDispatch packetDispatch = new PacketDispatch(this);
    private final Appearance appearance = new Appearance(this);
    private final Settings settings = new Settings(this);
    public Prayer getPrayer() { return prayer; }
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public Appearance getAppearance() { return appearance; }
    public void dispatch(Event event) { oracle.Support.event(this, event); }
    public void sendMessage(String message) { oracle.Support.emit("send_message", oracle.Support.ix(this), message); }
    public boolean isPlaying() { oracle.Support.emit("is_playing", oracle.Support.ix(this)); return oracle.Support.flag(this, "playing", true); }
    /* ORIGINAL_PLAYER_METHODS */
}
'''

ADAPTERS['core/game/node/entity/player/link/PacketDispatch.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
// The varp packet is recorded.
public class PacketDispatch {
    private final Player player;
    public PacketDispatch(Player player) { this.player = player; }
    public void sendVarp(int index, int value) { oracle.Support.emit("send_varp", oracle.Support.ix(player), index, value); }
}
'''

ADAPTERS['core/game/node/entity/player/link/Settings.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.player.Player;
// The two energy setters fullRestore calls are recorded.
public class Settings {
    private final Player player;
    public Settings(Player player) { this.player = player; }
    public void setSpecialEnergy(int value) { oracle.Support.emit("set_special_energy", oracle.Support.ix(player), value); }
    public void updateRunEnergy(double drain) { oracle.Support.emit("update_run_energy", oracle.Support.ix(player), drain); }
}
'''

ADAPTERS['core/game/node/entity/player/link/appearance/Appearance.java'] = '''package core.game.node.entity.player.link.appearance;
import core.game.node.entity.player.Player;
// The head icon field with recorded reads, writes and syncs.
public class Appearance {
    private final Player player;
    public int headIcon = -1;
    public Appearance(Player player) { this.player = player; }
    public int getHeadIcon() { oracle.Support.emit("head_icon", oracle.Support.ix(player)); return headIcon; }
    public void setHeadIcon(int icon) { oracle.Support.emit("set_head_icon", oracle.Support.ix(player), icon); headIcon = icon; }
    public void sync() { oracle.Support.emit("sync", oracle.Support.ix(player)); }
}
'''

ADAPTERS['core/game/node/entity/skill/Skills.java'] = '''package core.game.node.entity.skill;
import content.global.skill.skillcapeperks.SkillcapePerks;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.net.packet.PacketRepository;
import core.net.packet.context.SkillContext;
import core.net.packet.out.SkillLevel;
// The original fields and the prayer-point, static level and healing members, verbatim.
public final class Skills {
    /* ORIGINAL_SKILL_CONSTANTS */
    public final Entity entity;
    public final int[] staticLevels = new int[24];
    public double prayerPoints = 1.;
    public int lifepoints = 10;
    public int lifepointsIncrease = 0;
    public boolean lifepointsUpdate = false;
    public Skills(Entity entity) { this.entity = entity; }
    public Entity getEntity() { return entity; }
    /* ORIGINAL_SKILLS_METHODS */
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
    DIVINE_FAVOR, DAMAGE_SPONG;
    public static boolean isActive(SkillcapePerks perk, Player player) {
        oracle.Support.emit("perk", oracle.Support.ix(player), perk.name());
        return oracle.Support.perk(player, perk.name());
    }
}
'''

ADAPTERS['content/data/Quests.java'] = '''package content.data;
public enum Quests { KINGS_RANSOM }
'''

ADAPTERS['core/game/node/entity/player/link/diary/DiaryType.java'] = '''package core.game.node.entity.player.link.diary;
// Imported by PrayerType, never used.
public enum DiaryType {}
'''

ADAPTERS['core/game/world/map/zone/ZoneBorders.java'] = '''package core.game.world.map.zone;
// Imported by PrayerType, never used.
public class ZoneBorders {}
'''

ADAPTERS['core/game/component/Component.java'] = '''package core.game.component;
public class Component {}
'''

ADAPTERS['core/game/event/Events.kt'] = '''package core.game.event
import core.game.node.entity.player.link.prayer.PrayerType
interface Event
/* ORIGINAL_PRAYER_EVENTS */
'''

ADAPTERS['core/game/interaction/InterfaceListener.kt'] = '''package core.game.interaction
import core.game.component.Component
import core.game.node.entity.player.Player
// on(componentID, handler) hands the handler to the harness instead of InterfaceListeners.
interface InterfaceListener {
    fun defineInterfaceListeners()
    fun on(componentID: Int, handler: (player: Player, component: Component, opcode: Int, buttonID: Int, slot: Int, itemID: Int) -> Boolean) {
        oracle.Support.register(componentID, handler)
    }
}
'''

ADAPTERS['core/api/ContentAPI.kt'] = '''package core.api
import content.data.Quests
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.audio.Audio
import core.game.world.map.Location
// hasLevelStat and setVarp are original text; the rest record and answer from the case.
/* ORIGINAL_HAS_LEVEL_STAT */

fun sendDialogue(player: Player, message: String) {
    oracle.Support.emit("send_dialogue", oracle.Support.ix(player), message)
}

@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: Location? = null, radius: Int = Audio.defaultAudioRadius) {
    oracle.Support.emit("play_audio", oracle.Support.ix(player), id, delay, loops, oracle.Support.coord(location), radius)
}

/* ORIGINAL_SET_VARP */

fun getVarbit(player: Player, varbitId: Int): Int {
    oracle.Support.emit("get_varbit", oracle.Support.ix(player), varbitId)
    return player.varbits[varbitId] ?: 0
}

@JvmOverloads
fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("set_varbit", oracle.Support.ix(player), varbitId, value, save)
    player.varbits[varbitId] = value
}

@JvmOverloads
fun hasRequirement(player: Player, quest: Quests, message: Boolean = true): Boolean {
    oracle.Support.emit("has_requirement", oracle.Support.ix(player), quest.name, message)
    return oracle.Support.flag(player, "kingsRansom", false)
}
'''
