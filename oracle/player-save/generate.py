"""Drive the unmodified PlayerSaver.kt and PlayerSaveParser.kt over whole character saves.

Both original files compile unchanged with Kotlin 1.8.20 and run on JDK 11.
Their collaborators are, in order of preference: complete unchanged original
classes, verbatim selected members of larger original classes, and recording
adapters for systems that have no native port. --check never writes fixtures.
"""
import argparse, hashlib, importlib.util, json, os, pathlib, random, re, shutil, subprocess, tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/player-save-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
LOGIN = 'core/game/node/entity/player/info/login/'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


timers = load('timers', ROOT / 'oracle/timers/generate.py')
inventory = load('inventory', ROOT / 'oracle/inventory/generate.py')
compiler = timers.compiler
read, run, method = compiler.read, compiler.run, compiler.method


def field(text, name):
    """One original field declaration, verbatim, selected by its name."""
    found = re.search(r'^[ \t]*(?:public|private|protected)[^\n;(]*\b' + re.escape(name) + r'\b(?:[ \t]*=[^;]+)?;', text, re.M)
    if not found:
        raise RuntimeError('Original field not found: ' + name)
    return found.group(0) + '\n'


def members(text, fields, methods):
    missing = [signature for signature in methods if signature not in text]
    if missing:
        raise RuntimeError('Original members not found: ' + '; '.join(missing))
    return ''.join(field(text, name) for name in fields) + '\n'.join(method(text, signature) for signature in methods) + '\n'


