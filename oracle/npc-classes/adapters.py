"""Recording adapters for the npc-classes oracle.

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
    public String key;
    public int nodeId;
    public Location nodeLocation;
    public int getId() { return nodeId; }
    public Location getLocation() { return nodeLocation; }
    public core.game.node.entity.npc.NPC asNpc() { return (core.game.node.entity.npc.NPC) this; }
    public core.game.node.entity.player.Player asPlayer() { return (core.game.node.entity.player.Player) this; }
    @Override public String toString() { return key; }
}
''',
    'core/game/node/entity/Entity.java': r'''package core.game.node.entity;
import core.game.node.Node;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import oracle.Support;
import java.util.Map;
public abstract class Entity extends Node {
    public final Map<String, Object> attrs = new java.util.TreeMap<>();
    public Direction oracleDirection;
    final core.game.node.entity.impl.Properties props = new core.game.node.entity.impl.Properties(this);
    final core.game.node.entity.skill.Skills skillSet = new core.game.node.entity.skill.Skills(this);
    final core.game.world.map.zone.ZoneMonitor zones = new core.game.world.map.zone.ZoneMonitor(this);
    public core.game.node.entity.impl.Properties getProperties() { return props; }
    public core.game.node.entity.skill.Skills getSkills() { return skillSet; }
    public core.game.world.map.zone.ZoneMonitor getZoneMonitor() { return zones; }
    public abstract core.game.node.entity.combat.CombatSwingHandler getSwingHandler(boolean swing);
    public Direction getDirection() { Support.emit("direction", key); return oracleDirection; }
    public void setDirection(Direction direction) { Support.emit("setDirection", key, direction == null ? null : direction.name()); oracleDirection = direction; }
    public boolean sendChat(String text) { Support.emit("sendChat", key, text); return Support.answerBool("sendChat", true); }
    public void faceLocation(Location l) { Support.emit("faceLocation", key, Support.coord(l)); }
    public boolean inCombat() { Support.emit("inCombat", key); return Support.answerBool("inCombat:" + key, false); }
    public boolean animate(Animation animation) { Support.emit("animate", key, animation == null ? null : animation.oracleId()); return true; }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k, T fail) { Support.emit("getAttribute", key, k); return attrs.containsKey(k) ? (T) attrs.get(k) : fail; }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k) { Support.emit("getAttribute", key, k); return (T) attrs.get(k); }
    public void setAttribute(String k, Object v) { Support.emit("setAttribute", key, k, Support.value(v)); attrs.put(k, v); }
    public void removeAttribute(String k) { Support.emit("removeAttribute", key, k); attrs.remove(k); }
}
''',
    'core/game/node/entity/impl/Properties.java': r'''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import oracle.Support;
public class Properties {
    final Entity owner;
    final core.game.node.entity.combat.CombatPulse pulse;
    public Properties(Entity owner) { this.owner = owner; pulse = new core.game.node.entity.combat.CombatPulse(owner); }
    public core.game.node.entity.combat.CombatPulse getCombatPulse() { return pulse; }
    public void setAutocastSpell(core.game.node.entity.combat.spell.CombatSpell spell) {
        Support.emit("setAutocastSpell", owner.key, spell == null ? null : spell.oracleId);
    }
}
''',
    'core/game/node/entity/combat/CombatPulse.java': r'''package core.game.node.entity.combat;
import core.game.node.Node;
import core.game.node.entity.Entity;
import oracle.Support;
public class CombatPulse {
    final Entity owner;
    public CombatPulse(Entity owner) { this.owner = owner; }
    public Entity getVictim() { Support.emit("victim", owner.key); return (Entity) Support.node(Support.answer("victim:" + owner.key, null)); }
    public boolean isAttacking() { Support.emit("isAttacking", owner.key); return Support.answerBool("isAttacking:" + owner.key, false); }
    public void setNextAttack(int ticks) { Support.emit("setNextAttack", owner.key, ticks); }
    public void setStyle(CombatStyle style) { Support.emit("setStyle", owner.key, style == null ? null : style.name()); }
    public void setCombatFlags(Entity victim) { Support.emit("setCombatFlags", owner.key, Support.key(victim)); }
    public void attack(Node victim) { Support.emit("combatAttack", owner.key, Support.key(victim)); }
}
''',
    'core/game/node/entity/combat/CombatStyle.java': 'package core.game.node.entity.combat;\npublic enum CombatStyle { MELEE, RANGE, MAGIC }\n',
    'core/game/node/entity/combat/BattleState.java': 'package core.game.node.entity.combat;\npublic class BattleState {}\n',
    'core/game/node/entity/combat/InteractionType.java': 'package core.game.node.entity.combat;\npublic enum InteractionType { STILL_INTERACT, MOVE_INTERACT, NO_INTERACT }\n',
    'core/game/node/entity/combat/CombatSwingHandler.java': r'''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.Support;
public class CombatSwingHandler {
    public final String oracleRef;
    public CombatSwingHandler() { this("default"); }
    public CombatSwingHandler(String ref) { oracleRef = ref; }
    public int swing(Entity entity, Entity victim, BattleState state) {
        Support.emit("baseSwing", oracleRef, Support.key(entity), Support.key(victim));
        return Support.answerInt("baseSwing", 0);
    }
    public InteractionType canSwing(Entity entity, Entity victim) {
        Support.emit("canSwing", oracleRef, Support.key(entity), Support.key(victim));
        Object v = Support.answer("canSwing", null);
        return v == null ? null : InteractionType.valueOf((String) v);
    }
}
''',
    'core/game/node/entity/combat/MeleeSwingHandler.java': r'''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.Support;
public class MeleeSwingHandler extends CombatSwingHandler {
    public MeleeSwingHandler() { super("melee"); }
    @Override public int swing(Entity entity, Entity victim, BattleState state) {
        Support.emit("meleeSwing", Support.key(entity), Support.key(victim));
        return Support.answerInt("meleeSwing", 4);
    }
}
''',
    'core/game/node/entity/combat/DeathTask.java': r'''package core.game.node.entity.combat;
import core.game.node.Node;
public class DeathTask {
    public static boolean isDead(Node e) { oracle.Support.emit("isDead", oracle.Support.key(e)); return oracle.Support.answerBool("isDead", false); }
}
''',
    'core/game/node/entity/combat/spell/CombatSpell.java': r'''package core.game.node.entity.combat.spell;
public class CombatSpell {
    public final int oracleId;
    public CombatSpell(int id) { oracleId = id; }
}
''',
    'core/game/node/entity/player/link/SpellBookManager.java': r'''package core.game.node.entity.player.link;
import oracle.Support;
public class SpellBookManager {
    public enum SpellBook {
        MODERN, ANCIENT, LUNAR;
        public Object getSpell(int buttonId) {
            Support.emit("getSpell", name(), buttonId);
            Object v = Support.answer("getSpell:" + buttonId, buttonId);
            if (v == null) return null;
            if ("other".equals(v)) return "not a combat spell";
            return new core.game.node.entity.combat.spell.CombatSpell(((Number) v).intValue());
        }
    }
}
''',
    'content/global/skill/slayer/Tasks.java': r'''package content.global.skill.slayer;
public class Tasks {
    public int levelReq;
    public Tasks(int levelReq) { this.levelReq = levelReq; }
}
''',
    # The NPC base class: constructor, the members the subclasses reach and the original NPC.create.
    'core/game/node/entity/npc/NPC.java': r'''package core.game.node.entity.npc;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import oracle.Support;
public class NPC extends Entity {
    public boolean oracleWalks, oracleAggressive;
    public int oracleRadius = 11;
    /** A node the case sets up (no constructor call is recorded). */
    public NPC() {}
    public NPC(int id) { this(id, null); }
    public NPC(int id, Location location) { this(id, location, Direction.SOUTH); }
    protected NPC(int id, Location location, Direction direction) {
        nodeId = id;
        nodeLocation = location;
        oracleDirection = direction;
        Support.emit("newNPC", getClass().getName(), id, Support.coord(location), direction == null ? null : direction.name());
    }
    /* ORIGINAL_NPC_CREATE */
    public void init() { Support.emit("npcInit", key); }
    public void tick() { Support.emit("npcTick", key); }
    public void handleTickActions() { Support.emit("npcTickActions", key); }
    public void finalizeDeath(Entity killer) { Support.emit("npcFinalizeDeath", key, Support.key(killer)); }
    public void onAttack(Entity target) { Support.emit("npcOnAttack", key, Support.key(target)); }
    public void onImpact(Entity entity, BattleState state) { Support.emit("npcOnImpact", key, Support.key(entity)); }
    public CombatSwingHandler getSwingHandler(boolean swing) { Support.emit("npcSwingHandler", key, swing); return new CombatSwingHandler("npc"); }
    public int getWalkRadius() { Support.emit("walkRadius", key); return oracleRadius; }
    public boolean isWalks() { Support.emit("walks", key); return oracleWalks; }
    public void setWalks(boolean walks) { Support.emit("setWalks", key, walks); oracleWalks = walks; }
    public boolean isAggressive() { Support.emit("aggressive", key); return oracleAggressive; }
    public void setAggressive(boolean aggressive) { Support.emit("setAggressive", key, aggressive); oracleAggressive = aggressive; }
    public NPC transform(int id) { Support.emit("transform", key, id); return this; }
    public void reTransform() { Support.emit("reTransform", key); }
    public boolean isInvisible() { Support.emit("isInvisible", key); return Support.answerBool("isInvisible", false); }
    public boolean isAttackable(Entity entity, core.game.node.entity.combat.CombatStyle style, boolean message) { throw new IllegalStateException("NPC.isAttackable"); }
    public content.global.skill.slayer.Tasks getTask() {
        Support.emit("task", key);
        Object v = Support.answer("task", null);
        return v == null ? null : new content.global.skill.slayer.Tasks(((Number) v).intValue());
    }
}
''',
    'core/game/node/entity/npc/OracleRegistry.java': r'''package core.game.node.entity.npc;
public class OracleRegistry {
    public static void reset() { AbstractNPC.mapping.clear(); }
}
''',
    # A concrete IdleAbstractNPC so that the abstract class's own members can run.
    'oracle/IdleProbe.java': r'''package oracle;
import core.game.node.entity.npc.AbstractNPC;
import core.game.node.entity.npc.IdleAbstractNPC;
import core.game.world.map.Location;
public class IdleProbe extends IdleAbstractNPC {
    public IdleProbe(int idleId, int activeId, Location location) { super(idleId, activeId, location); }
    public int oracleTimeout() { return timeout; }
    public void oracleSetTimeout(int t) { timeout = t; }
    @Override public AbstractNPC construct(int id, Location location, Object... objects) { return null; }
    @Override public int[] getIds() { return new int[0]; }
}
''',
    # Two AbstractNPC classes that claim ids another class has: one with its own simple name, one named like ShopNPC.
    'oracle/ConflictProbe.java': r'''package oracle;
import core.game.node.entity.npc.AbstractNPC;
import core.game.world.map.Location;
public class ConflictProbe extends AbstractNPC {
    public ConflictProbe() { super(0, null); }
    ConflictProbe(int id, Location location) { super(id, location); }
    @Override public AbstractNPC construct(int id, Location location, Object... objects) { return new ConflictProbe(id, location); }
    @Override public int[] getIds() { return new int[] { 520, 4999999, 46 }; }
}
''',
    'oracle/probe/ShopNPC.java': r'''package oracle.probe;
import core.game.node.entity.npc.AbstractNPC;
import core.game.world.map.Location;
public class ShopNPC extends AbstractNPC {
    public ShopNPC() { super(0, null); }
    @Override public AbstractNPC construct(int id, Location location, Object... objects) { return null; }
    @Override public int[] getIds() { return new int[] { 4999998, 520 }; }
}
''',
    'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
import core.game.node.entity.combat.CombatSwingHandler;
import oracle.Support;
public class Player extends core.game.node.entity.Entity {
    final core.game.dialogue.DialogueInterpreter interpreter = new core.game.dialogue.DialogueInterpreter(this);
    final core.game.node.entity.player.link.PacketDispatch dispatch = new core.game.node.entity.player.link.PacketDispatch(this);
    final core.game.node.entity.player.link.quest.QuestRepository quests = new core.game.node.entity.player.link.quest.QuestRepository(this);
    final core.game.node.entity.player.link.diary.AchievementDiaryManager diaries = new core.game.node.entity.player.link.diary.AchievementDiaryManager(this);
    public core.game.dialogue.DialogueInterpreter getDialogueInterpreter() { return interpreter; }
    public core.game.node.entity.player.link.PacketDispatch getPacketDispatch() { return dispatch; }
    public core.game.node.entity.player.link.quest.QuestRepository getQuestRepository() { return quests; }
    public core.game.node.entity.player.link.diary.AchievementDiaryManager getAchievementDiaryManager() { return diaries; }
    public void sendMessage(String message) { Support.emit("playerMessage", key, message); }
    @Override public CombatSwingHandler getSwingHandler(boolean swing) { Support.emit("swingHandler", key, swing); return new CombatSwingHandler("player"); }
}
''',
    'core/game/node/entity/player/link/PacketDispatch.java': r'''package core.game.node.entity.player.link;
public class PacketDispatch {
    final core.game.node.entity.player.Player owner;
    public PacketDispatch(core.game.node.entity.player.Player owner) { this.owner = owner; }
    public void sendMessage(String message) { oracle.Support.emit("packetMessage", owner.key, message); }
}
''',
    'core/game/node/entity/player/link/IronmanMode.java': 'package core.game.node.entity.player.link;\npublic enum IronmanMode { NONE, STANDARD, HARDCORE, ULTIMATE }\n',
    'core/game/node/entity/player/link/quest/Quest.java': r'''package core.game.node.entity.player.link.quest;
import core.game.node.entity.player.Player;
public class Quest {
    final String name;
    public Quest(String name) { this.name = name; }
    public boolean isStarted(Player player) { oracle.Support.emit("questStarted", name, player.key); return oracle.Support.answerBool("questStarted", false); }
}
''',
    'core/game/node/entity/player/link/quest/QuestRepository.java': r'''package core.game.node.entity.player.link.quest;
public class QuestRepository {
    final core.game.node.entity.player.Player owner;
    public QuestRepository(core.game.node.entity.player.Player owner) { this.owner = owner; }
    public Quest getQuest(content.data.Quests quest) {
        oracle.Support.emit("getQuest", owner.key, quest == null ? null : quest.name());
        return oracle.Support.answerBool("questMissing", false) ? null : new Quest(quest.name());
    }
}
''',
    'content/data/Quests.java': 'package content.data;\npublic enum Quests { COOKS_ASSISTANT, WITCHS_POTION }\n',
    'core/game/node/entity/player/link/diary/DiaryType.java': 'package core.game.node.entity.player.link.diary;\npublic enum DiaryType { KARAMJA, VARROCK, LUMBRIDGE, FALADOR }\n',
    'core/game/node/entity/player/link/diary/AchievementDiary.java': r'''package core.game.node.entity.player.link.diary;
import core.game.node.entity.player.Player;
import oracle.Support;
public class AchievementDiary {
    final DiaryType type;
    public AchievementDiary(DiaryType type) { this.type = type; }
    public boolean isComplete(int level, int task) { Support.emit("diaryComplete", type.name(), level, task); return Support.answerBool("diaryComplete", false); }
    public void updateTask(Player player, int level, int index, boolean complete) { Support.emit("diaryUpdate", type.name(), player.key, level, index, complete); }
}
''',
    'core/game/node/entity/player/link/diary/AchievementDiaryManager.java': r'''package core.game.node.entity.player.link.diary;
public class AchievementDiaryManager {
    final core.game.node.entity.player.Player owner;
    public AchievementDiaryManager(core.game.node.entity.player.Player owner) { this.owner = owner; }
    public AchievementDiary getDiary(DiaryType type) {
        oracle.Support.emit("getDiary", owner.key, type == null ? null : type.name());
        return oracle.Support.answerBool("diaryMissing", false) ? null : new AchievementDiary(type);
    }
}
''',
    'core/game/node/entity/skill/Skills.java': r'''package core.game.node.entity.skill;
import core.game.node.entity.Entity;
import oracle.Support;
public class Skills {
    /* ORIGINAL_SKILL_IDS */
    final Entity owner;
    public Skills(Entity owner) { this.owner = owner; }
    public int getLevel(int skill) { Support.emit("level", owner.key, skill); return Support.answerInt("level", 1); }
    public int getLifepoints() { Support.emit("lifepoints", owner.key); return Support.answerInt("lifepoints", 10); }
    public int getMaximumLifepoints() { Support.emit("maximumLifepoints", owner.key); return Support.answerInt("maximumLifepoints", 10); }
    public int heal(int amount) { Support.emit("heal", owner.key, amount); return 0; }
}
''',
    'core/game/world/map/zone/ZoneMonitor.java': r'''package core.game.world.map.zone;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatStyle;
public class ZoneMonitor {
    final Entity owner;
    public ZoneMonitor(Entity owner) { this.owner = owner; }
    public boolean continueAttack(Node target, CombatStyle style, boolean message) {
        oracle.Support.emit("continueAttack", owner.key, oracle.Support.key(target), style == null ? null : style.name(), message);
        return oracle.Support.answerBool("continueAttack", true);
    }
}
''',
    # Location: the constructor's plane and 15-bit packing (Location.kt and its LocationHash), the transforms and
    # withinDistance with getDistance as the original defines them.
    'core/game/world/map/Location.java': r'''package core.game.world.map;
public class Location {
    final int hashValue;
    static int unpack(int raw) { return raw >= 0x4000 ? raw - 0x8000 : raw; }
    public Location(int x, int y) { this(x, y, 0); }
    public Location(int x, int y, int z) {
        int plane = (z % 4 + 4) % 4;
        hashValue = ((plane & 0x3) << 30) | ((x & 0x7FFF) << 15) | (y & 0x7FFF);
    }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public static Location create(int x, int y) { return new Location(x, y, 0); }
    public int getX() { return unpack((hashValue >>> 15) & 0x7FFF); }
    public int getY() { return unpack(hashValue & 0x7FFF); }
    public int getZ() { return hashValue >>> 30; }
    public int getRegionId() { return getX() >> 6 << 8 | (getY() >> 6); }
    public Location transform(int diffX, int diffY, int z) { return new Location(getX() + diffX, getY() + diffY, (getZ() + z) % 4); }
    public Location transform(Direction dir) { return transform(dir, 1); }
    public Location transform(Direction dir, int steps) { return new Location(getX() + dir.getStepX() * steps, getY() + dir.getStepY() * steps, getZ()); }
    public static double getDistance(Location first, Location second) {
        int xDiff = first.getX() - second.getX();
        int yDiff = first.getY() - second.getY();
        return Math.sqrt((double) (xDiff * xDiff + yDiff * yDiff));
    }
    public boolean withinDistance(Location other, int dist) {
        if (other.getZ() != getZ()) return false;
        return getDistance(this, other) <= dist;
    }
    @Override public boolean equals(Object o) { return o instanceof Location && ((Location) o).hashValue == hashValue; }
    @Override public int hashCode() { return hashValue; }
}
''',
    'core/game/world/map/path/ClipMaskSupplier.java': 'package core.game.world.map.path;\npublic interface ClipMaskSupplier { int getClippingFlag(int z, int x, int y); }\n',
    'core/game/world/map/Point.java': 'package core.game.world.map;\npublic class Point { public final int px, py; public Point(int x, int y) { px = x; py = y; } }\n',
    'core/game/world/map/RegionManager.java': r'''package core.game.world.map;
public class RegionManager {
    public static int getClippingFlag(int z, int x, int y) { throw new IllegalStateException("clipping"); }
}
''',
    'core/game/world/update/flag/context/Animation.java': r'''package core.game.world.update.flag.context;
public class Animation {
    final int animationId;
    public Animation(int id) { animationId = id; }
    public static Animation create(int id) { return new Animation(id); }
    public int oracleId() { return animationId; }
}
''',
    'core/game/node/item/Item.java': r'''package core.game.node.item;
public class Item extends core.game.node.Node {
    public final int oracleAmount;
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { nodeId = id; oracleAmount = amount; }
}
''',
    'core/game/node/item/GroundItemManager.java': r'''package core.game.node.item;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
public class GroundItemManager {
    public static Object create(Item item, Location location, Player player) {
        oracle.Support.emit("groundItem", item.getId(), item.oracleAmount, oracle.Support.coord(location), oracle.Support.key(player));
        return null;
    }
}
''',
    'core/game/node/scenery/Scenery.java': 'package core.game.node.scenery;\npublic class Scenery extends core.game.node.Node {}\n',
    'core/cache/def/impl/SceneryDefinition.java': r'''package core.cache.def.impl;
public class SceneryDefinition {
    public static SceneryDefinition forId(int id) { throw new IllegalStateException("not reached"); }
    public int[] getChildrenIds() { return null; }
}
''',
    'core/cache/def/impl/NPCDefinition.java': r'''package core.cache.def.impl;
import core.game.interaction.OptionHandler;
import oracle.Support;
public class NPCDefinition {
    final int definitionId;
    final java.util.Map<String, Object> handlerMap = new java.util.LinkedHashMap<String, Object>() {
        @Override public Object put(String k, Object v) { Support.emit("npcHandler", definitionId, k, Support.handler(v)); Support.handlers.put("npc:" + definitionId + ":" + k, (OptionHandler) v); return super.put(k, v); }
    };
    NPCDefinition(int id) { definitionId = id; }
    public static NPCDefinition forId(int id) { Support.emit("npcDefinition", id); return new NPCDefinition(id); }
    public java.util.Map<String, Object> getHandlers() { return handlerMap; }
    public static boolean setOptionHandler(String name, OptionHandler handler) {
        Support.emit("npcOptionHandler", name, Support.handler(handler));
        Support.handlers.put("option:" + name, handler);
        return true;
    }
}
''',
    # ------------------------------------------------------------------ dialogue
    'core/game/dialogue/DialogueInterpreter.java': r'''package core.game.dialogue;
import core.game.node.entity.player.Player;
import oracle.Support;
public class DialogueInterpreter {
    final Player owner;
    public DialogueInterpreter(Player owner) { this.owner = owner; }
    public boolean open(String dialogueType, Object... args) {
        Support.emit("openDialogueName", owner.key, dialogueType, Support.values(args));
        return Support.answerBool("openDialogueName", true);
    }
    public boolean open(int dialogueKey, Object... args) {
        Support.emit("openDialogueId", owner.key, dialogueKey, Support.values(args));
        return Support.answerBool("openDialogueId", true);
    }
    public void open(DialogueFile file, Object... args) {
        Support.emit("openFile", owner.key, file == null ? null : file.getClass().getName(), Support.values(args));
    }
    public core.game.component.Component sendDialogue(String... messages) {
        Support.emit("sendDialogue", owner.key, Support.values(messages));
        return null;
    }
}
''',
    'core/game/component/Component.java': 'package core.game.component;\npublic class Component {}\n',
    'core/game/dialogue/DialogueFile.kt': 'package core.game.dialogue\n\nabstract class DialogueFile\n',
    'core/game/dialogue/ChatAnim.kt': 'package core.game.dialogue\n\nenum class ChatAnim { NEUTRAL, HAPPY, ANNOYED, ANGRY, WORRIED, ASKING, FRIENDLY }\n',
    # Only DialogueLabeller.open is reached by the listeners; the conversation's builder calls compile and do nothing.
    'core/game/dialogue/DialogueLabeller.kt': r'''package core.game.dialogue

import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player

class DialogueOption(
    val goto: String,
    val option: String,
    val spokenText: String = option,
    val expression: ChatAnim = ChatAnim.NEUTRAL,
    val skipPlayer: Boolean = false,
    val optionIf: ((player: Player, npc: NPC) -> Boolean)? = null
)

abstract class DialogueLabeller : DialogueFile() {
    companion object {
        fun open(player: Player, dialogue: Any, npc: NPC) {
            oracle.Support.emit("labellerOpen", player.key, dialogue.javaClass.name, oracle.Support.key(npc))
        }
    }
    abstract fun addConversation()
    fun label(vararg label: String, nesting: () -> Unit = {}) {}
    fun goto(label: String) {}
    fun loadLabel(player: Player, label: String) {}
    fun exec(callback: (player: Player, npc: NPC) -> Unit) {}
    fun player(chatAnim: ChatAnim = ChatAnim.NEUTRAL, vararg messages: String, unclosable: Boolean = false) {}
    fun player(vararg messages: String) {}
    fun npc(chatAnim: ChatAnim = ChatAnim.NEUTRAL, vararg messages: String, unclosable: Boolean = false) {}
    fun npc(vararg messages: String) {}
    fun options(vararg options: DialogueOption, title: String = "Select an Option", unclosable: Boolean = false) {}
}
''',
    # ------------------------------------------------------------------ content the listeners reach
    'content/minigame/gnomecooking/GnomeTipper.kt': 'package content.minigame.gnomecooking\n\nobject GnomeTipper { enum class LEVEL { EASY, HARD } }\n',
    'content/minigame/gnomecooking/GCCompletionDialogue.kt': r'''package content.minigame.gnomecooking

import core.game.dialogue.DialogueFile

class GCCompletionDialogue(val job: GnomeCookingJob) : DialogueFile() {
    init { oracle.Support.emit("gcCompletionDialogue", job.name) }
}
''',
    'content/region/kandarin/barcrawl/BarcrawlType.java': r'''package content.region.kandarin.barcrawl;
public enum BarcrawlType {
    /* ORIGINAL_BARCRAWL_TYPES */;
    private final int[] npc;
    BarcrawlType(int... npc) { this.npc = npc; }
    /* ORIGINAL_BARCRAWL_FOR_ID */
    public int[] getNpc() { return npc; }
}
''',
    'content/region/kandarin/barcrawl/BarcrawlManager.java': r'''package content.region.kandarin.barcrawl;
import core.game.node.entity.player.Player;
import oracle.Support;
public final class BarcrawlManager {
    final Player owner;
    BarcrawlManager(Player owner) { this.owner = owner; }
    public static BarcrawlManager getInstance(Player player) { Support.emit("barcrawlInstance", player.key); return new BarcrawlManager(player); }
    public boolean isFinished() { Support.emit("barcrawlFinished", owner.key); return Support.answerBool("barcrawlFinished", false); }
    public boolean isStarted() { Support.emit("barcrawlStarted", owner.key); return Support.answerBool("barcrawlStarted", false); }
    public boolean isCompleted(int index) { Support.emit("barcrawlCompleted", owner.key, index); return Support.answerBool("barcrawlCompleted", false); }
}
''',
    'content/global/ame/RandomEventNPC.kt': r'''package content.global.ame

import core.game.node.entity.npc.NPC

open class RandomEventNPC(id: Int) : NPC(id) {
    var finalized: Boolean
        get() { oracle.Support.emit("finalized", key); return oracle.Support.answerBool("finalized:" + key, false) }
        set(value) {}
    open fun talkTo(npc: NPC) { oracle.Support.emit("talkTo", key, oracle.Support.key(npc)) }
}
''',
    'content/global/ame/RandomEvents.kt': r'''package content.global.ame

enum class RandomEvents {
    ;
    companion object {
        val randomIDs: List<Int>
            get() { oracle.Support.emit("randomIDs"); return oracle.Support.ints("randomIDs") }
    }
}
''',
    'core/game/system/timer/impl/AntiMacro.kt': r'''package core.game.system.timer.impl

import content.global.ame.RandomEventNPC
import core.game.node.entity.player.Player

class AntiMacro {
    companion object {
        fun getEventNpc(player: Player): RandomEventNPC? {
            oracle.Support.emit("antiMacroEventNpc", player.key)
            return oracle.Support.node(oracle.Support.answer("antiMacroEventNpc", null)) as RandomEventNPC?
        }
    }
}
''',
    'core/game/worldevents/holiday/HolidayRandomEventNPC.kt': r'''package core.game.worldevents.holiday

import core.game.node.entity.npc.NPC

open class HolidayRandomEventNPC(id: Int) : NPC(id) {
    var finalized: Boolean
        get() { oracle.Support.emit("finalized", key); return oracle.Support.answerBool("finalized:" + key, false) }
        set(value) {}
    open fun talkTo(npc: NPC) { oracle.Support.emit("talkTo", key, oracle.Support.key(npc)) }
}
''',
    'core/game/worldevents/holiday/HolidayRandomEvents.kt': r'''package core.game.worldevents.holiday

enum class HolidayRandomEvents {
    ;
    companion object {
        val holidayRandomIDs: List<Int>
            get() { oracle.Support.emit("holidayRandomIDs"); return oracle.Support.ints("holidayRandomIDs") }
    }
}
''',
    'core/game/worldevents/holiday/HolidayRandoms.kt': r'''package core.game.worldevents.holiday

import core.game.node.entity.player.Player

class HolidayRandoms {
    companion object {
        fun getEventNpc(player: Player): HolidayRandomEventNPC? {
            oracle.Support.emit("holidayEventNpc", player.key)
            return oracle.Support.node(oracle.Support.answer("holidayEventNpc", null)) as HolidayRandomEventNPC?
        }
    }
}
''',
    'content/global/handlers/scenery/BankBoothListener.kt': r'''package content.global.handlers.scenery

import org.rs09.consts.Scenery

class BankBoothListener {
    companion object {
        /* ORIGINAL_BANK_BOOTHS */
    }
}
''',
    # ------------------------------------------------------------------ registries
    'core/game/interaction/InteractionListener.kt': r'''package core.game.interaction

import core.api.ContentInterface
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.map.Location

interface InteractionListener : ContentInterface {
    fun defineListeners()
    fun defineDestinationOverrides() {}
    fun on(id: Int, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(intArrayOf(id), type.name, arrayOf(*option), handler)
    }
    fun on(ids: IntArray, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(ids, type.name, arrayOf(*option), handler)
    }
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(null, type.name, arrayOf(*option), handler)
    }
    fun setDest(type: IntType, ids: IntArray, vararg options: String, handler: (Entity, Node) -> Location) {
        oracle.Support.setDest(type.name, ids, arrayOf(*options), handler)
    }
}
''',
    'core/api/ContentInterface.java': 'package core.api;\npublic interface ContentInterface {}\n',
    'core/plugin/Initializable.java': 'package core.plugin;\npublic @interface Initializable {}\n',
    'core/ServerConstants.java': r'''package core;
public class ServerConstants {
    public static boolean SECOND_BANK = true;
}
''',
    'core/tools/Log.kt': 'package core.tools\n\nenum class Log { FINE, INFO, WARN, ERR, DEBUG }\n',
    'core/tools/SystemLogger.kt': 'package core.tools\n\nobject SystemLogger\n',
    'core/tools/RandomFunction.java': r'''package core.tools;
import oracle.Support;
public class RandomFunction {
    public static int random(int maxValue) { Support.emit("random", maxValue); return Support.answerInt("random", 0); }
    public static int randomize(int maxValue) { Support.emit("randomize", maxValue); return Support.answerInt("randomize", 0); }
}
''',
    # ------------------------------------------------------------------ ContentAPI
    'core/api/ContentAPI.kt': r'''package core.api

import core.game.node.entity.Entity
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.IronmanMode
import core.game.node.item.Item
import core.game.node.scenery.Scenery
import core.game.world.map.Location
import core.tools.Log
import oracle.Support

fun log(origin: Class<*>, type: Log, message: String) { Support.emit("log", origin.name, type.name, message) }
fun sendMessage(player: Player, message: String) { Support.emit("apiMessage", player.key, message) }
@Suppress("UNCHECKED_CAST")
fun <T> getAttribute(entity: Entity, attribute: String, default: T): T {
    Support.emit("apiGetAttribute", entity.key, attribute)
    return if (entity.attrs.containsKey(attribute)) entity.attrs[attribute] as T else default
}
fun openDialogue(player: Player, dialogue: Any, vararg args: Any) { Support.emit("apiOpenDialogue", player.key, Support.value(dialogue), Support.values(args)) }
fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    Support.emit("removeItem", player.key, Support.value(item), container.name)
    return Support.answerBool("removeItem", false)
}
fun openBankAccount(player: Player) { Support.emit("openBankAccount", player.key) }
fun openGrandExchangeCollectionBox(player: Player) { Support.emit("openCollectionBox", player.key) }
fun getScenery(loc: Location): Scenery? {
    Support.emit("getScenery", Support.coord(loc))
    return Support.node(Support.answer("scenery:" + loc.x + "," + loc.y + "," + loc.z, null)) as Scenery?
}
fun hasSealOfPassage(player: Player): Boolean { Support.emit("hasSealOfPassage", player.key); return Support.answerBool("hasSealOfPassage", false) }
fun hasIronmanRestriction(player: Player, restriction: IronmanMode): Boolean { Support.emit("ironmanRestriction", player.key, restriction.name); return Support.answerBool("ironmanRestriction", false) }
fun hasAwaitingGrandExchangeCollections(player: Player): Boolean { Support.emit("awaitingCollections", player.key); return false }
fun hasActivatedSecondaryBankAccount(player: Player): Boolean { Support.emit("secondaryActivated", player.key); return false }
fun isUsingSecondaryBankAccount(player: Player): Boolean { Support.emit("usingSecondary", player.key); return false }
enum class SecondaryBankAccountActivationResult { ALREADY_ACTIVE, NOT_ENOUGH_MONEY, INTERNAL_FAILURE, SUCCESS }
fun activateSecondaryBankAccount(player: Player): SecondaryBankAccountActivationResult { Support.emit("activateSecondary", player.key); return SecondaryBankAccountActivationResult.SUCCESS }
fun toggleBankAccount(player: Player) { Support.emit("toggleBankAccount", player.key) }
fun openBankPinSettings(player: Player) { Support.emit("openBankPinSettings", player.key) }
''',
}
