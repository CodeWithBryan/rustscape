"""Stand-ins for everything the unchanged player tab classes reach outside themselves.

Each entry is one source file compiled next to the unchanged WeaponInterface,
ComponentDefinition, InterfaceType, LevelUp, LevelUpJingles, EmoteManager,
Emotes, MusicPlayer, MusicEntry, MusicZone, QuestRepository, Quest and Quests.
`/* ORIGINAL_* */` markers are filled with original text at generation time
(generate.py, `markers`). Every recorded call goes through oracle.Support.emit,
which also injects the failures of the sweep. Stand-ins for paths the corpus
does not drive throw.
"""

ADAPTERS = {}

ADAPTERS['core/tools/Log.kt'] = '''package core.tools
// The original constants, verbatim.
/* ORIGINAL_LOG */
'''

ADAPTERS['core/plugin/Plugin.java'] = '''package core.plugin;
// The two abstract members of the original interface.
public interface Plugin<T> {
    Plugin<T> newInstance(T arg) throws Throwable;
    Object fireEvent(String identifier, Object... args);
}
'''

ADAPTERS['core/plugin/PluginType.java'] = '''package core.plugin;
public enum PluginType { ACTION, DIALOGUE, QUEST }
'''

ADAPTERS['core/plugin/PluginManifest.java'] = '''package core.plugin;
import java.lang.annotation.*;
@Retention(RetentionPolicy.RUNTIME)
public @interface PluginManifest { PluginType type() default PluginType.ACTION; String name() default ""; }
'''

ADAPTERS['core/game/component/ComponentPlugin.java'] = '''package core.game.component;
// Only stored by ComponentDefinition here.
public abstract class ComponentPlugin {}
'''

ADAPTERS['core/game/component/CloseEvent.java'] = '''package core.game.component;
import core.game.node.entity.player.Player;
public interface CloseEvent { boolean close(Player player, Component c); }
'''

ADAPTERS['core/game/component/Component.java'] = '''package core.game.component;
import core.game.node.entity.player.Player;
// The original fields and constructor body; open is overridden by WeaponInterface and not reached otherwise.
public class Component {
    protected int id;
    protected final ComponentDefinition definition;
    protected CloseEvent closeEvent;
    protected ComponentPlugin plugin;
    public Component(int id) {
        this.id = id;
        this.definition = ComponentDefinition.forId(id);
        this.plugin = definition.getPlugin();
    }
    public void open(Player player) { throw new IllegalStateException("not driven"); }
    public int getId() { return id; }
    public ComponentDefinition getDefinition() { return definition; }
    public CloseEvent getCloseEvent() { return closeEvent; }
    public Component setCloseEvent(CloseEvent closeEvent) { this.closeEvent = closeEvent; return this; }
}
'''

ADAPTERS['core/game/world/GameWorld.java'] = '''package core.game.world;
import core.game.system.task.Pulse;
// Recording tick reads; the pulser is reached only by the undriven GIVE_THANKS emote.
public final class GameWorld {
    public static final class Pulser { public void submit(Pulse pulse) { throw new IllegalStateException("not driven"); } }
    public static int getTicks() { oracle.Support.emit("ticks"); return oracle.Support.ticks(); }
    public static Pulser getPulser() { return new Pulser(); }
}
'''

ADAPTERS['core/game/system/task/Pulse.java'] = '''package core.game.system.task;
import core.game.node.Node;
public abstract class Pulse {
    public Pulse(int delay, Node... checks) {}
    public abstract boolean pulse();
}
'''

ADAPTERS['core/game/world/map/Location.java'] = '''package core.game.world.map;
// Coordinates only; reached by the undriven SALUTE and GOBLIN_BOW emotes.
public class Location {
    public final int x, y, z;
    public Location(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
    public int getRegionId() { throw new IllegalStateException("not driven"); }
    public boolean withinDistance(Location other, int distance) { throw new IllegalStateException("not driven"); }
    @Override public boolean equals(Object o) { throw new IllegalStateException("not driven"); }
    @Override public int hashCode() { return x ^ y ^ z; }
}
'''