# Recording adapters for systems PlayerSaver/PlayerSaveParser reach but that have
# no native port. They hold exactly the values the saver reads through getters
# and record what the parser hands over. None of them models content behaviour.
ADAPTERS = {
'core/tools/Log.java': '''package core.tools;
/** One class for three callers: the timer classes (INSTANCE getter), the item classes (static int) and the profile files. */
public final class Log {
    public static final Log INSTANCE = new Log();
    public static int ERR = 1;
    public static int WARN = 2;
    public int getERR() { return ERR; }
}''',
'core/api/ContentAPIKt.java': '''package core.api;
public class ContentAPIKt {
    public static void log(Object origin, int level, String text) { throw new AssertionError(text); }
    public static void setVarbit(core.game.node.entity.player.Player player, int id, int value) { throw new UnsupportedOperationException("setVarbit is outside the player save oracle"); }
}''',
'core/api/ContentInterface.java': 'package core.api;\npublic interface ContentInterface {}',
'core/api/ProfileLog.kt': '''package core.api
/** PlayerSaver/PlayerSaveParser log calls, kept apart from the timer classes' own log adapter. */
fun log(origin: Class<*>, type: Int, message: String) {
    oracle.Rec.call("log", if (type == core.tools.Log.ERR) "ERR" else "WARN", message)
}''',
'core/game/node/entity/player/info/login/System.java': '''package core.game.node.entity.player.info.login;
/** Same-package clock for the two profile files; they use no other System member. */
public class System {
    public static long currentTimeMillis() { return oracle.Rec.now(); }
}''',
'core/game/node/entity/player/link/System.java': '''package core.game.node.entity.player.link;
public class System {
    public static long currentTimeMillis() { return oracle.Rec.now(); }
}''',
'core/game/node/entity/combat/graves/GraveController.kt': '''package core.game.node.entity.combat.graves
import core.game.node.entity.player.Player
object GraveController {
    fun updateGraveType(player: Player, type: GraveType) { oracle.Rec.call("grave", type.ordinal) }
}''',
'core/game/node/entity/player/info/PlayerDetails.java': None,
'core/game/node/entity/npc/NPC.java': '''package core.game.node.entity.npc;
public class NPC extends core.game.node.entity.Entity {
    public Definition getDefinition() { throw new UnsupportedOperationException("NPC definitions are outside the player save oracle"); }
    public static class Definition { public int getCombatLevel() { throw new UnsupportedOperationException(); } }
}''',
'core/net/packet/PacketRepository.java': '''package core.net.packet;
public class PacketRepository {
    public static void send(Class<?> type, core.net.packet.context.SkillContext context) { oracle.Rec.call("packet", context.skill); }
}''',
'core/net/packet/context/SkillContext.java': '''package core.net.packet.context;
public class SkillContext {
    public final core.game.node.entity.player.Player player;
    public final int skill;
    public SkillContext(core.game.node.entity.player.Player player, int skill) { this.player = player; this.skill = skill; }
}''',
'core/net/packet/out/SkillLevel.java': 'package core.net.packet.out;\npublic class SkillLevel {}',
'core/game/component/Component.java': 'package core.game.component;\npublic class Component { public Component(int id) {} }',
'core/game/node/entity/player/InterfaceManager.java': '''package core.game.node.entity.player;
public class InterfaceManager { public void openTab(core.game.component.Component component) { throw new UnsupportedOperationException(); } }''',
'core/game/node/entity/combat/spell/MagicSpell.java': '''package core.game.node.entity.combat.spell;
import core.game.node.entity.player.link.SpellBookManager.SpellBook;
public class MagicSpell {
    protected final SpellBook book;
    protected int spellId;
    public MagicSpell(SpellBook book) { this.book = book; }
    public SpellBook getBook() { return book; }
    public int getSpellId() { return spellId; }
    public void setSpellId(int spellId) { this.spellId = spellId; }
}''',
'core/game/node/entity/combat/spell/CombatSpell.java': '''package core.game.node.entity.combat.spell;
import core.game.node.entity.player.link.SpellBookManager.SpellBook;
public class CombatSpell extends MagicSpell { public CombatSpell(SpellBook book) { super(book); } }''',
'content/global/skill/farming/FarmingPatch.java': 'package content.global.skill.farming;\npublic class FarmingPatch {}',
'core/game/event/VarbitUpdateEvent.java': 'package core.game.event;\npublic class VarbitUpdateEvent {}',
'core/cache/def/impl/VarbitDefinition.java': 'package core.cache.def.impl;\npublic class VarbitDefinition {}',
'content/global/skill/runecrafting/PouchManager.java': '''package content.global.skill.runecrafting;
import org.json.simple.*;
public class PouchManager {
    public PouchManager(core.game.node.entity.player.Player player) {}
    public void save(JSONObject root) {
        oracle.Rec.call("pouches.save");
        for (kotlin.Pair<String, String> entry : oracle.Rec.INSTANCE.getPouches()) root.put(entry.getFirst(), oracle.Rec.json(entry.getSecond()));
    }
    public void parse(JSONArray data) { oracle.Rec.call("pouches.parse", data.toJSONString()); }
}''',
'content/global/skill/construction/decoration/pohstorage/StorageState.java': '''package content.global.skill.construction.decoration.pohstorage;
import org.json.simple.*;
public class StorageState {
    public StorageState(core.game.node.entity.player.Player player) {}
    public JSONObject toJson() { oracle.Rec.call("poh.save"); return (JSONObject) oracle.Rec.json(oracle.Rec.INSTANCE.getPoh()); }
    public void readJson(JSONObject data) { oracle.Rec.call("poh.parse", data.toJSONString()); }
}''',
'core/game/node/entity/player/link/diary/DiaryType.java': None,
'core/game/node/entity/player/link/diary/AchievementDiary.java': '''package core.game.node.entity.player.link.diary;
public class AchievementDiary {
    private final DiaryType type;
    private final boolean[] levelStarted, levelRewarded;
    private final boolean[][] taskCompleted;
    public AchievementDiary(DiaryType type, boolean[] started, boolean[][] completed, boolean[] rewarded) { this.type = type; levelStarted = started; taskCompleted = completed; levelRewarded = rewarded; }
    public boolean[][] getTaskCompleted() { return taskCompleted; }
    public DiaryType getType() { return type; }
    public boolean[] getLevelStarted() { return levelStarted; }
    public boolean[] getLevelRewarded() { return levelRewarded; }
}''',
'core/game/node/entity/player/link/diary/AchievementDiaryManager.java': '''package core.game.node.entity.player.link.diary;
import org.json.simple.JSONArray;
public class AchievementDiaryManager {
    private AchievementDiary[] diarys = new AchievementDiary[0];
    public AchievementDiaryManager(core.game.node.entity.player.Player player) {}
    public void script(AchievementDiary[] diarys) { this.diarys = diarys; }
    public void parse(JSONArray data) { oracle.Rec.call("diary.parse", data.toJSONString()); }
    public void resetRewards() { oracle.Rec.call("diary.reset"); }
    public AchievementDiary[] getDiarys() { return diarys; }
}''',
'core/game/node/entity/player/link/emote/Emotes.java': None,
'core/game/node/entity/player/link/quest/QuestRepository.java': None,
'core/game/node/entity/player/link/music/MusicPlayer.java': None,
'content/global/skill/construction/Ordinal.java': '''package content.global.skill.construction;
/** Stands in for the house enums; the saver only reads their ordinals. */
public class Ordinal {
    private final int ordinal;
    public Ordinal(int ordinal) { this.ordinal = ordinal; }
    public int getOrdinal() { return ordinal; }
}''',
'core/game/world/map/Direction.java': '''package core.game.world.map;
public class Direction {
    private final int value;
    public Direction(int value) { this.value = value; }
    public int toInteger() { return value; }
}''',
'content/global/skill/construction/Hotspot.java': '''package content.global.skill.construction;
public class Hotspot {
    private final int decorationIndex;
    public Hotspot(int decorationIndex) { this.decorationIndex = decorationIndex; }
    public int getDecorationIndex() { return decorationIndex; }
}''',
'content/global/skill/construction/Room.java': '''package content.global.skill.construction;
import core.game.world.map.Direction;
public class Room {
    private final Ordinal properties;
    private final Hotspot[] hotspots;
    private final Direction rotation;
    public Room(int properties, int rotation, Hotspot[] hotspots) { this.properties = new Ordinal(properties); this.rotation = new Direction(rotation); this.hotspots = hotspots; }
    public Hotspot[] getHotspots() { return hotspots; }
    public Ordinal getProperties() { return properties; }
    public Direction getRotation() { return rotation; }
}''',
'content/global/skill/construction/Servant.java': '''package content.global.skill.construction;
import core.game.node.item.Item;
public class Servant {
    private final Ordinal type;
    private final Item item;
    private final int uses;
    private final boolean greet;
    public Servant(int type, int uses, Item item, boolean greet) { this.type = new Ordinal(type); this.uses = uses; this.item = item; this.greet = greet; }
    public Item getItem() { return item; }
    public int getUses() { return uses; }
    public boolean isGreet() { return greet; }
    public Ordinal getType() { return type; }
}''',
'content/global/skill/construction/HouseManager.java': None,
'content/global/skill/summoning/familiar/Familiar.java': '''package content.global.skill.summoning.familiar;
public class Familiar extends core.game.node.entity.npc.NPC {
    private int originalId;
    protected int ticks;
    protected int specialPoints = 60;
    public void script(int originalId, int ticks, int specialPoints) { this.originalId = originalId; this.ticks = ticks; this.specialPoints = specialPoints; }
    public int getOriginalId() { return originalId; }
    /*ORIGINAL*/
}''',
'content/global/skill/summoning/familiar/BurdenBeast.java': '''package content.global.skill.summoning.familiar;
import core.game.container.Container;
public class BurdenBeast extends Familiar {
    protected Container container;
    public BurdenBeast(Container container) { this.container = container; }
    public boolean isBurdenBeast() { return true; }
    /*ORIGINAL*/
}''',
'content/global/skill/summoning/pet/Pet.java': '''package content.global.skill.summoning.pet;
public final class Pet extends content.global.skill.summoning.familiar.Familiar {
    private final int itemId;
    public Pet(int itemId) { this.itemId = itemId; }
    /*ORIGINAL*/
}''',
'content/global/skill/summoning/familiar/FamiliarManager.java': None,
'core/game/container/impl/RecordingListener.java': '''package core.game.container.impl;
import core.game.container.*;
public class RecordingListener implements ContainerListener {
    public void update(Container c, ContainerEvent e) { throw new AssertionError("Profile code should not fire container updates"); }
    public void refresh(Container c) { throw new AssertionError("Profile code should not refresh containers"); }
}''',
}
for _name in ['BankListener', 'EquipmentListener', 'InventoryListener']:
    ADAPTERS['core/game/container/impl/' + _name + '.java'] = 'package core.game.container.impl;\npublic class ' + _name + ' extends RecordingListener { public ' + _name + '(core.game.node.entity.player.Player player) {} }'


