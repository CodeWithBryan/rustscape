"""Recording adapters for the player link classes, dialogue, map, packets and the Java content classes ContentAPI
reaches."""

ADAPTERS = {
'core/game/node/entity/player/link/PacketDispatch.java': r'''package core.game.node.entity.player.link;
import core.game.node.scenery.Scenery;
import core.game.world.update.flag.context.Animation;
import oracle.R;
public class PacketDispatch {
    public final Object owner;
    public PacketDispatch(Object owner) { this.owner = owner; }
    public void sendSceneryAnimation(Scenery object, Animation animation, boolean global) { R.v("PacketDispatch.sendSceneryAnimation", owner, object, animation, global); }
    public void sendVarp(int index, int value) { R.v("PacketDispatch.sendVarp", owner, index, value); }
    public void sendVarcUpdate(short index, int value) { R.v("PacketDispatch.sendVarcUpdate", owner, index, value); }
    public void sendString(String string, int interfaceId, int lineId) { R.v("PacketDispatch.sendString", owner, string, interfaceId, lineId); }
    public void sendInterfaceConfig(int interfaceId, int childId, boolean hide) { R.v("PacketDispatch.sendInterfaceConfig", owner, interfaceId, childId, hide); }
    public void sendPlayerOnInterface(int interfaceId, int childId) { R.v("PacketDispatch.sendPlayerOnInterface", owner, interfaceId, childId); }
    public void sendAnimationInterface(int animationId, int interfaceId, int childId) { R.v("PacketDispatch.sendAnimationInterface", owner, animationId, interfaceId, childId); }
    public void sendItemOnInterface(int itemId, int amount, int interfaceId, int childId) { R.v("PacketDispatch.sendItemOnInterface", owner, itemId, amount, interfaceId, childId); }
    public void sendItemZoomOnInterface(int itemId, int zoom, int interfaceId, int childId) { R.v("PacketDispatch.sendItemZoomOnInterface", owner, itemId, zoom, interfaceId, childId); }
    public void sendRunScript(int id, String string, Object... objects) { R.v("PacketDispatch.sendRunScript", owner, id, string, objects); }
    public void sendIfaceSettings(int settingsHash, int childId, int interfaceId, int offset, int length) { R.v("PacketDispatch.sendIfaceSettings", owner, settingsHash, childId, interfaceId, offset, length); }
}
''',
'core/game/node/entity/player/link/InterfaceManager.java': r'''package core.game.node.entity.player.link;
import core.game.component.Component;
import oracle.R;
public class InterfaceManager {
    public final Object owner;
    public InterfaceManager(Object owner) { this.owner = owner; }
    public Component open(Component component) { R.v("InterfaceManager.open", owner, component); return component; }
    public void openOverlay(Component component) { R.v("InterfaceManager.openOverlay", owner, component); }
    public void closeOverlay() { R.v("InterfaceManager.closeOverlay", owner); }
    public void removeTabs(int... tabs) { R.v("InterfaceManager.removeTabs", owner, tabs); }
    public void restoreTabs() { R.v("InterfaceManager.restoreTabs", owner); }
    public boolean close() { return R.b("InterfaceManager.close", owner); }
    public boolean closeSingleTab() { return R.b("InterfaceManager.closeSingleTab", owner); }
    public void closeChatbox() { R.v("InterfaceManager.closeChatbox", owner); }
    public Component openSingleTab(Component component) { R.v("InterfaceManager.openSingleTab", owner, component); return component; }
}
''',
'core/game/component/Component.java': r'''package core.game.component;
import java.util.Arrays;
public class Component implements oracle.R.Describable {
    protected int id;
    public Component(int id) { this.id = id; }
    public int getId() { return id; }
    public Object describe() { return Arrays.asList("component", id); }
}
''',
'core/game/node/entity/player/link/appearance/Appearance.java': r'''package core.game.node.entity.player.link.appearance;
import core.game.world.update.flag.context.Animation;
import oracle.R;
public class Appearance {
    public final Object owner;
    public Appearance(Object owner) { this.owner = owner; }
    public void setAnimations(Animation animation) { R.v("Appearance.setAnimations", owner, animation); }
    public void setAnimations() { R.v("Appearance.setAnimations", owner); }
    public void setDefaultAnimations() { R.v("Appearance.setDefaultAnimations", owner); }
    public void sync() { R.v("Appearance.sync", owner); }
}
''',
'core/game/node/entity/player/link/TeleportManager.java': r'''package core.game.node.entity.player.link;
import core.game.world.map.Location;
public class TeleportManager {
    public final Object owner;
    public TeleportManager(Object owner) { this.owner = owner; }
    public boolean send(Location location, TeleportType type) { return oracle.R.b("TeleportManager.send", owner, location, type); }
    public enum TeleportType { NORMAL, ANCIENT, LUNAR, TELETABS, INSTANT, OBELISK, TELE_OTHER, FAIRY_RING, PURO_PURO, ECTOPHIAL, CHRISTMAS, CABBAGE, ENTRANA_MAGIC_DOOR, MINIGAME }
}
''',
'core/game/node/entity/player/link/HintIconManager.java': r'''package core.game.node.entity.player.link;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import oracle.R;
public class HintIconManager {
    public final Object owner;
    public HintIconManager(Object owner) { this.owner = owner; }
    public static int registerHintIcon(Player player, Node target) { return R.i("HintIconManager.registerHintIcon", player, target); }
    public static int registerHintIcon(Player player, Node target, int arrowId, int modelId, int slot, int height, int targetType) {
        return R.i("HintIconManager.registerHintIcon", player, target, arrowId, modelId, slot, height, targetType);
    }
    public static void removeHintIcon(Player player, int slot) { R.v("HintIconManager.removeHintIcon", player, slot); }
    public int freeSlot() { return R.i("HintIconManager.freeSlot", owner); }
}
''',
'core/game/node/entity/player/link/IronmanMode.java': r'''package core.game.node.entity.player.link;
public enum IronmanMode {
    NONE(-1), STANDARD(5), HARDCORE(6), ULTIMATE(7);
    final int icon;
    IronmanMode(int icon) { this.icon = icon; }
}
''',
'core/game/node/entity/player/link/IronmanManager.java': r'''package core.game.node.entity.player.link;
import oracle.R;
public class IronmanManager {
    public final Object owner;
    public IronmanManager(Object owner) { this.owner = owner; }
    public boolean checkRestriction(IronmanMode mode) { return R.b("IronmanManager.checkRestriction", owner, mode); }
    public boolean isIronman() { return R.b("IronmanManager.isIronman", owner); }
    public IronmanMode getMode() { String m = R.s("IronmanManager.getMode", owner); return m == null ? null : IronmanMode.valueOf(m); }
}
''',
'core/game/node/entity/player/link/BankPinManager.java': r'''package core.game.node.entity.player.link;
public class BankPinManager {
    public final Object owner;
    public BankPinManager(Object owner) { this.owner = owner; }
    public void openSettings() { oracle.R.v("BankPinManager.openSettings", owner); }
}
''',
'core/game/node/entity/player/link/emote/Emotes.java': r'''package core.game.node.entity.player.link.emote;
import core.game.world.update.flag.context.Animation;
public enum Emotes {
    YES(855), NO(856), BOW(858);
    final int animation;
    Emotes(int animation) { this.animation = animation; }
    public Animation getAnimation() { return new Animation(animation); }
}
''',
'core/game/node/entity/player/link/quest/Quest.java': r'''package core.game.node.entity.player.link.quest;
import core.game.node.entity.player.Player;
import oracle.R;
public class Quest {
    public String key;
    public boolean hasRequirements(Player player) { return R.b("Quest.hasRequirements", key, player); }
    public void start(Player player) { R.v("Quest.start", key, player); }
    public void finish(Player player) { R.v("Quest.finish", key, player); }
}
''',
'core/game/node/entity/player/link/quest/QuestRepository.java': r'''package core.game.node.entity.player.link.quest;
import content.data.Quests;
import core.game.node.entity.player.Player;
import java.util.Map;
import oracle.R;
public class QuestRepository {
    public final Object owner;
    public QuestRepository(Object owner) { this.owner = owner; }
    public int getPoints() { return R.i("QuestRepository.getPoints", owner); }
    public int getAvailablePoints() { return R.i("QuestRepository.getAvailablePoints", owner); }
    public int getStage(Quests quest) { return R.i("QuestRepository.getStage", owner, quest); }
    public void setStage(Quest quest, int stage) { R.v("QuestRepository.setStage", owner, quest == null ? null : quest.key, stage); }
    public void syncronizeTab(Player player) { R.v("QuestRepository.syncronizeTab", owner, player); }
    public Quest getQuest(Quests quest) { return R.quest(R.s("QuestRepository.getQuest", owner, quest)); }
    public static Map<Quests, Quest> getQuests() { return R.quests(R.ev("QuestRepository.getQuests")); }
}
''',
'core/game/node/entity/player/link/diary/DiaryType.java': r'''package core.game.node.entity.player.link.diary;
public enum DiaryType {
    KARAMJA(new String[]{"Easy", "Medium", "Hard"}),
    VARROCK(new String[]{"Easy", "Medium", "Hard"}),
    LUMBRIDGE(new String[]{"Beginner", "Easy", "Medium"});
    private final String[] levelNames;
    DiaryType(String[] levelNames) { this.levelNames = levelNames; }
    public String[] getLevelNames() { return levelNames; }
}
''',
'core/game/node/entity/player/link/diary/AchievementDiaryManager.java': r'''package core.game.node.entity.player.link.diary;
import core.game.node.entity.player.Player;
public class AchievementDiaryManager {
    public final Object owner;
    public AchievementDiaryManager(Object owner) { this.owner = owner; }
    public void finishTask(Player player, DiaryType type, int level, int index) { oracle.R.v("AchievementDiaryManager.finishTask", owner, player, type, level, index); }
}
''',
'core/game/node/entity/player/link/prayer/PrayerType.java': r'''package core.game.node.entity.player.link.prayer;
public enum PrayerType { PROTECT_FROM_MAGIC }
''',
'core/game/node/entity/player/link/prayer/Prayer.java': r'''package core.game.node.entity.player.link.prayer;
public class Prayer {
    public final Object owner;
    public Prayer(Object owner) { this.owner = owner; }
    public boolean get(PrayerType type) { return oracle.R.b("Prayer.get", owner, type); }
}
''',
'core/game/node/entity/player/info/PlayerDetails.java': r'''package core.game.node.entity.player.info;
public class PlayerDetails {
    public core.auth.UserAccountInfo accountInfo;
    public PlayerDetails(Object owner) { this.accountInfo = new core.auth.UserAccountInfo(owner); }
}
''',
'core/auth/UserAccountInfo.java': r'''package core.auth;
public class UserAccountInfo {
    public final Object owner;
    public UserAccountInfo(Object owner) { this.owner = owner; }
    public int getCredits() { return oracle.R.i("UserAccountInfo.getCredits", owner); }
    public void setCredits(int credits) { oracle.R.v("UserAccountInfo.setCredits", owner, credits); }
}
''',
'core/game/dialogue/DialogueInterpreter.java': r'''package core.game.dialogue;
import core.game.component.Component;
import core.game.node.entity.Entity;
import core.game.node.item.Item;
import oracle.R;
public class DialogueInterpreter {
    public final Object owner;
    public DialogueInterpreter(Object owner) { this.owner = owner; }
    static Component c(Object a) { return a == null ? null : new Component(((Number) a).intValue()); }
    public boolean open(int dialogueKey, Object... args) { return R.b("DialogueInterpreter.open", owner, dialogueKey, args); }
    public void open(DialogueFile file, Object... args) { R.v("DialogueInterpreter.open", owner, file, args); }
    public boolean close() { return R.b("DialogueInterpreter.close", owner); }
    public Component sendDialogue(String... messages) { return c(R.ev("DialogueInterpreter.sendDialogue", owner, messages)); }
    public Component sendItemMessage(final Item item, String... messages) { return c(R.ev("DialogueInterpreter.sendItemMessage", owner, item, messages)); }
    public Component sendDoubleItemMessage(int first, int second, String... message) { return c(R.ev("DialogueInterpreter.sendDoubleItemMessage", owner, first, second, message)); }
    public Component sendDoubleItemMessage(Item first, Item second, String... messages) { return c(R.ev("DialogueInterpreter.sendDoubleItemMessage", owner, first, second, messages)); }
    public Component sendDialogues(Entity entity, FacialExpression expression, boolean hide, String... messages) { return c(R.ev("DialogueInterpreter.sendDialogues", owner, entity, expression, hide, messages)); }
    public Component sendDialogues(int npcId, FacialExpression expression, boolean hide, String... messages) { return c(R.ev("DialogueInterpreter.sendDialogues", owner, npcId, expression, hide, messages)); }
    public Component sendDialogues(int npcId, FacialExpression expression, String... messages) { return c(R.ev("DialogueInterpreter.sendDialogues", owner, npcId, expression, messages)); }
    public Component sendOptions(Object title, String... options) { return c(R.ev("DialogueInterpreter.sendOptions", owner, title, options)); }
    public void sendInput(boolean string, Object... objects) { R.v("DialogueInterpreter.sendInput", owner, string, objects); }
    public void sendLongInput(Object... objects) { R.v("DialogueInterpreter.sendLongInput", owner, objects); }
    public void sendMessageInput(String reciever) { R.v("DialogueInterpreter.sendMessageInput", owner, reciever); }
    public void addAction(DialogueAction action) { R.v("DialogueInterpreter.addAction", owner, action); }
}
''',
'core/game/dialogue/FacialExpression.java': r'''package core.game.dialogue;
public enum FacialExpression { NEUTRAL, HAPPY, ASKING, ANGRY, FRIENDLY }
''',
'core/game/dialogue/DialogueAction.java': r'''package core.game.dialogue;
public interface DialogueAction {
    void handle(core.game.node.entity.player.Player player, int buttonId);
}
''',
'core/game/world/map/Direction.java': r'''package core.game.world.map;
public enum Direction {
    NORTH_WEST(-1, 1), NORTH(0, 1), NORTH_EAST(1, 1), WEST(-1, 0), EAST(1, 0), SOUTH_WEST(-1, -1), SOUTH(0, -1), SOUTH_EAST(1, -1);
    private final int stepX, stepY;
    Direction(int stepX, int stepY) { this.stepX = stepX; this.stepY = stepY; }
    public int getStepX() { return stepX; }
    public int getStepY() { return stepY; }
    /* ORIGINAL_DIRECTION_GET */
}
''',
'core/game/world/map/MapDistance.java': r'''package core.game.world.map;
public enum MapDistance {
    RENDERING(15);
    private final int distance;
    MapDistance(int distance) { this.distance = distance; }
    public int getDistance() { return distance; }
}
''',
'core/game/world/map/Region.java': r'''package core.game.world.map;
public class Region {
    public final int id;
    public Region(int id) { this.id = id; }
    public boolean isActive() { return oracle.R.b("Region.isActive", id); }
    public RegionChunk[][][] getChunks() { return null; }
    public static void load(Region region) { oracle.R.v("Region.load", region); }
}
''',
'core/game/world/map/RegionChunk.java': r'''package core.game.world.map;
public class RegionChunk {
    public void flag(Object flag) { oracle.R.v("RegionChunk.flag", this, flag); }
    public RegionChunk copy() { return oracle.R.n("RegionChunk.copy", this); }
}
''',
'core/game/world/map/build/DynamicRegion.java': r'''package core.game.world.map.build;
import core.game.world.map.Region;
import core.game.world.map.RegionChunk;
import core.game.world.map.zone.ZoneBorders;
public class DynamicRegion extends Region {
    public DynamicRegion(ZoneBorders borders) { super(-1); oracle.R.v("DynamicRegion", borders); }
    public static DynamicRegion create(int regionId) { return oracle.R.n("DynamicRegion.create", regionId); }
    public static ZoneBorders reserveArea(int width, int height) { return oracle.R.borders(oracle.R.ev("DynamicRegion.reserveArea", width, height)); }
    public void replaceChunk(int z, int x, int y, RegionChunk chunk, Region region) { oracle.R.v("DynamicRegion.replaceChunk", this, z, x, y, chunk, region); }
}
''',
'core/game/world/update/flag/chunk/AnimateObjectUpdateFlag.java': r'''package core.game.world.update.flag.chunk;
import java.util.Arrays;
public class AnimateObjectUpdateFlag implements oracle.R.Describable {
    final core.game.world.update.flag.context.Animation animation;
    public AnimateObjectUpdateFlag(core.game.world.update.flag.context.Animation animation) { this.animation = animation; }
    public Object describe() { return Arrays.asList("AnimateObjectUpdateFlag", oracle.R.value(animation)); }
}
''',
'core/game/world/map/zone/ZoneBorders.java': r'''package core.game.world.map.zone;
import core.game.node.Node;
import core.game.world.map.Location;
import java.util.Arrays;
public class ZoneBorders implements oracle.R.Describable {
    public final int southWestX, southWestY, northEastX, northEastY, plane;
    public ZoneBorders(int x1, int y1, int x2, int y2) { this(x1, y1, x2, y2, 0); }
    public ZoneBorders(int x1, int y1, int x2, int y2, int plane) { this.southWestX = x1; this.southWestY = y1; this.northEastX = x2; this.northEastY = y2; this.plane = plane; }
    public static ZoneBorders forRegion(int regionId) { return oracle.R.borders(oracle.R.ev("ZoneBorders.forRegion", regionId)); }
    public boolean insideBorder(Node node) { return oracle.R.b("ZoneBorders.insideBorder", this, node); }
    public Location getRandomLoc() { return oracle.R.n("ZoneBorders.getRandomLoc", this); }
    public Object describe() { return Arrays.asList("borders", southWestX, southWestY, northEastX, northEastY, plane); }
}
''',
'core/game/world/map/zone/MapZone.java': r'''package core.game.world.map.zone;
public class MapZone {
    public String key;
    public void register(ZoneBorders borders) { oracle.R.v("MapZone.register", key, borders); }
}
''',
'core/game/world/map/zone/ZoneBuilder.java': r'''package core.game.world.map.zone;
public class ZoneBuilder {
    public static void configure(MapZone zone) { oracle.R.v("ZoneBuilder.configure", zone.key); }
}
''',
'core/game/world/map/zone/RegionZone.java': 'package core.game.world.map.zone;\npublic class RegionZone {}\n',
'core/game/world/map/zone/ZoneRestriction.java': 'package core.game.world.map.zone;\npublic enum ZoneRestriction { FOLLOWERS }\n',
'core/game/node/entity/combat/CombatStyle.java': 'package core.game.node.entity.combat;\npublic enum CombatStyle { MELEE, RANGE, MAGIC }\n',
'core/game/world/map/path/Pathfinder.java': r'''package core.game.world.map.path;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
public class Pathfinder implements oracle.R.Describable {
    public static final Pathfinder SMART = new Pathfinder("SMART");
    public static final Pathfinder DUMB = new Pathfinder("DUMB");
    final String name;
    Pathfinder(String name) { this.name = name; }
    public Object describe() { return name; }
    public static Path find(Entity mover, Node destination, boolean near, Pathfinder finder) { return Path.of(oracle.R.ev("Pathfinder.find", mover, destination, near, finder)); }
    public static Path find(Location start, Node destination, int moverSize) { return Path.of(oracle.R.ev("Pathfinder.find", start, destination, moverSize)); }
    public static Path find(Location start, Node destination, boolean near, Pathfinder finder) { return Path.of(oracle.R.ev("Pathfinder.find", start, destination, near, finder)); }
}
''',
'core/game/world/map/path/Path.java': r'''package core.game.world.map.path;
import java.util.LinkedList;
import java.util.Map;
public class Path {
    boolean successful, moveNear;
    public LinkedList<core.game.node.entity.player.link.Point> points = new LinkedList<>();
    static Path of(Object answer) {
        if (answer == null) return null;
        Path p = new Path();
        Map<?, ?> m = (Map<?, ?>) answer;
        p.successful = Boolean.TRUE.equals(m.get("successful"));
        p.moveNear = Boolean.TRUE.equals(m.get("moveNear"));
        oracle.R.fresh(p, "path");
        return p;
    }
    public boolean isSuccessful() { return successful; }
    public boolean isMoveNear() { return moveNear; }
    public LinkedList<core.game.node.entity.player.link.Point> getPoints() { return points; }
    public void walk(core.game.node.entity.Entity mover) { oracle.R.v("Path.walk", this, mover); }
}
''',
'core/game/node/entity/player/link/Point.java': 'package core.game.node.entity.player.link;\npublic class Point { public int getX() { return 0; } public int getY() { return 0; } }\n',
'core/game/global/action/SpecialLadders.java': r'''package core.game.global.action;
public class SpecialLadders {
    public static void add(core.game.world.map.Location from, core.game.world.map.Location to) { oracle.R.v("SpecialLadders.add", from, to); }
}
''',
'core/game/global/action/DoorActionHandler.java': r'''package core.game.global.action;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.scenery.Scenery;
import core.game.world.map.Location;
import oracle.R;
public class DoorActionHandler {
    public static void handleDoor(final Player player, final Scenery object) { R.v("DoorActionHandler.handleDoor", player, object); }
    public static Location getEndLocation(Entity entity, Scenery object) { return R.n("DoorActionHandler.getEndLocation", entity, object); }
    public static boolean handleAutowalkDoorWithLocation(final Entity entity, final Scenery object, final Location endLocation) { return R.b("DoorActionHandler.handleAutowalkDoorWithLocation", entity, object, endLocation); }
}
''',
'core/net/packet/PacketRepository.java': r'''package core.net.packet;
public class PacketRepository {
    public static void send(Class<?> clazz, Object context) { oracle.R.v("PacketRepository.send", clazz.getSimpleName(), context); }
}
''',
'core/net/packet/out/AudioPacket.java': 'package core.net.packet.out;\npublic class AudioPacket {}\n',
'core/net/packet/out/MusicPacket.java': 'package core.net.packet.out;\npublic class MusicPacket {}\n',
'core/net/packet/out/CameraViewPacket.java': 'package core.net.packet.out;\npublic class CameraViewPacket {}\n',
'core/net/packet/context/DefaultContext.java': r'''package core.net.packet.context;
import java.util.ArrayList;
import java.util.List;
public class DefaultContext implements oracle.R.Describable {
    final Object player; final Object[] objects;
    public DefaultContext(core.game.node.entity.player.Player player, Object... objects) { this.player = player; this.objects = objects; }
    public Object describe() { List<Object> l = new ArrayList<>(); l.add("DefaultContext"); l.add(oracle.R.value(player)); for (Object o : objects) l.add(oracle.R.value(o)); return l; }
}
''',
'core/net/packet/context/MusicContext.java': r'''package core.net.packet.context;
import java.util.Arrays;
public class MusicContext implements oracle.R.Describable {
    final Object player; final int id; final boolean temporary;
    public MusicContext(core.game.node.entity.player.Player player, int musicId, boolean temporary) { this.player = player; this.id = musicId; this.temporary = temporary; }
    public Object describe() { return Arrays.asList("MusicContext", oracle.R.value(player), id, temporary); }
}
''',
'core/net/packet/context/CameraContext.java': r'''package core.net.packet.context;
import java.util.Arrays;
public class CameraContext implements oracle.R.Describable {
    public enum CameraType { POSITION, ROTATION, SET, RESET, SHAKE }
    final Object player; final CameraType type; final int x, y, height, speed, zoomSpeed;
    public CameraContext(core.game.node.entity.player.Player player, CameraType type, int x, int y, int height, int speed, int zoomSpeed) {
        this.player = player; this.type = type; this.x = x; this.y = y; this.height = height; this.speed = speed; this.zoomSpeed = zoomSpeed;
    }
    public int getX() { return x; }
    public int getY() { return y; }
    public int getHeight() { return height; }
    public Object describe() { return Arrays.asList("CameraContext", oracle.R.value(player), type.name(), x, y, height, speed, zoomSpeed); }
}
''',
'core/tools/RandomFunction.java': r'''package core.tools;
public class RandomFunction {
    public static int getRandom(int maxValue) { return oracle.R.i("RandomFunction.getRandom", maxValue); }
    public static double random(double a, double b) { return oracle.R.d("RandomFunction.random", a, b); }
}
''',
'content/data/skill/SkillingTool.java': r'''package content.data.skill;
import core.game.node.entity.player.Player;
public enum SkillingTool {
    BRONZE_AXE, DRAGON_AXE, BRONZE_PICKAXE, RUNE_PICKAXE;
    static SkillingTool of(String name) { return name == null ? null : valueOf(name); }
    public static SkillingTool getPickaxe(Player player) { return of(oracle.R.s("SkillingTool.getPickaxe", player)); }
    public static SkillingTool getHatchet(Player player) { return of(oracle.R.s("SkillingTool.getHatchet", player)); }
}
''',
'content/global/skill/summoning/familiar/FamiliarManager.java': r'''package content.global.skill.summoning.familiar;
import oracle.R;
public class FamiliarManager {
    public final Object owner;
    public FamiliarManager(Object owner) { this.owner = owner; }
    public boolean hasFamiliar() { return R.b("FamiliarManager.hasFamiliar", owner); }
    public Familiar getFamiliar() { return R.n("FamiliarManager.getFamiliar", owner); }
    public int getBoost(int skill) { return R.i("FamiliarManager.getBoost", owner, skill); }
}
''',
'content/global/skill/summoning/familiar/Familiar.java': r'''package content.global.skill.summoning.familiar;
public class Familiar extends core.game.node.entity.npc.NPC {
    public boolean isBurdenBeast() { return false; }
}
''',
'content/global/skill/summoning/familiar/BurdenBeast.java': r'''package content.global.skill.summoning.familiar;
public class BurdenBeast extends Familiar {
    public final core.game.container.Container container = new core.game.container.Container(this, "bob");
    @Override public boolean isBurdenBeast() { return true; }
    public core.game.container.Container getContainer() { return container; }
}
''',
'content/global/skill/construction/HouseManager.java': r'''package content.global.skill.construction;
public class HouseManager {
    public final Object owner;
    public HouseManager(Object owner) { this.owner = owner; }
    public boolean hasHouse() { return oracle.R.b("HouseManager.hasHouse", owner); }
}
''',
'core/game/consumable/Consumable.java': r'''package core.game.consumable;
public class Consumable {
    public String key;
    public int identityHash;
    protected final int[] ids;
    public Consumable(int[] ids) { this.ids = ids; }
    public int[] getIds() { return ids; }
    @Override public int hashCode() { return identityHash; }
}
''',
'core/game/consumable/Potion.java': r'''package core.game.consumable;
public class Potion extends Consumable {
    public Potion(int[] ids) { super(ids); }
}
''',
'content/data/consumables/Consumables.java': r'''package content.data.consumables;
public class Consumables {
    public core.game.consumable.Consumable consumable;
    public static Consumables getConsumableById(final int itemId) { return oracle.R.consumables(itemId); }
    public core.game.consumable.Consumable getConsumable() { return consumable; }
}
''',
'content/global/skill/slayer/Tasks.java': r'''package content.global.skill.slayer;
public enum Tasks {
    CRAWLING_HANDS, BANSHEE, ABERRANT_SPECTRES;
    public String[] getTip() { java.util.List<?> l = (java.util.List<?>) oracle.R.ev("Tasks.getTip", this); return l == null ? null : l.toArray(new String[0]); }
}
''',
'content/global/skill/slayer/Master.java': r'''package content.global.skill.slayer;
public enum Master {
    TURAEL(8273), MAZCHNA(8274), VANNAKA(1597), CHAELDAR(1598), DURADEL(8275);
    final int npc_id;
    Master(int npc_id) { this.npc_id = npc_id; }
    public int getNpc() { return this.npc_id; }
}
''',
'core/game/system/task/Pulse.java': r'''package core.game.system.task;
public abstract class Pulse {
    public int delay;
    public Pulse() { this(1); }
    public Pulse(int delay) { this.delay = delay; }
    public abstract boolean pulse();
    public int getDelay() { return delay; }
}
''',
'core/game/system/task/PulseRunner.java': r'''package core.game.system.task;
public class PulseRunner {
    public void submit(Pulse pulse) { oracle.R.pulse("PulseRunner.submit", null, pulse); }
}
''',
}