ADAPTERS['core/game/world/map/Direction.java'] = '''package core.game.world.map;
public enum Direction { NORTH_WEST, NORTH, NORTH_EAST, WEST, EAST, SOUTH_WEST, SOUTH, SOUTH_EAST }
'''

ADAPTERS['core/game/world/map/RegionManager.java'] = '''package core.game.world.map;
import core.game.node.entity.npc.NPC;
import java.util.List;
public final class RegionManager {
    public static List<NPC> getLocalNPCs(Location location) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/world/map/zone/Zone.java'] = '''package core.game.world.map.zone;
import core.game.node.entity.Entity;
public interface Zone {
    boolean enter(Entity e);
    boolean leave(Entity e, boolean logout);
}
'''

ADAPTERS['core/game/world/map/zone/ZoneBorders.java'] = '''package core.game.world.map.zone;
public class ZoneBorders {
    public ZoneBorders(int southWestX, int southWestY, int northEastX, int northEastY) {}
}
'''

ADAPTERS['core/game/world/update/flag/context/Graphics.java'] = '''package core.game.world.update.flag.context;
// The original constructor defaults.
public class Graphics {
    public final int id, height, delay;
    public Graphics(int id) { this(id, 0, 0); }
    public Graphics(int id, int height) { this(id, height, 0); }
    public Graphics(int id, int height, int delay) { this.id = id; this.height = height; this.delay = delay; }
    public static Graphics create(int id) { return new Graphics(id, 0, 0); }
    public static Graphics create(int id, int height) { return new Graphics(id, height, 0); }
}
'''

ADAPTERS['core/game/world/update/flag/context/Animation.java'] = '''package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
// The original constructor defaults; getDuration's definition lookup is a recorded read answered from the case.
public class Animation {
    public final int id, delay;
    public final Priority priority;
    public Animation(int id) { this(id, 0, Priority.MID); }
    public static Animation create(int id) { return new Animation(id, 0, Priority.MID); }
    public Animation(int id, Priority priority) { this(id, 0, priority); }
    public Animation(int id, int delay) { this(id, delay, Priority.MID); }
    public Animation(int id, int delay, Priority priority) { this.id = id; this.delay = delay; this.priority = priority; }
    public int getId() { return id; }
    public int getDuration() { oracle.Support.emit("duration", id); return oracle.Support.duration(id); }
}
'''

ADAPTERS['core/game/node/entity/impl/Animator.java'] = '''package core.game.node.entity.impl;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
// The original Priority constants; animate is recorded.
public class Animator {
    public static enum Priority { /* ORIGINAL_PRIORITIES */ }
    public boolean animate(Animation animation, Graphics graphic) {
        oracle.Support.emit("animate", oracle.Support.anim(animation), oracle.Support.gfx(graphic));
        return true;
    }
}
'''

ADAPTERS['core/game/node/Node.java'] = '''package core.game.node;
import core.game.world.map.Location;
public abstract class Node {
    public Location location;
    public Location getLocation() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/item/Item.java'] = '''package core.game.node.item;
import core.game.node.Node;
import core.cache.def.impl.ItemDefinition;
// Id only; the name is the definition name the case supplies (not a recorded call).
public class Item extends Node {
    private final int id;
    public Item(int id) { this.id = id; }
    public int getId() { return id; }
    public String getName() { return oracle.Support.itemName(id); }
    public ItemDefinition getDefinition() { return new ItemDefinition(id); }
}
'''

ADAPTERS['core/cache/def/impl/ItemDefinition.java'] = '''package core.cache.def.impl;
// The name the case supplies; configuration reads are recorded and answered from the case.
public class ItemDefinition {
    private final int id;
    public ItemDefinition(int id) { this.id = id; }
    public String getName() { return oracle.Support.itemName(id); }
    @SuppressWarnings("unchecked")
    public <V> V getConfiguration(String key, V fail) {
        oracle.Support.emit("item_config", id, key);
        Object v = oracle.Support.itemConfig(id, key);
        return v == null ? fail : (V) v;
    }
}
'''

ADAPTERS['core/game/system/config/ItemConfigParser.java'] = '''package core.game.system.config;
// The original keys.
public final class ItemConfigParser {
        /* ORIGINAL_ITEM_CONFIG_KEYS */
}
'''

ADAPTERS['core/game/container/impl/EquipmentContainer.java'] = '''package core.game.container.impl;
import core.game.node.item.Item;
// The worn items: recorded get/getNew reads; the slot constants are original text.
public class EquipmentContainer {
    /* ORIGINAL_SLOTS */
    public Item get(int slot) {
        oracle.Support.emit("equipment", slot);
        Integer id = oracle.Support.worn(slot);
        return id == null ? null : new Item(id);
    }
    public Item getNew(int slot) {
        oracle.Support.emit("equipment_new", slot);
        Integer id = oracle.Support.worn(slot);
        return id == null ? new Item(0) : new Item(id);
    }
}
'''

ADAPTERS['core/game/node/entity/combat/spell/MagicSpell.java'] = '''package core.game.node.entity.combat.spell;
// A registered spell of the case: its id only.
public class MagicSpell {
    public final int spellId;
    public MagicSpell(int spellId) { this.spellId = spellId; }
    public int getSpellId() { return spellId; }
}
'''

ADAPTERS['core/game/node/entity/combat/spell/CombatSpell.java'] = '''package core.game.node.entity.combat.spell;
public class CombatSpell extends MagicSpell {
    public CombatSpell(int spellId) { super(spellId); }
}
'''

ADAPTERS['core/game/node/entity/player/link/SpellBookManager.java'] = '''package core.game.node.entity.player.link;
import core.game.node.entity.combat.spell.MagicSpell;
// The book is a recorded read; spell lookups are recorded and answered from the case's registrations.
public class SpellBookManager {
    public enum SpellBook {
        MODERN(192), ANCIENT(193), LUNAR(430);
        private final int interfaceId;
        SpellBook(int interfaceId) { this.interfaceId = interfaceId; }
        public int getInterfaceId() { return interfaceId; }
        public MagicSpell getSpell(int buttonId) {
            oracle.Support.emit("spell_lookup", name(), buttonId);
            return oracle.Support.spell(name(), buttonId);
        }
    }
    public int getSpellBook() { oracle.Support.emit("spell_book"); return oracle.Support.book(); }
}
'''

ADAPTERS['core/game/node/entity/combat/CombatPulse.java'] = '''package core.game.node.entity.combat;
public class CombatPulse {
    public void updateStyle() { oracle.Support.emit("update_style"); }
    public boolean isAttacking() { oracle.Support.emit("is_attacking"); return oracle.Support.flag("attacking"); }
}
'''

ADAPTERS['core/game/node/entity/impl/Properties.java'] = '''package core.game.node.entity.impl;
import core.game.node.entity.combat.CombatPulse;
import core.game.node.entity.combat.equipment.WeaponInterface.AttackStyle;
import core.game.node.entity.combat.spell.CombatSpell;
import core.game.world.update.flag.context.Animation;
// Attack style, attack animation and autocast spell are case state; every access is recorded.
public class Properties {
    public AttackStyle attackStyle;
    public Animation attackAnimation;
    public CombatSpell autocastSpell;
    private final CombatPulse pulse = new CombatPulse();
    public AttackStyle getAttackStyle() { oracle.Support.emit("attack_style"); return attackStyle; }
    public void setAttackStyle(AttackStyle style) { oracle.Support.emit("set_attack_style", oracle.Support.style(style)); attackStyle = style; }
    public void setAttackAnimation(Animation animation) { oracle.Support.emit("set_attack_animation", oracle.Support.anim(animation)); attackAnimation = animation; }
    public CombatSpell getAutocastSpell() { oracle.Support.emit("autocast"); return autocastSpell; }
    public void setAutocastSpell(CombatSpell spell) {
        oracle.Support.emit("set_autocast", spell == null ? null : spell.getSpellId());
        autocastSpell = spell;
    }
    public CombatPulse getCombatPulse() { return pulse; }
    public int getCurrentCombatLevel() { oracle.Support.emit("combat_level"); return oracle.Support.combatLevel(); }
}
'''

ADAPTERS['core/game/node/entity/player/link/Settings.java'] = '''package core.game.node.entity.player.link;
// The attack style index and special bar toggle are case state; every access is recorded.
public class Settings {
    public int attackStyleIndex;
    public boolean specialToggled;
    public int getAttackStyleIndex() { oracle.Support.emit("style_index"); return attackStyleIndex; }
    public void toggleAttackStyleIndex(int index) { oracle.Support.emit("toggle_style_index", index); attackStyleIndex = index; }
    public boolean isSpecialToggled() { oracle.Support.emit("special_toggled"); return specialToggled; }
    public void toggleSpecialBar() { oracle.Support.emit("toggle_special_bar"); specialToggled = !specialToggled; }
}
'''

ADAPTERS['core/game/node/entity/player/link/appearance/Appearance.java'] = '''package core.game.node.entity.player.link.appearance;
public class Appearance {
    public boolean isNpc() { oracle.Support.emit("is_npc"); return oracle.Support.flag("npc"); }
    public void transformNPC(int id) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/player/link/InterfaceManager.java'] = '''package core.game.node.entity.player.link;
import core.game.component.Component;
// The tab array is case state (getTabs is recorded); opening a tab or the chatbox is recorded.
public class InterfaceManager {
    public final Component[] tabs = new Component[15];
    public Component[] getTabs() { oracle.Support.emit("tabs"); return tabs; }
    public boolean isResizable() { oracle.Support.emit("resizable"); return oracle.Support.flag("resizable"); }
    public void openTab(Component component) { oracle.Support.emit("open_tab", component.getId()); }
    public void openChatbox(Component component) { oracle.Support.emit("open_chatbox", component.getId()); }
    public void open(Component component) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/player/link/PacketDispatch.java'] = '''package core.game.node.entity.player.link;
// Each packet the classes send through PacketDispatch is recorded with its arguments.
public class PacketDispatch {
    public void sendInterfaceConfig(int interfaceId, int childId, boolean hide) { oracle.Support.emit("interface_config", interfaceId, childId, hide); }
    public void sendMessage(String message) { oracle.Support.emit("message", message); }
    public void sendString(String string, int interfaceId, int lineId) { oracle.Support.emit("string", string, interfaceId, lineId); }
    public void sendIfaceSettings(int settingsHash, int childId, int interfaceId, int offset, int length) {
        oracle.Support.emit("access_mask", settingsHash, childId, interfaceId, offset, length);
    }
    public void sendGraphic(int id) { oracle.Support.emit("graphic", id); }
    public void sendAnimation(int id) { oracle.Support.emit("animation", id); }
    public void sendRunScript(int id, String string, Object... objects) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/net/packet/out/Interface.java'] = 'package core.net.packet.out;\npublic final class Interface {}\n'
ADAPTERS['core/net/packet/out/InterfaceConfig.java'] = 'package core.net.packet.out;\npublic final class InterfaceConfig {}\n'
ADAPTERS['core/net/packet/out/StringPacket.java'] = 'package core.net.packet.out;\npublic final class StringPacket {}\n'
ADAPTERS['core/net/packet/out/MusicPacket.java'] = 'package core.net.packet.out;\npublic final class MusicPacket {}\n'

ADAPTERS['core/net/packet/Context.java'] = 'package core.net.packet;\npublic interface Context {}\n'

ADAPTERS['core/net/packet/context/InterfaceContext.java'] = '''package core.net.packet.context;
import core.game.node.entity.player.Player;
public final class InterfaceContext implements core.net.packet.Context {
    public final int windowId, componentId, interfaceId;
    public final boolean walkable;
    public InterfaceContext(Player player, int windowId, int componentId, int interfaceId, boolean walkable) {
        this.windowId = windowId; this.componentId = componentId; this.interfaceId = interfaceId; this.walkable = walkable;
    }
}
'''

ADAPTERS['core/net/packet/context/InterfaceConfigContext.java'] = '''package core.net.packet.context;
import core.game.node.entity.player.Player;
public final class InterfaceConfigContext implements core.net.packet.Context {
    public final int interfaceId, childId;
    public final boolean hidden;
    public InterfaceConfigContext(Player player, int interfaceId, int childId, boolean hidden) {
        this.interfaceId = interfaceId; this.childId = childId; this.hidden = hidden;
    }
}
'''

ADAPTERS['core/net/packet/context/StringContext.java'] = '''package core.net.packet.context;
import core.game.node.entity.player.Player;
public final class StringContext implements core.net.packet.Context {
    public final String string;
    public final int interfaceId, lineId;
    public StringContext(Player player, String string, int interfaceId, int lineId) { this.string = string; this.interfaceId = interfaceId; this.lineId = lineId; }
}
'''

ADAPTERS['core/net/packet/context/MusicContext.java'] = '''package core.net.packet.context;
import core.game.node.entity.player.Player;
public final class MusicContext implements core.net.packet.Context {
    public final int musicId;
    public final boolean secondary;
    public MusicContext(Player player, int musicId) { this(player, musicId, false); }
    public MusicContext(Player player, int musicId, boolean secondary) { this.musicId = musicId; this.secondary = secondary; }
}
'''

ADAPTERS['core/net/packet/PacketRepository.java'] = '''package core.net.packet;
import core.net.packet.context.*;
// The packet each send queues, recorded by its context's fields.
public final class PacketRepository {
    public static void send(Class<?> clazz, Context context) {
        if (context instanceof InterfaceContext) {
            InterfaceContext c = (InterfaceContext) context;
            oracle.Support.emit("interface", c.windowId, c.componentId, c.interfaceId, c.walkable);
        } else if (context instanceof InterfaceConfigContext) {
            InterfaceConfigContext c = (InterfaceConfigContext) context;
            oracle.Support.emit("interface_config", c.interfaceId, c.childId, c.hidden);
        } else if (context instanceof StringContext) {
            StringContext c = (StringContext) context;
            oracle.Support.emit("string", c.string, c.interfaceId, c.lineId);
        } else if (context instanceof MusicContext) {
            MusicContext c = (MusicContext) context;
            oracle.Support.emit("music", c.musicId, c.secondary);
        } else {
            throw new IllegalStateException("unknown packet " + clazz.getName());
        }
    }
}
'''

ADAPTERS['core/game/node/entity/skill/Skills.java'] = '''package core.game.node.entity.skill;
import core.game.node.entity.Entity;
// The original constants and skill names; every read and write is recorded, the milestones are case state.
public final class Skills {
    /* ORIGINAL_SKILL_CONSTANTS */
    /* ORIGINAL_SKILL_NAME */
    public int combatMilestone, skillMilestone;
    public Skills(Entity entity) {}
    public int getStaticLevel(int slot) { oracle.Support.emit("static_level", slot); return oracle.Support.staticLevel(slot); }
    public void incrementPrayerPoints(double amount) { oracle.Support.emit("prayer_points", oracle.Support.hex(amount)); }
    public int getTotalLevel() { oracle.Support.emit("total_level"); return oracle.Support.totalLevel(); }
    public int getCombatMilestone() { oracle.Support.emit("combat_milestone"); return combatMilestone; }
    public void setCombatMilestone(int value) { oracle.Support.emit("set_combat_milestone", value); combatMilestone = value; }
    public int getSkillMilestone() { oracle.Support.emit("skill_milestone"); return skillMilestone; }
    public void setSkillMilestone(int value) { oracle.Support.emit("set_skill_milestone", value); skillMilestone = value; }
}
'''

ADAPTERS['core/game/global/Skillcape.java'] = '''package core.game.global;
import core.game.node.entity.player.Player;
public final class Skillcape {
    public static void trim(Player player) { oracle.Support.emit("skillcape_trim"); }
}
'''

ADAPTERS['core/game/world/repository/Repository.java'] = '''package core.game.world.repository;
public final class Repository {
    public static void sendNews(String string) { oracle.Support.emit("news", string, null, null); }
    public static void sendNews(String string, int icon, String color) { oracle.Support.emit("news", string, icon, color); }
}
'''

ADAPTERS['core/game/node/entity/lock/Locks.java'] = '''package core.game.node.entity.lock;
// Recorded lock queries and locks; the emote lock's state is the case's.
public class Locks {
    public boolean isLocked(String key) { oracle.Support.emit("is_locked", key); return oracle.Support.flag("emoteLocked"); }
    public void lock(String key, int ticks) { oracle.Support.emit("lock", key, ticks); }
    public void lock(int ticks) { oracle.Support.emit("lock_all", ticks); }
}
'''

ADAPTERS['core/game/node/entity/player/info/Rights.java'] = '''package core.game.node.entity.player.info;
public enum Rights { REGULAR_PLAYER, PLAYER_MODERATOR, ADMINISTRATOR }
'''

ADAPTERS['core/game/node/entity/player/link/diary/DiaryType.java'] = '''package core.game.node.entity.player.link.diary;
public enum DiaryType { KARAMJA, VARROCK, LUMBRIDGE, FALADOR, FREMENNIK, SEERS_VILLAGE, ARDOUGNE }
'''

ADAPTERS['core/game/node/entity/player/link/diary/AchievementDiary.java'] = '''package core.game.node.entity.player.link.diary;
import core.game.node.entity.player.Player;
public class AchievementDiary {
    private final DiaryType type;
    public AchievementDiary(DiaryType type) { this.type = type; }
    public boolean isComplete(int level, int index) {
        oracle.Support.emit("diary_complete", type.name(), level, index);
        return oracle.Support.flag("diaryComplete");
    }
    public void updateTask(Player player, int level, int index, boolean complete) { oracle.Support.emit("diary_update", type.name(), level, index, complete); }
}
'''

ADAPTERS['core/game/node/entity/player/link/diary/AchievementDiaryManager.java'] = '''package core.game.node.entity.player.link.diary;
import core.game.node.entity.player.Player;
public class AchievementDiaryManager {
    public AchievementDiary getDiary(DiaryType type) { return new AchievementDiary(type); }
    public boolean hasCompletedTask(DiaryType type, int level, int index) { throw new IllegalStateException("not driven"); }
    public void finishTask(Player player, DiaryType type, int level, int index) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/dialogue/DialogueInterpreter.java'] = '''package core.game.dialogue;
public class DialogueInterpreter {
    public void sendDialogue(String... messages) { oracle.Support.emit("dialogue", java.util.Arrays.asList(messages)); }
    public boolean open(int id, Object... args) { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/player/link/PulseManager.java'] = '''package core.game.node.entity.player.link;
public class PulseManager { public void clear() { oracle.Support.emit("pulse_clear"); } }
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.node.Node;
import core.game.node.entity.impl.Properties;
import core.game.node.entity.skill.Skills;
import java.util.HashMap;
import java.util.Map;
// Attributes are case state with the original /save: prefix handling; every access is recorded.
public abstract class Entity extends Node {
    public final Map<String, Object> attributes = new HashMap<>();
    public Skills skills = new Skills(this);
    private final Properties properties = new Properties();
    public Skills getSkills() { return skills; }
    public Properties getProperties() { return properties; }
    public void setAttribute(String key, Object value) {
        oracle.Support.emit("set_attribute", key, oracle.Support.value(value));
        attributes.put(key.startsWith("/save:") ? key.substring(6) : key, value);
    }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        oracle.Support.emit("get_attribute", key, oracle.Support.value(fail));
        Object v = attributes.get(key.replace("/save:", ""));
        return v == null ? fail : (T) v;
    }
    public void removeAttribute(String key) { oracle.Support.emit("remove_attribute", key); attributes.remove(key); }
}
'''

ADAPTERS['core/game/node/entity/npc/NPC.java'] = '''package core.game.node.entity.npc;
public class NPC extends core.game.node.entity.Entity {
    public int getId() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
import core.game.container.impl.EquipmentContainer;
import core.game.dialogue.DialogueInterpreter;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.lock.Locks;
import core.game.node.entity.player.info.Rights;
import core.game.node.entity.player.link.*;
import core.game.node.entity.player.link.appearance.Appearance;
import core.game.node.entity.player.link.diary.AchievementDiaryManager;
import core.game.node.entity.player.link.emote.EmoteManager;
import core.game.node.entity.player.link.music.MusicPlayer;
import core.game.node.entity.player.link.quest.QuestRepository;
import core.game.world.map.Direction;
// The player of the case. Its managers are the unchanged EmoteManager, MusicPlayer and QuestRepository; everything else
// is a recording stand-in.
public class Player extends core.game.node.entity.Entity {
    private final InterfaceManager interfaceManager = new InterfaceManager();
    private final EquipmentContainer equipment = new EquipmentContainer();
    private final Settings settings = new Settings();
    private final Appearance appearance = new Appearance();
    private final SpellBookManager spellBookManager = new SpellBookManager();
    private final PacketDispatch packetDispatch = new PacketDispatch();
    private final Locks locks = new Locks();
    private final Animator animator = new Animator();
    private final PulseManager pulseManager = new PulseManager();
    private final AchievementDiaryManager diaries = new AchievementDiaryManager();
    private final DialogueInterpreter dialogue = new DialogueInterpreter();
    public EmoteManager emoteManager;
    public MusicPlayer musicPlayer;
    public QuestRepository questRepository;
    public InterfaceManager getInterfaceManager() { return interfaceManager; }
    public EquipmentContainer getEquipment() { return equipment; }
    public Settings getSettings() { return settings; }
    public Appearance getAppearance() { return appearance; }
    public SpellBookManager getSpellBookManager() { return spellBookManager; }
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public Locks getLocks() { return locks; }
    public Animator getAnimator() { return animator; }
    public PulseManager getPulseManager() { return pulseManager; }
    public AchievementDiaryManager getAchievementDiaryManager() { return diaries; }
    public DialogueInterpreter getDialogueInterpreter() { return dialogue; }
    public EmoteManager getEmoteManager() { return emoteManager; }
    public MusicPlayer getMusicPlayer() { return musicPlayer; }
    public QuestRepository getQuestRepository() { return questRepository; }
    public void addExtension(Class<?> clazz, Object instance) { oracle.Support.emit("add_extension", clazz.getSimpleName()); }
    public boolean isArtificial() { return oracle.Support.flag("artificial"); }
    public String getUsername() { return oracle.Support.username(); }
    public String getName() { return oracle.Support.username(); }
    public boolean isDebug() { oracle.Support.emit("is_debug"); return oracle.Support.flag("debug"); }
    public void debug(String string) { oracle.Support.emit("debug", string); }
    public Rights getRights() { oracle.Support.emit("rights"); return Rights.valueOf(oracle.Support.rights()); }
    public boolean inCombat() { oracle.Support.emit("in_combat"); return oracle.Support.flag("inCombat"); }
    public void lock(int ticks) { throw new IllegalStateException("not driven"); }
    public Direction getDirection() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/api/ContentAPI.kt'] = '''package core.api
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.repository.Repository
import core.tools.Log
// getAttribute, setAttribute, removeAttribute(s), setComponentVisibility, getStatLevel and sendNews are original text;
// varps, varbits, jingles and logging are recorded.
/* ORIGINAL_CONTENT_API */

@JvmOverloads
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("varp", varpIndex, value, save)
}

@JvmOverloads
fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("varbit", varbitId, value, save)
}

fun playJingle(player: Player, jingleId: Int) {
    oracle.Support.emit("music", jingleId, true)
}

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", origin.getName(), type.name, message)
}
'''

ADAPTERS['oracle/CaseQuest.java'] = '''package oracle;
import content.data.Quests;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.quest.Quest;
// A registered quest of the case. A quest marked custom overrides getConfig (recorded, answered from the case);
// every quest records its updateVarps, the content member syncronizeTab calls.
public class CaseQuest extends Quest {
    private final boolean custom;
    private final java.util.List<Object> customConfig;
    public CaseQuest(Quests quest, int index, int buttonId, int points, int[] configs, boolean custom, java.util.List<Object> customConfig) {
        super(quest, index, buttonId, points, configs);
        this.custom = custom;
        this.customConfig = customConfig;
    }
    @Override public Quest newInstance(Object object) { return this; }
    @Override public int[] getConfig(Player player, int stage) {
        if (!custom) return super.getConfig(player, stage);
        Support.emit("custom_config", getQuest().name(), stage);
        int[] out = new int[customConfig.size()];
        for (int i = 0; i < out.length; i++) out[i] = ((Number) customConfig.get(i)).intValue();
        return out;
    }
    @Override public void updateVarps(Player player) { Support.emit("update_varps", getQuest().name()); }
}
'''