def enum_names(text, pattern, expected):
    names = re.findall(pattern, text, re.M)
    if len(names) != expected or len(set(names)) != expected:
        raise RuntimeError('Original enum constants changed; review the adapter')
    return names


def adapters():
    """Fill the adapters that carry verbatim original members."""
    out = dict(ADAPTERS)
    src = lambda path: read(SERVER / path)
    constants = src('core/ServerConstants.kt')
    out['core/ServerConstants.kt'] = 'package core\nclass ServerConstants {\n    companion object {\n' + ''.join(
        '        @JvmField\n        ' + re.search(r'var ' + name + r'\b[^\n]*', constants).group(0) + '\n'
        for name in ['CURRENT_SAVEFILE_VERSION', 'PLAYER_SAVE_PATH', 'PLAYER_ATTRIBUTE_PATH', 'BANK_SIZE', 'JAVA_VERSION']) + '    }\n}\n'
    location = src('core/game/world/map/Location.kt')
    out['core/game/world/map/Location.kt'] = ('package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);'
        'var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};'
        'override fun toString()="[$x, $y, $z]";companion object {fun create(x:Int,y:Int,z:Int)=Location(x,y,z);' + method(location, 'fun fromString(') + '}\n@JvmInline '
        + method(location, 'value class LocationHash(') + '}')
    entity = src('core/game/node/entity/Entity.java')
    out['core/game/node/entity/Entity.java'] = ('package core.game.node.entity;\nimport core.game.node.entity.impl.*;\nimport core.game.node.entity.skill.Skills;\nimport core.game.system.timer.TimerManager;\n'
        'import core.game.world.map.Location;\nimport java.util.Map;\npublic class Entity {\n    private final Properties properties = new Properties();\n'
        + field(entity, 'attributes') + field(entity, 'timers') +
        '    private final Scripts scripts = new Scripts();\n    public Skills skills = new Skills(this);\n    private Location location;\n    private String name = "entity";\n'
        + method(entity, 'public Properties getProperties()') + method(entity, 'public Map<String, Object> getAttributes()') + method(entity, 'public GameAttributes getGameAttributes()') + method(entity, 'public Skills getSkills()') +
        '    public TimerManager getTimers() { return timers; }\n    public Scripts getScripts() { return scripts; }\n    public Location getLocation() { return location; }\n'
        '    public void setLocation(Location location) { this.location = location; }\n    public String getName() { return name; }\n    public void setName(String name) { this.name = name; }\n'
        '    public core.game.node.entity.player.Player asPlayer() { return (core.game.node.entity.player.Player) this; }\n}\n')
    properties = src('core/game/node/entity/impl/Properties.java')
    out['core/game/node/entity/impl/Properties.java'] = ('package core.game.node.entity.impl;\nimport core.game.node.entity.combat.spell.CombatSpell;\npublic class Properties {\n'
        + members(properties, ['retaliating', 'autocastSpell'], ['public boolean isRetaliating()', 'public void setRetaliating(', 'public CombatSpell getAutocastSpell()', 'public void setAutocastSpell(']) +
        '    private int combatLevel;\n    public int getCombatLevel() { return combatLevel; }\n    public void setCombatLevel(int level) { oracle.Rec.call("combat", level); combatLevel = level; }\n}\n')
    details = src('core/game/node/entity/player/info/PlayerDetails.java')
    out['core/game/node/entity/player/info/PlayerDetails.java'] = 'package core.game.node.entity.player.info;\npublic class PlayerDetails {\n' + field(details, 'saveParsed') + '}\n'
    skills = src('core/game/node/entity/skill/Skills.java')
    out['core/game/node/entity/skill/Skills.java'] = ('package core.game.node.entity.skill;\nimport core.ServerConstants;\nimport core.game.node.entity.Entity;\nimport core.game.node.entity.npc.NPC;\n'
        'import core.game.node.entity.player.Player;\nimport core.net.packet.PacketRepository;\nimport core.net.packet.context.SkillContext;\nimport core.net.packet.out.SkillLevel;\n'
        'import org.json.simple.JSONArray;\nimport org.json.simple.JSONObject;\nimport static java.lang.Math.floor;\nimport static java.lang.Math.max;\npublic final class Skills {\n'
        + members(skills, ['experienceMultiplier', 'ATTACK', 'entity', 'experience', 'staticLevels', 'dynamicLevels', 'prayerPoints', 'lifepoints', 'lifepointsIncrease', 'experienceGained', 'lifepointsUpdate', 'combatMilestone', 'skillMilestone'],
            ['public Skills(Entity entity)', 'public void parse(JSONArray skillData)', 'public void correct(double divisor)', 'public boolean updateCombatLevel()', 'private int calculateCombatLevel()',
             'public double getExperience(int slot)', 'public void setExperienceGained(', 'public double getExperienceGained()', 'public int getLifepoints()', 'public void setLifepoints(',
             'public double getPrayerPoints()', 'public void setPrayerPoints(', 'public int[] getStaticLevels()', 'public int[] getDynamicLevels()', 'public int getCombatMilestone()',
             'public void setCombatMilestone(', 'public int getSkillMilestone()', 'public void setSkillMilestone(', 'public int getStaticLevelByExperience(int slot)']) + '}\n')
    settings = src('core/game/node/entity/player/link/Settings.java')
    setting_fields = ['player', 'runEnergy', 'weight', 'brightness', 'musicVolume', 'soundEffectVolume', 'areaSoundVolume', 'singleMouseButton', 'disableChatEffects', 'splitPrivateChat', 'acceptAid', 'runToggled',
        'publicChatSetting', 'privateChatSetting', 'clanChatSetting', 'tradeSetting', 'assistSetting', 'specialToggled', 'specialEnergy', 'attackStyleIndex']
    setting_getters = ['public int getSpecialEnergy()', 'public int getBrightness()', 'public int getMusicVolume()', 'public int getSoundEffectVolume()', 'public int getAreaSoundVolume()',
        'public boolean isSingleMouseButton()', 'public boolean isDisableChatEffects()', 'public boolean isSplitPrivateChat()', 'public boolean isAcceptAid()', 'public boolean isRunToggled()',
        'public int getPublicChatSetting()', 'public int getPrivateChatSetting()', 'public int getClanChatSetting()', 'public int getTradeSetting()', 'public int getAssistSetting()',
        'public double getRunEnergy()', 'public int getAttackStyleIndex()']
    out['core/game/node/entity/player/link/Settings.java'] = ('package core.game.node.entity.player.link;\nimport core.game.node.entity.player.Player;\nimport org.json.simple.JSONObject;\npublic final class Settings {\n'
        + members(settings, setting_fields, ['public Settings(Player player)', 'public void parse(JSONObject settingsData)'] + setting_getters) + '}\n')
    appearance = src('core/game/node/entity/player/link/appearance/Appearance.java')
    out['core/game/node/entity/player/link/appearance/Appearance.java'] = ('package core.game.node.entity.player.link.appearance;\nimport core.game.node.entity.player.Player;\nimport org.json.simple.JSONArray;\nimport org.json.simple.JSONObject;\npublic final class Appearance {\n'
        + members(appearance, ['player', 'appearanceCache', 'gender'], ['public Appearance(final Player player)', 'public void parse(JSONObject appearance)', 'public Gender getGender()', 'public BodyPart[] getAppearanceCache()']) + '}\n')
    pin = src('core/game/node/entity/player/link/BankPinManager.java')
    out['core/game/node/entity/player/link/BankPinManager.java'] = ('package core.game.node.entity.player.link;\nimport core.game.node.entity.player.Player;\nimport org.json.simple.JSONObject;\npublic class BankPinManager {\n'
        + members(pin, ['player', 'pin', 'longRecovery', 'status', 'pendingDelay', 'tryDelay'], ['public void parse(JSONObject data)', 'public boolean hasPin()', 'public String getPin()',
            'public boolean isLongRecovery()', 'public long getTryDelay()', 'public PinStatus getStatus()', 'public long getPendingDelay()']) +
        # The original constructor also fills the keypad digit list and each status constant carries interface drawing code.
        '    public BankPinManager(final Player player) { this.player = player; }\n    public enum PinStatus { ' + ', '.join(enum_names(method(pin, 'public enum PinStatus'), r'^\t\t([A-Z_]+)\(\) \{', 3)) + ' }\n}\n')
    emotes = src('core/game/node/entity/player/link/emote/Emotes.java')
    out['core/game/node/entity/player/link/emote/Emotes.java'] = 'package core.game.node.entity.player.link.emote;\npublic enum Emotes { ' + ', '.join(enum_names(emotes, r'^    ([A-Z][A-Z_0-9]*)\(', 45)) + ' }\n'
    manager = src('core/game/node/entity/player/link/emote/EmoteManager.java')
    out['core/game/node/entity/player/link/emote/EmoteManager.java'] = ('package core.game.node.entity.player.link.emote;\nimport core.game.node.entity.player.Player;\nimport java.util.ArrayList;\nimport java.util.List;\npublic class EmoteManager {\n'
        + members(manager, ['emotes', 'player'], ['public EmoteManager(Player player)', 'public boolean isSaveRequired()', 'public List<Emotes> getEmotes()']) + '}\n')
    diary = src('core/game/node/entity/player/link/diary/DiaryType.java')
    out['core/game/node/entity/player/link/diary/DiaryType.java'] = ('package core.game.node.entity.player.link.diary;\n/** Constant names and order only. getName deliberately differs from name() so the fixture shows which one the saver writes. */\npublic enum DiaryType {\n    '
        + ', '.join(enum_names(diary, r'^\s+([A-Z][A-Z_]*)\("', 6)) + ';\n    public String getName() { return "display:" + name(); }\n}\n')
    quests = src('core/game/node/entity/player/link/quest/QuestRepository.java')
    out['core/game/node/entity/player/link/quest/QuestRepository.java'] = ('package core.game.node.entity.player.link.quest;\nimport core.game.node.entity.player.Player;\nimport org.json.simple.JSONObject;\nimport java.util.HashMap;\nimport java.util.Map;\npublic final class QuestRepository {\n'
        + members(quests, ['quests', 'points'], ['public int getPoints()', 'public Map<Integer, Integer> getQuestList()']) +
        '    public QuestRepository(final Player player) {}\n    public void script(int points) { this.points = points; }\n    public void parse(JSONObject questData) { oracle.Rec.call("quests.parse", questData.toJSONString()); }\n}\n')
    music = src('core/game/node/entity/player/link/music/MusicPlayer.java')
    out['core/game/node/entity/player/link/music/MusicPlayer.java'] = ('package core.game.node.entity.player.link.music;\nimport core.game.node.entity.player.Player;\nimport java.util.HashMap;\nimport java.util.Map;\npublic final class MusicPlayer {\n'
        + members(music, ['unlocked'], ['public Map<Integer, MusicEntry> getUnlocked()']) + '    public MusicPlayer(Player player) { this.unlocked = new HashMap<>(); }\n}\n')
    house = src('content/global/skill/construction/HouseManager.java')
    out['content/global/skill/construction/HouseManager.java'] = ('package content.global.skill.construction;\nimport org.json.simple.JSONObject;\npublic final class HouseManager {\n'
        + members(house, ['rooms', 'servant'], ['public boolean hasServant()', 'public Room[][][] getRooms()', 'public Servant getServant()']) +
        '    private Ordinal location = new Ordinal(0), style = new Ordinal(0), crest = new Ordinal(0);\n    public HouseManager() {}\n'
        '    public void script(int location, int style, int crest) { this.location = new Ordinal(location); this.style = new Ordinal(style); this.crest = new Ordinal(crest); }\n'
        '    public void setServant(Servant servant) { this.servant = servant; }\n    public Ordinal getLocation() { return location; }\n    public Ordinal getStyle() { return style; }\n    public Ordinal getCrest() { return crest; }\n'
        '    public void parse(JSONObject data) { oracle.Rec.call("house.parse", data.toJSONString()); }\n}\n')
    familiar = src('content/global/skill/summoning/familiar/Familiar.java')
    out['content/global/skill/summoning/familiar/Familiar.java'] = out['content/global/skill/summoning/familiar/Familiar.java'].replace('/*ORIGINAL*/',
        method(familiar, 'public boolean isBurdenBeast()') + method(familiar, 'public int getTicks()') + method(familiar, 'public int getSpecialPoints()'))
    burden = src('content/global/skill/summoning/familiar/BurdenBeast.java')
    out['content/global/skill/summoning/familiar/BurdenBeast.java'] = out['content/global/skill/summoning/familiar/BurdenBeast.java'].replace('/*ORIGINAL*/', method(burden, 'public Container getContainer()'))
    pet = src('content/global/skill/summoning/pet/Pet.java')
    out['content/global/skill/summoning/pet/Pet.java'] = out['content/global/skill/summoning/pet/Pet.java'].replace('/*ORIGINAL*/', method(pet, 'public int getItemId()'))
    manager = src('content/global/skill/summoning/familiar/FamiliarManager.java')
    out['content/global/skill/summoning/familiar/FamiliarManager.java'] = ('package content.global.skill.summoning.familiar;\nimport content.global.skill.summoning.pet.*;\nimport core.game.node.entity.player.Player;\n'
        'import org.json.simple.JSONObject;\nimport java.util.*;\npublic final class FamiliarManager {\n'
        + members(manager, ['petDetails', 'familiar', 'summoningCombatLevel'], ['public boolean hasFamiliar()', 'public boolean hasPet()', 'public Familiar getFamiliar()', 'public Map<Integer, ArrayList<PetDetails>> getPetDetails()', 'public int getSummoningCombatLevel()']) +
        '    public FamiliarManager(Player player) {}\n    public void script(Familiar familiar) { this.familiar = familiar; }\n'
        '    public void setSummoningCombatLevel(int level) { oracle.Rec.call("summon", level); summoningCombatLevel = level; }\n'
        '    public void parse(JSONObject familiarData) { oracle.Rec.call("familiar.parse", familiarData.toJSONString()); }\n}\n')
    god = src('content/data/GodBook.java')
    names = ['HOLY_BOOK', 'BOOK_OF_BALANCE', 'UNHOLY_BOOK']
    if not all(name + '(' in god for name in names):
        raise RuntimeError('GodBook constants changed')
    out['content/data/GodBook.java'] = 'package content.data;\npublic enum GodBook { ' + ', '.join(names) + ' }\n'
    grave = src('core/game/node/entity/combat/graves/GraveType.kt')
    npcs = sorted(set(re.findall(r'NPCs\.([A-Z_0-9]+)', grave)))
    out['org/rs09/consts/NPCs.java'] = 'package org.rs09.consts;\npublic class NPCs {\n' + ''.join('    public static final int ' + name + ' = ' + re.search(r'_(\d+)$', name).group(1) + ';\n' for name in npcs) + '}\n'
    out['content/data/Quests.java'] = 'package content.data;\npublic enum Quests { ' + ', '.join(sorted(set(re.findall(r'Quests\.([A-Z_]+)', grave)))) + ' }\n'
    config = src('core/game/system/config/ItemConfigParser.kt')
    out['core/game/system/config/ItemConfigParser.java'] = 'package core.game.system.config;\npublic class ItemConfigParser {' + ''.join(
        'public static String ' + name + '=' + re.search(r'const val ' + name + r' = ("[^"]+")', config).group(1) + ';' for name in ['HIGH_ALCHEMY', 'LOW_ALCHEMY']) + '}\n'
    bank = src('core/game/container/impl/BankContainer.java')
    out['core/game/container/impl/BankContainer.java'] = ('package core.game.container.impl;import core.game.container.*;import core.game.node.entity.player.Player;import core.ServerConstants;public class BankContainer extends Container {'
        + bank[bank.index('public static final int SIZE'):bank.index('public BankContainer(Player player)')] + method(bank, 'public BankContainer(Player player)') + method(bank, 'public int[] getTabStartSlot()') + '}')
    equipment = src('core/game/container/impl/EquipmentContainer.java')
    out['core/game/container/impl/EquipmentContainer.java'] = ('package core.game.container.impl;import core.game.container.*;import core.game.node.entity.player.Player;public class EquipmentContainer extends Container {private final Player player;'
        + method(equipment, 'public EquipmentContainer(Player player)') + '}')
    player = src('core/game/node/entity/player/Player.java')
    player_fields = ['pouchManager', 'varpManager', 'varpMap', 'saveVarp', 'POHStorageState', 'inventory', 'equipment', 'bank', 'bankSecondary', 'useSecondaryBank', 'spellBookManager', 'emoteManager', 'appearance', 'settings',
        'questRepository', 'familiarManager', 'savedData', 'musicPlayer', 'houseManager', 'bankPinManager', 'achievementDiaryManager', 'ironmanManager', 'version']
    player_getters = ['public StorageState getPOHStorageState()', 'public EquipmentContainer getEquipment()', 'public BankContainer getBank()', 'public BankContainer getBankPrimary()', 'public BankContainer getBankSecondary()',
        'public Container getInventory()', 'public Appearance getAppearance()', 'public SpellBookManager getSpellBookManager()', 'public Settings getSettings()', 'public QuestRepository getQuestRepository()',
        'public FamiliarManager getFamiliarManager()', 'public SavedData getSavedData()', 'public MusicPlayer getMusicPlayer()', 'public HouseManager getHouseManager()', 'public BankPinManager getBankPinManager()',
        'public AchievementDiaryManager getAchievementDiaryManager()', 'public IronmanManager getIronmanManager()', 'public EmoteManager getEmoteManager()']
    out['core/game/node/entity/player/Player.java'] = ('package core.game.node.entity.player;\nimport content.global.skill.construction.HouseManager;\nimport content.global.skill.construction.decoration.pohstorage.StorageState;\n'
        'import content.global.skill.runecrafting.PouchManager;\nimport content.global.skill.summoning.familiar.FamiliarManager;\nimport core.ServerConstants;\nimport core.game.container.Container;\nimport core.game.container.impl.*;\n'
        'import core.game.node.entity.Entity;\nimport core.game.node.entity.player.info.PlayerDetails;\nimport core.game.node.entity.player.link.*;\nimport core.game.node.entity.player.link.appearance.Appearance;\n'
        'import core.game.node.entity.player.link.diary.AchievementDiaryManager;\nimport core.game.node.entity.player.link.emote.EmoteManager;\nimport core.game.node.entity.player.link.music.MusicPlayer;\n'
        'import core.game.node.entity.player.link.quest.QuestRepository;\nimport java.util.*;\npublic class Player extends Entity {\n    private final PlayerDetails details = new PlayerDetails();\n'
        + members(player, player_fields, player_getters) +
        '    public PlayerDetails getDetails() { return details; }\n    public InterfaceManager getInterfaceManager() { throw new UnsupportedOperationException(); }\n'
        '    public void sendMessage(String text) { oracle.Rec.call("message", text); }\n'
        '    public boolean hasModalOpen() { oracle.Model.INSTANCE.getEvents().add(Arrays.asList("modal", oracle.Model.INSTANCE.getModal())); return oracle.Model.INSTANCE.getModal(); }\n}\n')
    missing = [name for name, text in out.items() if text is None]
    if missing:
        raise RuntimeError('Unfilled adapters: ' + ', '.join(missing))
    return out


