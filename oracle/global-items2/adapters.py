"""Recording adapters for the global-items2 oracle.

Each entry is a source file compiled with the unchanged originals listed in generate.py. `/* ORIGINAL_* */` markers are
replaced by original text read from the pinned sources at generation time. Everything else records the call in
oracle.Support and answers from the case. Field names differ from the getters' Kotlin properties so that Kotlin reads go
through the getters, as in the original.
"""

ADAPTERS = {
    # ------------------------------------------------------------------ nodes
    'core/game/node/Node.java': r'''package core.game.node;
import core.game.world.map.Location;
public class Node {
    public String key, nodeName;
    public int nodeId;
    public Location nodeLocation;
    public int getId() { return nodeId; }
    public String getName() { return nodeName; }
    public Location getLocation() { return nodeLocation; }
    public boolean isActive() { return true; }
    public core.game.node.entity.npc.NPC asNpc() { return (core.game.node.entity.npc.NPC) this; }
    public core.game.node.item.Item asItem() { return (core.game.node.item.Item) this; }
    public core.game.node.scenery.Scenery asScenery() { return (core.game.node.scenery.Scenery) this; }
    @Override public String toString() { return key; }
}
''',
    'core/game/node/entity/Entity.java': r'''package core.game.node.entity;
import core.game.node.Node;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import oracle.Support;
import java.util.Map;
public class Entity extends Node {
    public final Map<String, Object> attrs = new java.util.TreeMap<>();
    final core.game.node.entity.impl.Animator animatorObject = new core.game.node.entity.impl.Animator();
    final core.game.node.entity.impl.PulseManager pulseObject = new core.game.node.entity.impl.PulseManager();
    public core.game.node.entity.impl.Animator getAnimator() { return animatorObject; }
    public core.game.node.entity.impl.PulseManager getPulseManager() { return pulseObject; }
    public void lock() { Support.emit("lockAll"); }
    public void lock(int time) { Support.emit("lock", time); }
    public void unlock() { Support.emit("unlock"); }
    public boolean animate(Animation animation) { return animatorObject.animate(animation); }
    public boolean graphics(Graphics graphics) { return animatorObject.graphics(graphics); }
    public boolean visualize(Animation animation, Graphics graphics) { return animatorObject.animate(animation, graphics); }
    public void faceLocation(Location l) { Support.emit("faceLocation", Support.coord(l)); }
    public boolean face(Entity entity) { Support.emit("face", Support.key(entity)); return true; }
    public void faceTemporary(Entity entity, int duration) { Support.emit("faceTemporary", Support.key(entity), duration); }
    public void sendChat(String message) { Support.emit("chat", message); }
    // GameAttributes: "/save:" is stripped on set and read.
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k, T fail) {
        Support.emit("getAttribute", k);
        Object object = attrs.get(k.replace("/save:", ""));
        return object != null ? (T) object : fail;
    }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k) { Support.emit("getAttribute", k); return (T) attrs.get(k.replace("/save:", "")); }
    public void setAttribute(String k, Object v) {
        Support.emit("setAttribute", k, Support.value(v));
        attrs.put(k.startsWith("/save:") ? k.substring(6) : k, v);
    }
    public void removeAttribute(String k) {
        Support.emit("removeAttribute", k);
        attrs.remove(k.replace("/save:", ""));
    }
}
''',
    'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
import oracle.Support;
public class Player extends core.game.node.entity.Entity {
    final core.game.node.entity.player.link.PacketDispatch dispatchObject = new core.game.node.entity.player.link.PacketDispatch();
    final core.game.container.Container inventoryObject = new core.game.container.Container("inventory");
    final core.game.container.Container equipmentObject = new core.game.container.Container("equipment");
    final core.game.container.Container bankObject = new core.game.container.Container("bank");
    final core.game.dialogue.DialogueInterpreter interpreterObject = new core.game.dialogue.DialogueInterpreter();
    final core.game.node.entity.player.link.appearance.Appearance appearanceObject = new core.game.node.entity.player.link.appearance.Appearance();
    final core.game.node.entity.player.link.diary.AchievementDiaryManager diaryObject = new core.game.node.entity.player.link.diary.AchievementDiaryManager();
    final core.game.node.entity.player.link.quest.QuestRepository questObject = new core.game.node.entity.player.link.quest.QuestRepository();
    final core.game.node.entity.combat.SkullManager skullObject = new core.game.node.entity.combat.SkullManager();
    final core.game.node.entity.skill.Skills skillsObject = new core.game.node.entity.skill.Skills();
    final core.game.node.entity.player.link.InterfaceManager interfaceObject = new core.game.node.entity.player.link.InterfaceManager();
    public core.game.node.entity.player.link.PacketDispatch getPacketDispatch() { return dispatchObject; }
    public core.game.container.Container getInventory() { return inventoryObject; }
    public core.game.container.Container getEquipment() { return equipmentObject; }
    public core.game.container.Container getBank() { return bankObject; }
    public core.game.dialogue.DialogueInterpreter getDialogueInterpreter() { return interpreterObject; }
    public core.game.node.entity.player.link.appearance.Appearance getAppearance() { return appearanceObject; }
    public core.game.node.entity.player.link.diary.AchievementDiaryManager getAchievementDiaryManager() { return diaryObject; }
    public core.game.node.entity.player.link.quest.QuestRepository getQuestRepository() { return questObject; }
    public core.game.node.entity.combat.SkullManager getSkullManager() { return skullObject; }
    public core.game.node.entity.skill.Skills getSkills() { return skillsObject; }
    public core.game.node.entity.player.link.InterfaceManager getInterfaceManager() { return interfaceObject; }
    public String getUsername() { Support.emit("username"); return Support.text("username", "Tracer"); }
    public void sendMessage(String text) { Support.emit("message", text); }
    public void debug(String text) { Support.emit("debug", text); }
    final content.global.skill.construction.decoration.pohstorage.StorageState storageObject = new content.global.skill.construction.decoration.pohstorage.StorageState();
    public content.global.skill.construction.decoration.pohstorage.StorageState getPOHStorageState() { return storageObject; }
}
''',
    'core/game/node/entity/npc/NPC.java': r'''package core.game.node.entity.npc;
public class NPC extends core.game.node.entity.Entity {
    public NPC() {}
    public NPC(int id) { oracle.Support.emit("newNpc", id); nodeId = id; }
}
''',
    'core/game/node/scenery/Scenery.java': 'package core.game.node.scenery;\npublic class Scenery extends core.game.node.Node {}\n',
    'core/game/node/item/Item.java': r'''package core.game.node.item;
public class Item extends core.game.node.Node {
    private int amount = 1;
    public int itemSlot = -1;
    public int itemCharge = 1000;
    public Item() {}
    public Item(int id) { this(id, 1, 1000); }
    public Item(int id, int amount) { this(id, amount, 1000); }
    public Item(int id, int amount, int charge) { nodeId = id; this.amount = amount; itemCharge = charge; oracle.Support.newItem(this, id, amount); }
    public void oracleAmount(int value) { amount = value; }
    public int getAmount() { return amount; }
    public int getSlot() { return itemSlot; }
    public int getCharge() { return itemCharge; }
    public void setCharge(int charge) { oracle.Support.emit("setCharge", oracle.Support.key(this), charge); itemCharge = charge; }
    public core.cache.def.impl.ItemDefinition getDefinition() { return core.cache.def.impl.ItemDefinition.forId(nodeId); }
}
''',
    'core/cache/def/impl/ItemDefinition.java': r'''package core.cache.def.impl;
import java.util.Map;
public class ItemDefinition {
    public final int definitionId;
    public String definitionName, definitionExamine;
    public Object equipmentSlot;
    public ItemDefinition(int id) { definitionId = id; }
    public static ItemDefinition forId(int id) {
        ItemDefinition d = oracle.Support.definitions.get(id);
        if (d != null) return d;
        d = new ItemDefinition(id);
        d.definitionName = oracle.Support.itemName(id);
        return d;
    }
    /** The definitions table, a HashMap filled one put at a time in the case's order (as the loader fills it). */
    public static Map<Integer, ItemDefinition> getDefinitions() {
        Map<Integer, ItemDefinition> m = new java.util.HashMap<>();
        for (Map.Entry<Integer, ItemDefinition> e : oracle.Support.definitions.entrySet()) m.put(e.getKey(), e.getValue());
        return m;
    }
    public int getId() { return definitionId; }
    public String getName() { return definitionName; }
    public String getExamine() { return definitionExamine; }
    @SuppressWarnings("unchecked")
    public <V> V getConfiguration(String key, V fail) {
        if (!"equipment_slot".equals(key)) throw new IllegalArgumentException(key);
        return equipmentSlot == null ? fail : (V) equipmentSlot;
    }
    public Map<String, Object> getHandlers() {
        return new java.util.TreeMap<String, Object>() {
            @Override public Object put(String k, Object v) { oracle.Support.itemHandler(definitionId, k, v); return null; }
        };
    }
}
''',
    'core/cache/def/impl/SceneryDefinition.java': r'''package core.cache.def.impl;
public class SceneryDefinition {
    public static SceneryDefinition forId(int id) { return new SceneryDefinition(); }
    public int[] getChildrenIds() { return null; }
}
''',
    'core/game/world/map/Location.java': r'''package core.game.world.map;
public class Location extends core.game.node.Node {
    final int lx, ly, lz;
    public Location(int x, int y) { this(x, y, 0); }
    public Location(int x, int y, int z) { lx = x; ly = y; lz = z; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public static Location create(Location l) { return new Location(l.getX(), l.getY(), l.getZ()); }
    public int getX() { return lx; }
    public int getY() { return ly; }
    public int getZ() { return lz; }
    // Location.kt: regionId = x shr 6 shl 8 or (y shr 6); isInRegion(region) = regionId == region.
    public int getRegionId() { return lx >> 6 << 8 | (ly >> 6); }
    public boolean isInRegion(int region) { return getRegionId() == region; }
    // Location.kt: equality by the packed hash of x, y and z; toString "[$x, $y, $z]".
    @Override public boolean equals(Object o) { return o instanceof Location && ((Location) o).lx == lx && ((Location) o).ly == ly && ((Location) o).lz == lz; }
    @Override public int hashCode() { return (lz << 30) | (lx << 15) | ly; }
    @Override public String toString() { return "[" + lx + ", " + ly + ", " + lz + "]"; }
    public static Location getRandomLocation(Location main, int radius, boolean reachable) {
        oracle.Support.emit("randomLocation", oracle.Support.coord(main), radius, reachable);
        return oracle.Support.location(oracle.Support.answer("randomLocation", java.util.Arrays.asList(3223L, 3219L, 0L)));
    }
}
''',
    # ------------------------------------------------------------------ entity members
    'core/game/world/update/flag/context/Animation.java': r'''package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
public class Animation {
    public static final Animation RESET = new Animation(-1, Priority.VERY_HIGH);
    final int animationId, animationDelay;
    final Priority animationPriority;
    public Animation(int id) { this(id, 0, Priority.MID); }
    public Animation(int id, Priority priority) { this(id, 0, priority); }
    public Animation(int id, int delay) { this(id, delay, Priority.MID); }
    public Animation(int id, int delay, Priority priority) { animationId = id; animationDelay = delay; animationPriority = priority; }
    public static Animation create(int id) { return new Animation(id, 0, Priority.MID); }
    public int getId() { return animationId; }
    public int getDuration() { oracle.Support.emit("animationDuration", animationId); return oracle.Support.answerInt("duration:" + animationId, 1); }
    public java.util.List<Object> oracle() { return java.util.Arrays.asList(animationId, animationDelay, animationPriority == null ? null : animationPriority.name()); }
}
''',
    'core/game/world/update/flag/context/Graphics.java': r'''package core.game.world.update.flag.context;
public class Graphics {
    final int graphicsId, graphicsHeight, graphicsDelay;
    public Graphics(int id) { this(id, 0, 0); }
    public Graphics(int id, int height) { this(id, height, 0); }
    public Graphics(int id, int height, int delay) { graphicsId = id; graphicsHeight = height; graphicsDelay = delay; }
    public java.util.List<Object> oracle() { return java.util.Arrays.asList(graphicsId, graphicsHeight, graphicsDelay); }
}
''',
    'core/game/node/entity/impl/Animator.java': r'''package core.game.node.entity.impl;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import oracle.Support;
public class Animator {
    public static enum Priority { LOW, MID, HIGH, VERY_HIGH }
    public boolean animate(Animation a) { Support.emit("animate", a.oracle()); return true; }
    public boolean animate(Animation a, Graphics g) { Support.emit("visualize", a.oracle(), g.oracle()); return true; }
    public boolean forceAnimation(Animation a) { Support.emit("forceAnimation", a.oracle()); return true; }
    public boolean graphics(Graphics g) { Support.emit("graphics", g.oracle()); return true; }
}
''',
    'core/game/node/entity/impl/PulseManager.java': r'''package core.game.node.entity.impl;
public class PulseManager {
    public void run(core.game.system.task.Pulse pulse) { oracle.Support.emit("pulseRun", oracle.Support.track(pulse), pulse.getDelay()); }
}
''',
    'core/game/node/entity/player/link/PacketDispatch.java': r'''package core.game.node.entity.player.link;
public class PacketDispatch {
    public void sendMessage(String text) { oracle.Support.emit("message", text); }
    public void sendString(String text, int iface, int child) { oracle.Support.emit("interfaceText", text, iface, child); }
    public void sendInterfaceConfig(int iface, int child, boolean hide) { oracle.Support.emit("interfaceConfig", iface, child, hide); }
}
''',
    'core/game/node/entity/player/link/InterfaceManager.java': r'''package core.game.node.entity.player.link;
import core.game.component.Component;
import oracle.Support;
public class InterfaceManager {
    public void open(Component c) { Support.emit("openComponent", c.getId(), Support.track(c.closeEvent)); }
    public void openDefaultTabs() { Support.emit("openDefaultTabs"); }
    public void removeTabs(int... tabs) { Support.emit("removeTabs", Support.values(tabs)); }
    public boolean close() { Support.emit("closeInterface"); return true; }
    public Component getOpened() { Support.emit("getOpened"); Object id = Support.answer("opened", null); return id == null ? null : new Component(((Number) id).intValue()); }
}
''',
    'core/game/node/entity/player/link/appearance/Appearance.java': r'''package core.game.node.entity.player.link.appearance;
public class Appearance {
    public void transformNPC(int id) { oracle.Support.emit("transformNpc", id); }
}
''',
    'core/game/node/entity/player/link/diary/DiaryType.java': 'package core.game.node.entity.player.link.diary;\npublic enum DiaryType { FALADOR }\n',
    'core/game/node/entity/player/link/diary/AchievementDiary.java': r'''package core.game.node.entity.player.link.diary;
import oracle.Support;
public class AchievementDiary {
    final DiaryType diaryType;
    AchievementDiary(DiaryType type) { diaryType = type; }
    public boolean isComplete(int level, int task) { Support.emit("diaryComplete", diaryType.name(), level, task); return Support.answerBool("diaryComplete", false); }
    public void updateTask(core.game.node.entity.player.Player player, int level, int task, boolean complete) { Support.emit("diaryUpdate", diaryType.name(), level, task, complete); }
}
''',
    'core/game/node/entity/player/link/diary/AchievementDiaryManager.java': r'''package core.game.node.entity.player.link.diary;
public class AchievementDiaryManager {
    public AchievementDiary getDiary(DiaryType type) { oracle.Support.emit("getDiary", type.name()); return new AchievementDiary(type); }
}
''',
    'core/game/node/entity/player/link/quest/Quest.java': r'''package core.game.node.entity.player.link.quest;
public class Quest {
    final int points;
    public Quest(int points) { this.points = points; }
    public int getQuestPoints() { return points; }
}
''',
    'core/game/node/entity/player/link/quest/QuestRepository.java': r'''package core.game.node.entity.player.link.quest;
import oracle.Support;
public class QuestRepository {
    public static java.util.Map<String, Quest> getQuests() {
        Support.emit("getQuests");
        java.util.Map<String, Quest> m = new java.util.TreeMap<>();
        java.util.List l = (java.util.List) Support.answers.getOrDefault("questPoints", java.util.Arrays.asList(1L, 2L, 3L));
        for (int i = 0; i < l.size(); i++) m.put("q" + i, new Quest(((Number) l.get(i)).intValue()));
        return m;
    }
    public int getPoints() { Support.emit("questPoints"); return Support.answerInt("points", 6); }
}
''',
    'core/game/node/entity/combat/SkullManager.java': r'''package core.game.node.entity.combat;
public class SkullManager {
    public int getLevel() { oracle.Support.emit("wildernessLevel"); return oracle.Support.answerInt("wildernessLevel", 0); }
}
''',
    'core/game/node/entity/skill/Skills.java': r'''package core.game.node.entity.skill;
import oracle.Support;
public class Skills {
    /* ORIGINAL_SKILL_IDS */
    /* ORIGINAL_SKILL_NAME */
    public int getStaticLevel(int skill) { Support.emit("staticLevel", skill); return Support.answerInt("staticLevel", 1); }
    public void addExperience(int skill, double experience) { Support.emit("addExperience", skill, experience); }
}
''',
    'core/game/container/Container.java': r'''package core.game.container;
import core.game.node.item.Item;
import oracle.Support;
public class Container {
    final String containerName;
    public Container(String name) { containerName = name; }
    public boolean add(Item item) { Support.emit(containerName + ".add", Support.key(item)); return Support.answerBool(containerName + ".add", true); }
    public boolean add(Item... items) { Support.emit(containerName + ".addAll", Support.values(items)); return Support.answerBool(containerName + ".addAll", true); }
    public boolean remove(Item item) { Support.emit(containerName + ".remove", Support.key(item)); return Support.answerBool(containerName + ".remove", true); }
    public boolean remove(Item item, int slot, boolean fireListener) { Support.emit(containerName + ".removeSlot", Support.key(item), slot, fireListener); return Support.answerBool(containerName + ".removeSlot", true); }
    public int freeSlots() { Support.emit(containerName + ".freeSlots"); return Support.answerInt(containerName + ".freeSlots", 28); }
    public Item replace(Item item, int slot) { Support.emit(containerName + ".replace", Support.key(item), slot); return (Item) Support.node(Support.answer(containerName + ".replace", null)); }
    public Item get(int slot) { Support.emit(containerName + ".get", slot); return (Item) Support.node(Support.answer(containerName + ".get", null)); }
    public boolean containsItem(Item item) { Support.emit(containerName + ".containsItem", Support.key(item)); return Support.answerBool(containerName + ".containsItem", false); }
    public Item[] toArray() { Support.emit(containerName + ".toArray"); return Support.items(containerName + ".toArray"); }
    public boolean hasSpaceFor(Item... items) { Support.emit(containerName + ".hasSpaceFor", Support.values(items)); return Support.answerBool(containerName + ".hasSpaceFor", true); }
    public boolean isFull() { Support.emit(containerName + ".isFull"); return Support.answerBool(containerName + ".isFull", false); }
}
''',
    'core/game/node/item/GroundItemManager.java': r'''package core.game.node.item;
public class GroundItemManager {
    public static Object create(Item item, core.game.node.entity.player.Player player) { oracle.Support.emit("groundItem", oracle.Support.key(item)); return null; }
}
''',
    # ------------------------------------------------------------------ components and dialogue
    'core/game/component/Component.java': r'''package core.game.component;
public class Component {
    final int componentId;
    public CloseEvent closeEvent;
    public Component(int id) { componentId = id; }
    public int getId() { return componentId; }
    public Component setCloseEvent(CloseEvent event) { closeEvent = event; return this; }
    public void close(core.game.node.entity.player.Player player) { oracle.Support.emit("componentClose", componentId); }
}
''',
    'core/game/component/CloseEvent.java': r'''package core.game.component;
public interface CloseEvent {
    boolean close(core.game.node.entity.player.Player player, Component c);
}
''',
    'core/game/dialogue/FacialExpression.java': 'package core.game.dialogue;\npublic enum FacialExpression { NEUTRAL, FRIENDLY, HALF_GUILTY, ASKING, HALF_ASKING, HAPPY, HALF_THINKING, OLD_NORMAL }\n',
    'core/game/dialogue/DialogueInterpreter.java': r'''package core.game.dialogue;
import oracle.Support;
public class DialogueInterpreter {
    /** The topics showTopics offers (the interpreter's button handling reads them). */
    public static class TopicList extends java.util.ArrayList<Topic<?>> {
        @Override public boolean add(Topic<?> t) { Support.emit("activeTopic", t.getText()); return super.add(t); }
        @Override public void clear() { Support.emit("clearTopics"); super.clear(); }
    }
    final TopicList topicList = new TopicList();
    public java.util.ArrayList<Topic<?>> getActiveTopics() { return topicList; }
    public static class DialogueHolder { public void loadFile(DialogueFile file) { Support.emit("loadFile", Support.track(file)); } }
    public DialogueHolder getDialogue() { return new DialogueHolder(); }
    public core.game.component.Component sendItemMessage(core.game.node.item.Item item, String... messages) { Support.emit("itemMessage", Support.key(item), Support.values(messages)); return null; }
    public core.game.component.Component sendOptions(Object title, String... options) { Support.emit("sendOptions", title, Support.values(options)); return null; }
    public core.game.component.Component sendDialogues(core.game.node.entity.Entity entity, FacialExpression expression, String... messages) {
        Support.emit("sendDialogues", Support.key(entity), expression == null ? null : expression.name(), Support.values(messages)); return null;
    }
    public core.game.component.Component sendDialogues(int id, FacialExpression expression, String... messages) {
        Support.emit("sendDialogues", id, expression == null ? null : expression.name(), Support.values(messages)); return null;
    }
    public boolean close() { Support.emit("interpreterClose"); return true; }
}
''',
    'core/game/dialogue/DialogueFile.kt': r'''package core.game.dialogue

import core.api.splitLines
import core.game.component.Component
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.tools.START_DIALOGUE

abstract class DialogueFile {
    var player: Player? = null
    var npc: NPC? = null
    var interpreter: DialogueInterpreter? = null
    open var stage = START_DIALOGUE

    abstract fun handle(componentID: Int, buttonID: Int)

    /* ORIGINAL_DIALOGUE_FILE_NPC */

    /* ORIGINAL_DIALOGUE_FILE_PLAYER_PLAIN */

    /* ORIGINAL_DIALOGUE_FILE_PLAYER */

    /* ORIGINAL_DIALOGUE_FILE_NPCL */

    /* ORIGINAL_DIALOGUE_FILE_END */

    /* ORIGINAL_DIALOGUE_FILE_OPTIONS */

    /* ORIGINAL_DIALOGUE_FILE_SHOW_TOPICS */
}
''',
    # ------------------------------------------------------------------ registries
    'core/game/interaction/InteractionListener.kt': r'''package core.game.interaction

import core.api.ContentInterface
import core.game.node.Node
import core.game.node.entity.player.Player

interface InteractionListener : ContentInterface {
    val ITEM
        get() = IntType.ITEM
    val GROUNDITEM
        get() = IntType.GROUNDITEM
    val NPC
        get() = IntType.NPC
    val SCENERY
        get() = IntType.SCENERY
    fun defineListeners()
    fun on(id: Int, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(intArrayOf(id), type.name, arrayOf(*option), handler)
    }
    fun on(ids: IntArray, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(ids, type.name, arrayOf(*option), handler)
    }
    fun onUseWith(type: IntType, used: Int, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        oracle.Support.onUseWith(type.name, intArrayOf(used), with, handler)
    }
    fun onUseWith(type: IntType, used: IntArray, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        oracle.Support.onUseWith(type.name, used, with, handler)
    }
    // InteractionListener.kt: the array form registers each id in turn.
    fun onEquip(id: Int, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.onEquip(id, handler)
    }
    fun onEquip(ids: IntArray, handler: (player: Player, node: Node) -> Boolean) {
        ids.forEach { id -> oracle.Support.onEquip(id, handler) }
    }
}
''',
    'core/game/interaction/InterfaceListener.kt': r'''package core.game.interaction

import core.api.ContentInterface
import core.game.component.Component
import core.game.node.entity.player.Player

interface InterfaceListener : ContentInterface {
    fun defineInterfaceListeners()
    fun on(componentID: Int, handler: (player: Player, component: Component, opcode: Int, buttonID: Int, slot: Int, itemID: Int) -> Boolean) {
        oracle.Support.onInterface(componentID, handler)
    }
    fun onClose(componentID: Int, handler: (player: Player, component: Component) -> Boolean) {
        oracle.Support.onInterfaceClose(componentID, handler)
    }
}
''',
    'core/api/ContentInterface.java': 'package core.api;\npublic interface ContentInterface {}\n',
    'core/plugin/Initializable.java': 'package core.plugin;\npublic @interface Initializable {}\n',
    'core/tools/Log.kt': 'package core.tools\n\nenum class Log { FINE, INFO, WARN, ERR }\n',
    'core/tools/SystemLogger.kt': 'package core.tools\n\nobject SystemLogger\n',
    'core/game/system/communication/CommunicationInfo.java': 'package core.game.system.communication;\npublic class CommunicationInfo {}\n',
    'core/tools/StringUtils.java': r'''package core.tools;
public class StringUtils {
    /* ORIGINAL_IS_PLUS_N */
}
''',
    # ------------------------------------------------------------------ world
    'core/game/system/task/PulseRunner.java': r'''package core.game.system.task;
public class PulseRunner {
    public void submit(Pulse pulse) { oracle.Support.emit("submitWorld", oracle.Support.track(pulse), pulse.getDelay()); }
}
''',
    'core/game/world/GameWorld.kt': r'''package core.game.world

import core.game.system.task.PulseRunner

object GameWorld {
    @JvmStatic
    val ticks: Int get() = oracle.Support.intValue("ticks", 0)
    @JvmStatic
    var Pulser = PulseRunner()
}
''',
    'core/game/node/entity/npc/drop/NPCDropTables.java': r'''package core.game.node.entity.npc.drop;
public class NPCDropTables {
    /* ORIGINAL_DROP_RATES */
}
''',
    # ------------------------------------------------------------------ drop-table neighbours WeightBasedTable names
    'content/global/activity/ttrail/ClueLevel.java': 'package content.global.activity.ttrail;\npublic enum ClueLevel { EASY, MEDIUM, HARD }\n',
    'content/global/activity/ttrail/ClueScrollPlugin.java': r'''package content.global.activity.ttrail;
public class ClueScrollPlugin {
    public static core.game.node.item.Item getClue(ClueLevel level) { oracle.Support.emit("tableRetrieve", "clue:" + level.name()); return null; }
}
''',
    'content/data/tables/RareDropTable.java': r'''package content.data.tables;
public class RareDropTable {
    public static core.game.node.item.Item retrieve(core.game.node.entity.Entity receiver) { oracle.Support.emit("tableRetrieve", "RDT"); return null; }
}
''',
    'content/data/tables/CELEMinorTable.java': 'package content.data.tables;\npublic class CELEMinorTable {\n    public static core.game.node.item.Item retrieve() { oracle.Support.emit("tableRetrieve", "CELEDT"); return null; }\n}\n',
    'content/data/tables/UncommonSeedDropTable.java': 'package content.data.tables;\npublic class UncommonSeedDropTable {\n    public static core.game.node.item.Item retrieve() { oracle.Support.emit("tableRetrieve", "USDT"); return null; }\n}\n',
    'content/data/tables/HerbDropTable.java': 'package content.data.tables;\npublic class HerbDropTable {\n    public static core.game.node.item.Item retrieve() { oracle.Support.emit("tableRetrieve", "HDT"); return null; }\n}\n',
    'content/data/tables/GemDropTable.java': 'package content.data.tables;\npublic class GemDropTable {\n    public static core.game.node.item.Item retrieve() { oracle.Support.emit("tableRetrieve", "GDT"); return null; }\n}\n',
    'content/data/tables/RareSeedDropTable.java': 'package content.data.tables;\npublic class RareSeedDropTable {\n    public static core.game.node.item.Item retrieve() { oracle.Support.emit("tableRetrieve", "RSDT"); return null; }\n}\n',
    'content/data/tables/AllotmentSeedDropTable.java': 'package content.data.tables;\npublic class AllotmentSeedDropTable {\n    public static core.game.node.item.Item retrieve() { oracle.Support.emit("tableRetrieve", "ASDT"); return null; }\n}\n',
    # ------------------------------------------------------------------ unused imports of ToyListeners
    'kotlinx/coroutines/Coroutines.kt': r'''package kotlinx.coroutines

fun delay(timeMillis: Long) {}
fun launch() {}
fun runBlocking() {}
''',
    # ------------------------------------------------------------------ the pet rock's projectile, Diango, the snow globe
    'core/game/node/entity/impl/Projectile.java': r'''package core.game.node.entity.impl;
public class Projectile {
    public static core.game.world.map.Location getLocation(core.game.node.entity.Entity entity) {
        oracle.Support.emit("projectileLocation", oracle.Support.key(entity));
        return oracle.Support.location(oracle.Support.answer("projectileLocation", java.util.Arrays.asList(3222L, 3218L, 0L)));
    }
}
''',
    'core/cache/def/impl/NPCDefinition.java': r'''package core.cache.def.impl;
import java.util.Map;
public class NPCDefinition {
    final int definitionId;
    NPCDefinition(int id) { definitionId = id; }
    public static NPCDefinition forId(int id) { return new NPCDefinition(id); }
    public Map<String, Object> getHandlers() {
        return new java.util.TreeMap<String, Object>() {
            @Override public Object put(String k, Object v) { oracle.Support.npcHandler(definitionId, k, v); return null; }
        };
    }
}
''',
    'core/game/component/ComponentDefinition.java': r'''package core.game.component;
public class ComponentDefinition {
    public static void put(int id, ComponentPlugin plugin) { oracle.Support.componentPlugin(id, plugin); }
}
''',
    'core/game/container/access/InterfaceContainer.java': r'''package core.game.container.access;
import oracle.Support;
public class InterfaceContainer {
    public static int generateItems(core.game.node.entity.player.Player player, core.game.node.item.Item[] itemArray, String[] options, int interfaceIndex, int childIndex, int x, int y) {
        Support.emit("generateItems", Support.values(itemArray), Support.values(options), interfaceIndex, childIndex, x, y);
        return Support.answerInt("generateItems", 0);
    }
}
''',
    'core/plugin/ClassScanner.kt': r'''package core.plugin

object ClassScanner {
    /** ClassScanner.definePlugin for a plugin without a PluginManifest: `newInstance(null)`. */
    @JvmStatic
    fun definePlugin(plugin: Plugin<*>) {
        oracle.Support.emit("definePlugin", oracle.Support.label(plugin))
        @Suppress("UNCHECKED_CAST")
        (plugin as Plugin<Any?>).newInstance(null)
    }
}
''',
    'content/global/skill/construction/decoration/pohstorage/StorageContainer.java': r'''package content.global.skill.construction.decoration.pohstorage;
public class StorageContainer {
    final String familyName;
    StorageContainer(String family) { familyName = family; }
    public boolean contains(int id) { oracle.Support.emit("storageContains", familyName, id); return oracle.Support.answerBool("storageContains:" + id, false); }
}
''',
    'content/global/skill/construction/decoration/pohstorage/StorageState.java': r'''package content.global.skill.construction.decoration.pohstorage;
public class StorageState {
    public StorageContainer getContainer(StorableFamily type) { return new StorageContainer(type.name()); }
}
''',
    'content/global/skill/slayer/SlayerUtils.kt': r'''package content.global.skill.slayer

object SlayerUtils {
    @JvmStatic
    /* ORIGINAL_PLURALISE */
}
''',    # ------------------------------------------------------------------ ContentAPI
    'core/api/ContentAPI.kt': r'''package core.api

import core.game.dialogue.FacialExpression
import core.game.interaction.QueueStrength
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.npc.NPC
import core.game.node.entity.impl.Animator
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.game.node.scenery.Scenery
import core.game.system.task.Pulse
import core.game.world.map.Location
import core.game.world.update.flag.context.Animation
import core.game.world.update.flag.context.Graphics
import core.tools.Log
import content.data.Quests
import org.rs09.consts.NPCs
import oracle.Support

fun sendMessage(player: Player, message: String) { Support.emit("message", message) }
fun sendDialogue(player: Player, message: String) { Support.emit("sendDialogue", message) }
fun sendDialogueLines(player: Player, vararg message: String) { Support.emit("dialogueLines", Support.values(arrayOf<Any>(*message))) }
fun sendItemDialogue(player: Player, item: Any, message: String) { Support.emit("itemDialogue", if (item is Item) Support.key(item) else item, message) }
fun sendPlayerDialogue(player: Player, msg: String, expr: FacialExpression = FacialExpression.NEUTRAL, hide: Boolean = false) {
    Support.emit("playerDialogue", msg, expr.name, hide)
}
fun openInterface(player: Player, id: Int) { Support.emit("openInterface", id) }
fun setInterfaceText(player: Player, string: String, iface: Int, child: Int) { Support.emit("interfaceText", string, iface, child) }
fun closeAllInterfaces(player: Player) { Support.emit("closeAllInterfaces") }
@JvmOverloads
fun hasRequirement(player: Player, quest: Quests, message: Boolean = true): Boolean {
    Support.emit("hasRequirement", quest.name, message)
    return Support.answerBool("hasRequirement:" + quest.name, true)
}
fun isQuestComplete(player: Player, quest: Quests): Boolean { Support.emit("isQuestComplete", quest.name); return Support.answerBool("isQuestComplete:" + quest.name, true) }
fun getQuestStage(player: Player, quest: Quests): Int { Support.emit("questStage", quest.name); return Support.answerInt("questStage:" + quest.name, 0) }
fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    Support.emit("queueScript", delay, strength.name, persist, Support.track(script))
}
fun stopExecuting(entity: Entity): Boolean { Support.emit("stopExecuting"); return Support.answerBool("stopExecuting", true) }
@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: Location? = null, radius: Int = 8) {
    Support.emit("audio", id, delay, loops, Support.coord(location), radius)
}
fun playJingle(player: Player, jingleId: Int) { Support.emit("jingle", jingleId) }
fun stopWalk(entity: Entity) { Support.emit("stopWalk") }
fun lockInteractions(entity: Entity, duration: Int) { Support.emit("lockInteractions", duration) }
fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    Support.emit("removeItem", if (item is Item) Support.key(item) else item, container.name)
    return Support.answerBool("removeItem", true)
}
fun replaceSlot(player: Player, slot: Int, item: Item, currentItem: Item? = null, container: Container = Container.INVENTORY): Item? {
    Support.emit("replaceSlot", slot, Support.key(item), Support.key(currentItem), container.name)
    return Support.node(Support.answer("replaceSlot", Support.key(currentItem))) as Item?
}
fun addItem(player: Player, id: Int, amount: Int = 1, container: Container = Container.INVENTORY): Boolean {
    Support.emit("addItem", id, amount, container.name)
    return Support.answerBool("addItem", true)
}
fun addItemOrDrop(player: Player, id: Int, amount: Int = 1) { Support.emit("addItemOrDrop", id, amount) }
fun amountInInventory(player: Player, id: Int): Int { Support.emit("amountInInventory", id); return Support.answerInt("amountInInventory", 0) }
fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean { Support.emit("inInventory", id, amount); return Support.answerBool("inInventory", false) }
fun inEquipment(player: Player, id: Int, amount: Int = 1): Boolean { Support.emit("inEquipment", id, amount); return Support.answerBool("inEquipment", false) }
fun produceGroundItem(player: Player, item: Int) { Support.emit("produceGroundItem", item) }
fun replaceScenery(toReplace: Scenery, with: Int, forTicks: Int, loc: Location? = null) {
    Support.emit("replaceScenery", Support.key(toReplace), with, forTicks, Support.coord(loc))
}
fun log(origin: Class<*>, type: Log, message: String) { Support.emit("log", origin.name, type.name, message) }
fun openDialogue(player: Player, dialogue: Any, vararg args: Any) { Support.openDialogue(player, dialogue, arrayOf(*args)) }
fun getSlayerMaster(player: Player): NPC {
    Support.emit("slayerMaster")
    return Support.node(Support.answer("slayerMaster", "master")) as NPC
}
fun getSlayerTip(player: Player): Array<out String> {
    Support.emit("slayerTip")
    val l = Support.answers.getOrDefault("slayerTip", listOf("Tip one.", "Tip two.")) as List<*>
    return l.map { it as String }.toTypedArray()
}
fun hasSlayerTask(player: Player): Boolean { Support.emit("hasSlayerTask"); return Support.answerBool("hasSlayerTask", true) }
fun getSlayerTaskName(player: Player): String { Support.emit("slayerTaskName"); return Support.answer("slayerTaskName", "goblin") as String }
fun getSlayerTaskKillsRemaining(player: Player): Int { Support.emit("slayerKillsRemaining"); return Support.answerInt("slayerKillsRemaining", 12) }
fun getSlayerTaskFlags(player: Player): Int { Support.emit("slayerTaskFlags"); return Support.answerInt("slayerTaskFlags", 0) }
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) { Support.emit("setVarp", varpIndex, value, save) }
fun animationDuration(animation: Animation): Int { Support.emit("animationDuration", animation.id); return Support.answerInt("duration:" + animation.id, 1) }
fun spawnProjectile(source: Location, dest: Location, projectile: Int, startHeight: Int, endHeight: Int, delay: Int, speed: Int, angle: Int) {
    Support.emit("projectile", Support.coord(source), Support.coord(dest), projectile, startHeight, endHeight, delay, speed, angle)
}

/* ORIGINAL_CONTENT_API */
''',
}