ORIGINAL_KOTLIN = [LOGIN + 'PlayerSaver.kt', LOGIN + 'PlayerSaveParser.kt', 'core/game/node/entity/player/VarpManager.kt', 'core/JSONUtils.kt', 'core/api/PersistPlayer.kt', 'core/game/node/entity/combat/graves/GraveType.kt']
ORIGINAL_JAVA = ['core/game/node/entity/impl/GameAttributes.java', 'core/game/node/entity/player/link/SavedData.java', 'core/game/node/entity/player/link/ActivityData.java', 'core/game/node/entity/player/link/QuestData.java',
    'core/game/node/entity/player/link/GlobalData.java', 'core/game/node/entity/player/link/IronmanManager.java', 'core/game/node/entity/player/link/IronmanMode.java', 'core/game/node/entity/player/link/SpellBookManager.java',
    'core/game/node/entity/player/link/appearance/BodyPart.java', 'core/game/node/entity/player/link/appearance/Gender.java', 'core/game/node/entity/player/link/music/MusicEntry.java', 'content/global/skill/summoning/pet/PetDetails.java']
SCOPE = ('Complete unmodified PlayerSaver.kt and PlayerSaveParser.kt, including populate/save/parse/parseData/runContentHooks and real save-file I/O in owned temporary directories with file.encoding=UTF-8. '
    'Also complete unmodified VarpManager, JSONUtils, PersistPlayer, GraveType, GameAttributes, SavedData/ActivityData/QuestData/GlobalData, IronmanManager/IronmanMode, SpellBookManager, BodyPart, Gender, MusicEntry and PetDetails, '
    'and the complete original timer and Item/Container classes of the timers and inventory oracles. Verbatim selected members only for Skills, Settings, Appearance, BankPinManager, EmoteManager, Properties, Entity, Player, '
    'Bank/EquipmentContainer, Location and ServerConstants. Recording adapters for quests, house, familiars/pets, music player, achievement diaries, pouches, POH storage, graves, autocast spells and content hooks; '
    'they hold getter values and record parser hand-offs, and do not model those systems. kotlinx.coroutines is a same-thread oracle stand-in. ServerConstants.JAVA_VERSION is fixed at 11; the script-engine pretty-printing branch, '
    'default-charset behaviour, a null save path, world login/logout wiring and concurrent saving are outside this corpus.')


def build():
    """Compile the oracle; returns the runtime classpath."""
    timers.compiler.JAVA_HOME = compiler.JAVA_HOME
    inventory.compiler.JAVA_HOME = compiler.JAVA_HOME
    timers.generate(True)
    inventory.generate(True)
    compiler.SOURCES.update(inventory.compiler.SOURCES)
    RUN.mkdir(parents=True, exist_ok=True)
    classes = RUN / 'classes'
    shutil.rmtree(classes, ignore_errors=True)
    shutil.rmtree(RUN / 'adapters', ignore_errors=True)
    classes.mkdir()
    jars = compiler.artifacts()
    jar = timers.RUN / 'json-simple-1.1.1.jar'
    runtime = [p for p in jars if p.name.startswith(('kotlin-stdlib-', 'kotlin-reflect-'))]
    cp = os.pathsep.join(map(str, [classes, timers.RUN / 'classes', inventory.RUN / 'classes', jar, *runtime]))
    kotlin, java = [], []
    for name, text in adapters().items():
        path = RUN / 'adapters' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if name.endswith('.kt') else java).append(path)
    for name in ORIGINAL_KOTLIN:
        read(SERVER / name)
        kotlin.append(SERVER / name)
    for name in ORIGINAL_JAVA:
        read(SERVER / name)
        java.append(SERVER / name)
    for name in ['Coroutines.kt', 'Support.kt']:
        read(HERE / name)
        kotlin.append(HERE / name)
    # Kotlin resolves the Java sources; javac then compiles them against the Kotlin output.
    run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-nowarn', '-jvm-target', '11', '-classpath', cp, '-d', classes, *kotlin, *java])
    run(['javac', '--release', '11', '-nowarn', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    return cp


def evaluate(cp, cases):
    file = RUN / 'input.jsonl'
    file.write_bytes(''.join(json.dumps(c, sort_keys=True) + '\n' for c in cases).encode())
    executable = compiler.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if compiler.JAVA_HOME else 'java'
    with tempfile.TemporaryDirectory(prefix='files-', dir=RUN) as temp:
        assert pathlib.Path(temp).resolve().parent == RUN.resolve()
        with file.open('rb') as stdin:
            result = subprocess.run(list(map(str, [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-cp', cp, 'oracle.SupportKt', temp])), cwd=ROOT, stdin=stdin, capture_output=True, timeout=600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:])
    output = [json.loads(line) for line in result.stdout.splitlines() if line]
    if len(output) != len(cases):
        raise RuntimeError('Missing player save cases')
    return output


def generate(check):
    cp = build()
    import corpus
    seed = evaluate(cp, corpus.seed_requests())
    output = evaluate(cp, corpus.requests(seed))
    # Texts written by the original are then loaded by the original into profiles that start empty.
    output += evaluate(cp, corpus.second_generation(output))
    data = ''.join(json.dumps(c, sort_keys=True, ensure_ascii=True, separators=(',', ':')) + '\n' for c in output).encode()
    read(pathlib.Path(__file__))
    read(HERE / 'corpus.py')
    read(ROOT / 'oracle/login/generate.py')
    provenance = dict(format=1, upstream=json.loads(read(ROOT / 'upstream.lock.json')), sources=dict(sorted(compiler.SOURCES.items())), caseCount=len(output),
        actionCount=sum(len(c['actions']) for c in output), fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm='sha256, CR removed', scope=SCOPE)
    folder = ROOT / 'fixtures/player-save'
    if check:
        if (folder / 'oracle.jsonl').read_bytes() != data or json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Player save oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original player save cases reproduced.')
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'oracle.jsonl').write_bytes(data)
        (folder / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {len(output)} original player save cases.')


if __name__ == '__main__':
    import sys
    sys.path.insert(0, str(HERE))
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--check', action='store_true')
    p.add_argument('--java-home', type=pathlib.Path)
    a = p.parse_args()
    compiler.JAVA_HOME = a.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(a.check)
