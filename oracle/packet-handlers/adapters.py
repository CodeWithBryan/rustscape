"""Recording adapters for the packet-handlers oracle.

Each entry is a source file compiled with the unchanged originals. `/* ORIGINAL_* */` markers are replaced by original text
read from the pinned sources at generation time (see generate.py). Everything else records the call in oracle.Support and
answers from the case.
"""

ADAPTERS = {
    'core/net/packet/PacketProcessor.kt': r'''package core.net.packet

import content.global.skill.magic.SpellListener
import content.global.skill.magic.SpellListeners
import content.global.skill.magic.SpellUtils
import content.global.skill.summoning.familiar.FamiliarSpecial
import core.ServerConstants
import core.api.getAttribute
import core.api.sendMessage
import core.cache.def.impl.ItemDefinition
import core.cache.def.impl.NPCDefinition
import core.cache.def.impl.SceneryDefinition
import core.game.container.Container
import core.game.container.impl.BankContainer
import core.game.interaction.*
import core.game.node.Node
import core.game.node.entity.combat.spell.MagicSpell
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.SpellBookManager
import core.game.node.item.GroundItemManager
import core.game.node.item.Item
import core.game.world.GameWorld
import core.game.world.map.Location
import core.game.world.map.RegionManager
import core.game.world.repository.Repository
import core.net.packet.context.PlayerContext
import core.net.packet.`in`.Packet
import core.net.packet.out.ClearMinimapFlag
import org.rs09.consts.Components

/** The selected members of the original object; the entry points call them. */
object PacketProcessor {
    @JvmStatic fun item(pkt: Packet.ItemAction) = processItemAction(pkt)
    @JvmStatic fun npc(pkt: Packet.NpcAction) = processNpcAction(pkt)
    @JvmStatic fun player(pkt: Packet.PlayerAction) = processPlayerAction(pkt)
    @JvmStatic fun useWith(pkt: Packet) = processUseWith(pkt)
    @JvmStatic fun component(pkt: Packet) = processComponentUseWith(pkt)
    @JvmStatic fun slotSwitch(pkt: Packet) = processSlotSwitch(pkt)

    /** The original `process` with only the four selected branches. */
    @JvmStatic fun process(pkt: Packet) {
        when (pkt) {
            /* ORIGINAL_PROCESS_ARMS */
            else -> {}
        }
    }

    /* ORIGINAL_PROCESSOR_METHODS */
}
''',
    'core/net/packet/in/Packet.java': r'''package core.net.packet.in;
import core.game.node.entity.player.Player;
public class Packet {
    public static class ItemAction extends Packet { public Player player; public int optIndex, itemId, slot, iface, child; }
    public static class NpcAction extends Packet { public Player player; public int optIndex, npcIndex; }
    public static class PlayerAction extends Packet { public Player player; public int optIndex, otherIndex; }
    public static class ItemExamine extends Packet { public Player player; public int id; }
    public static class SceneryExamine extends Packet { public Player player; public int id; }
    public static class NpcExamine extends Packet { public Player player; public int id; }
    public static class UseWithNpc extends Packet { public Player player; public int itemId, npcIndex, iface, slot; }
    public static class UseWithPlayer extends Packet { public Player player; public int itemId, otherIndex, iface, slot; }
    public static class UseWithItem extends Packet { public Player player; public int usedId, usedWithId, usedSlot, usedWithSlot, usedIface, usedWithIface, usedChild, usedWithChild; }
    public static class UseWithScenery extends Packet { public Player player; public int itemId, slot, sceneryId, x, y; }
    public static class UseWithGroundItem extends Packet { public Player player; public int usedId, withId, iface, child, slot, x, y; }
    public static class ComponentPlayerAction extends Packet { public Player player; public int otherIndex, iface, child; }
    public static class ComponentItemAction extends Packet { public Player player; public int iface, child, itemId, slot; }
    public static class ComponentNpcAction extends Packet { public Player player; public int iface, child, npcIndex; }
    public static class ComponentSceneryAction extends Packet { public Player player; public int iface, child, sceneryId, x, y; }
    public static class ComponentGroundItemAction extends Packet { public Player player; public int iface, child, slot, itemId, x, y; }
    public static class SlotSwitchMultiComponent extends Packet { public Player player; public int sourceIface, sourceChild, sourceSlot, destIface, destChild, destSlot; }
    public static class SlotSwitchSingleComponent extends Packet { public Player player; public int iface, child, sourceSlot, destSlot; public boolean isInsert; }
    public static class TrackFinished extends Packet { public Player player; public int trackId; }
}
''',
    'core/net/packet/PacketRepository.java': r'''package core.net.packet;
public class PacketRepository {
    public static void send(Class<?> type, Object context) {
        if (type == core.net.packet.out.ClearMinimapFlag.class) oracle.Support.emit("minimap");
        else throw new IllegalStateException(type.getName());
    }
}
''',
    'core/net/packet/out/ClearMinimapFlag.java': 'package core.net.packet.out;\npublic class ClearMinimapFlag {}\n',
    'core/net/packet/context/PlayerContext.java': r'''package core.net.packet.context;
public class PlayerContext { public final core.game.node.entity.player.Player player; public PlayerContext(core.game.node.entity.player.Player p) { player = p; } }
''',
    'core/ServerConstants.java': 'package core;\npublic class ServerConstants { public static int MAX_PLAYERS = 2000, MAX_NPCS = 32000; }\n',
    'core/game/node/Node.java': r'''package core.game.node;
import core.game.interaction.InteractPlugin;
import core.game.world.map.Location;
public class Node {
    // Field names differ from the getters' properties so that Kotlin reads go through the getters.
    public String key, nodeName, interactionKey, text, shown, child, drop;
    public int nodeId, nodeIndex, configFile = -1, definitionId;
    public boolean nodeActive = true;
    public Location nodeLocation, spawn;
    public int getId() { return nodeId; }
    public String getName() { return nodeName; }
    public int getIndex() { return nodeIndex; }
    public void setIndex(int index) { this.nodeIndex = index; }
    public boolean isActive() { return nodeActive; }
    public Location getLocation() { return nodeLocation; }
    public InteractPlugin getInteraction() {
        oracle.Support.emit("interaction", key);
        return interactionKey == null ? null : oracle.Support.interactions.get(interactionKey);
    }
    public core.game.node.scenery.Scenery asScenery() { return (core.game.node.scenery.Scenery) this; }
    public core.game.node.item.Item asItem() { return (core.game.node.item.Item) this; }
    public Location getFaceLocation(Location from) {
        oracle.Support.emit("faceLocation", key, oracle.Support.coord(from));
        return nodeLocation;
    }
    @Override public String toString() { return text; }
}
''',
    'core/game/node/item/Item.java': r'''package core.game.node.item;
import core.cache.def.impl.ItemDefinition;
public class Item extends core.game.node.Node {
    // Item.toString reads this field; it is protected so that Kotlin reads go through getAmount.
    protected int amount = 1;
    public void oracleAmount(int amount) { this.amount = amount; }
    public int getAmount() { return amount; }
    public int getSlot() { return nodeIndex; }
    public ItemDefinition getDefinition() { return oracle.Support.itemDef(definitionId); }
    public Item getDropItem() {
        oracle.Support.emit("dropItem", key);
        return (Item) oracle.Support.node(drop == null ? key : drop);
    }
    @Override
    /* ORIGINAL_ITEM_TO_STRING */
}
''',
    'core/game/node/item/GroundItem.java': r'''package core.game.node.item;
public class GroundItem extends Item { @Override public String toString() { return text; } }
''',
    'core/game/node/item/GroundItemManager.java': r'''package core.game.node.item;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
public class GroundItemManager {
    public static GroundItem get(int id, Location l, Player player) {
        oracle.Support.emit("ground", id, l.getX(), l.getY(), l.getZ());
        return (GroundItem) oracle.Support.node(oracle.Support.ground);
    }
    public static GroundItem create(Item item, Location l, Player player) {
        oracle.Support.emit("groundCreate", oracle.Support.key(item), oracle.Support.coord(l));
        return null;
    }
}
''',
    'core/game/node/entity/Entity.java': r'''package core.game.node.entity;
public class Entity extends core.game.node.Node {
    public core.game.node.entity.impl.Properties getProperties() { return new core.game.node.entity.impl.Properties(this); }
}
''',
    'core/game/node/entity/impl/Properties.java': r'''package core.game.node.entity.impl;
public class Properties {
    final core.game.node.entity.Entity owner;
    public Properties(core.game.node.entity.Entity owner) { this.owner = owner; }
    public core.game.world.map.Location getSpawnLocation() { return owner.spawn; }
    public Object getAutocastSpell() { return oracle.Support.flag("autocast") ? "spell" : null; }
    public void setAutocastSpell(Object spell) {
        oracle.Support.emit(spell == null ? "clearAutocast" : "setAutocast");
        oracle.Support.flags.put("autocast", spell != null);
    }
}
''',
    'core/game/node/entity/impl/PulseType.java': 'package core.game.node.entity.impl;\npublic enum PulseType { STANDARD, STRONG }\n',
    'core/game/node/entity/impl/PulseManager.java': r'''package core.game.node.entity.impl;
import core.game.system.task.Pulse;
public class PulseManager {
    public boolean clear(PulseType type) { oracle.Support.emit("clearPulses"); return true; }
    public void run(Pulse pulse, PulseType type) { oracle.Support.emit("run", pulse.number); }
    public Pulse runUnhandledAction(core.game.node.entity.player.Player player, PulseType type) { oracle.Support.emit("unhandled"); return null; }
}
''',
    'core/game/node/entity/npc/NPC.java': r'''package core.game.node.entity.npc;
import core.cache.def.impl.NPCDefinition;
import core.game.node.entity.player.Player;
public class NPC extends core.game.node.entity.Entity {
    public NPC getShownNPC(Player player) {
        oracle.Support.emit("shown", key);
        return (NPC) oracle.Support.node(shown == null ? key : shown);
    }
    public NPCDefinition getDefinition() { NPCDefinition d = new NPCDefinition(); d.setConfigFileId(configFile); return d; }
}
''',
    'core/game/node/scenery/Scenery.java': r'''package core.game.node.scenery;
import core.game.node.entity.player.Player;
public class Scenery extends core.game.node.Node {
    public Scenery getChild(Player player) {
        oracle.Support.emit("child", key);
        return (Scenery) oracle.Support.node(child);
    }
}
''',
    'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
import core.game.container.Container;
import core.game.container.impl.BankContainer;
import core.game.container.impl.EquipmentContainer;
import core.game.dialogue.DialogueInterpreter;
import core.game.interaction.ScriptProcessor;
import core.game.node.entity.impl.PulseManager;
import core.game.node.entity.lock.ActionLocks;
import core.game.node.entity.player.link.FamiliarManager;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.SpellBookManager;
import core.game.node.entity.player.link.music.MusicPlayer;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneMonitor;
import oracle.Support;
public class Player extends core.game.node.entity.Entity {
    final ActionLocks locks = new ActionLocks();
    final ScriptProcessor scripts = new ScriptProcessor();
    final PacketDispatch packetDispatch = new PacketDispatch();
    final InterfaceManager interfaceManager = new InterfaceManager();
    final ZoneMonitor zoneMonitor = new ZoneMonitor();
    final PulseManager pulseManager = new PulseManager();
    final DialogueInterpreter dialogueInterpreter = new DialogueInterpreter();
    final FamiliarManager familiarManager = new FamiliarManager();
    final SpellBookManager spellBookManager = new SpellBookManager();
    final MusicPlayer musicPlayer = new MusicPlayer();
    public ActionLocks getLocks() { return locks; }
    public ScriptProcessor getScripts() { return scripts; }
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public InterfaceManager getInterfaceManager() { return interfaceManager; }
    public ZoneMonitor getZoneMonitor() { return zoneMonitor; }
    public PulseManager getPulseManager() { return pulseManager; }
    public DialogueInterpreter getDialogueInterpreter() { return dialogueInterpreter; }
    public FamiliarManager getFamiliarManager() { return familiarManager; }
    public SpellBookManager getSpellBookManager() { return spellBookManager; }
    public MusicPlayer getMusicPlayer() { return musicPlayer; }
    public Container getInventory() { return Support.containers.get("inventory"); }
    public BankContainer getBank() { return (BankContainer) Support.containers.get("bank"); }
    public EquipmentContainer getEquipment() { return (EquipmentContainer) Support.containers.get("equipment"); }
    public void debug(String text) { Support.emit("debug", text); }
    public void sendMessage(String text) { Support.emit("message", text); }
    public void faceLocation(Location l) { Support.emit("face", Support.coord(l)); }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        Support.emit("attribute", key);
        return Support.attributes.containsKey(key) ? (T) Support.attributes.get(key) : fail;
    }
    @SuppressWarnings("unchecked")
    public <T> T getExtension(Class<T> type) {
        return Support.flag("noWeaponInterface") ? null : (T) new core.game.node.entity.combat.equipment.WeaponInterface();
    }
    public void dispatch(Object event) {
        if (event instanceof core.game.event.UseWithEvent) {
            core.game.event.UseWithEvent e = (core.game.event.UseWithEvent) event;
            Support.emit("dispatchUse", e.used, e.with);
        } else if (event instanceof core.game.event.ItemEquipEvent) {
            core.game.event.ItemEquipEvent e = (core.game.event.ItemEquipEvent) event;
            Support.emit("dispatchEquip", e.itemId, e.slotId);
        } else if (event instanceof core.game.event.ItemUnequipEvent) {
            core.game.event.ItemUnequipEvent e = (core.game.event.ItemUnequipEvent) event;
            Support.emit("dispatchUnequip", e.itemId, e.slotId);
        } else throw new IllegalStateException(String.valueOf(event));
    }
}
''',
    'core/game/node/entity/lock/ActionLocks.java': r'''package core.game.node.entity.lock;
public class ActionLocks {
    public boolean isInteractionLocked() { oracle.Support.emit("interactionLocked"); return oracle.Support.flag("interactionLocked"); }
    public boolean isMovementLocked() { oracle.Support.emit("movementLocked"); return oracle.Support.flag("movementLocked"); }
    public Lock getEquipmentLock() { return oracle.Support.flag("equipmentLock") ? new Lock() : null; }
}
''',
    'core/game/node/entity/lock/Lock.java': r'''package core.game.node.entity.lock;
public class Lock {
    public boolean isLocked() { oracle.Support.emit("equipmentLocked"); return oracle.Support.flag("equipmentLocked"); }
    public String getMessage() { return oracle.Support.text("lockMessage", null); }
}
''',
    'core/game/interaction/ScriptProcessor.java': 'package core.game.interaction;\npublic class ScriptProcessor { public void reset() { oracle.Support.emit("scripts"); } }\n',
    'core/game/node/entity/player/link/PacketDispatch.java': r'''package core.game.node.entity.player.link;
public class PacketDispatch {
    public void sendMessage(String text) { oracle.Support.emit("packetMessage", text); }
    public void sendString(String text, int iface, int child) { oracle.Support.emit("sendString", text, iface, child); }
}
''',
    'core/game/node/entity/player/link/InterfaceManager.java': r'''package core.game.node.entity.player.link;
public class InterfaceManager {
    public boolean close() { oracle.Support.emit("closeInterface"); return true; }
    public boolean hasChatbox() { oracle.Support.emit("hasChatbox"); return oracle.Support.flag("chatbox"); }
    public void closeChatbox() { oracle.Support.emit("closeChatbox"); oracle.Support.flags.put("chatbox", false); }
}
''',
    'core/game/world/map/zone/ZoneMonitor.java': r'''package core.game.world.map.zone;
import core.game.interaction.Option;
import core.game.node.Node;
import core.game.node.item.Item;
public class ZoneMonitor {
    public boolean interact(Node target, Option option) {
        oracle.Support.emit("zoneInteract", oracle.Support.key(target), oracle.Support.key(option));
        return oracle.Support.answerBool("zoneInteract", false);
    }
    public boolean useWith(Item used, Node with) {
        oracle.Support.emit("zoneUseWith", oracle.Support.key(used), oracle.Support.key(with));
        return oracle.Support.answerBool("zoneUseWith", false);
    }
}
''',
    'core/game/dialogue/DialogueInterpreter.java': r'''package core.game.dialogue;
public class DialogueInterpreter {
    public boolean close() { oracle.Support.emit("dialogueClose"); return true; }
    public void sendDestroyItem(int id, String name) { oracle.Support.emit("destroyDialogue", id, name); }
}
''',
    'core/game/node/entity/player/link/FamiliarManager.java': r'''package core.game.node.entity.player.link;
import content.global.skill.summoning.familiar.FamiliarSpecial;
import core.game.node.item.Item;
public class FamiliarManager {
    public boolean hasFamiliar() { oracle.Support.emit("hasFamiliar"); return oracle.Support.flag("familiar"); }
    public Familiar getFamiliar() { return new Familiar(); }
    public void summon(Item item, boolean pet, boolean deposit) { oracle.Support.emit("summon", oracle.Support.key(item), pet, deposit); }
    public static class Familiar {
        public boolean executeSpecialMove(FamiliarSpecial s) {
            oracle.Support.emit("familiarSpecial", oracle.Support.key(s.node), s.interfaceID, s.component, oracle.Support.key(s.item));
            return true;
        }
    }
}
''',
    'content/global/skill/summoning/familiar/FamiliarSpecial.java': r'''package content.global.skill.summoning.familiar;
import core.game.node.Node;
import core.game.node.item.Item;
public class FamiliarSpecial {
    public final Node node; public final int interfaceID, component; public final Item item;
    public FamiliarSpecial(Node node, int interfaceID, int component, Item item) { this.node = node; this.interfaceID = interfaceID; this.component = component; this.item = item; }
}
''',
    'core/game/node/entity/player/link/SpellBookManager.java': r'''package core.game.node.entity.player.link;
public class SpellBookManager {
    public int getSpellBook() { oracle.Support.emit("spellBook"); return oracle.Support.intValue("spellBook", 192); }
    public static enum SpellBook {
        /* ORIGINAL_SPELL_BOOKS */;
        private final int interfaceId;
        SpellBook(int interfaceId) { this.interfaceId = interfaceId; }
        /* ORIGINAL_FOR_INTERFACE */
    }
}
''',
    'core/game/node/entity/player/link/music/MusicPlayer.java': r'''package core.game.node.entity.player.link.music;
public class MusicPlayer {
    public boolean isLooping() { return oracle.Support.flag("looping"); }
    public boolean isPlaying() { return oracle.Support.flag("playing"); }
    public void setPlaying(boolean playing) { oracle.Support.emit("setPlaying", playing); oracle.Support.flags.put("playing", playing); }
    public void replay() { oracle.Support.emit("replay"); }
}
''',
    'core/game/node/entity/combat/equipment/WeaponInterface.java': r'''package core.game.node.entity.combat.equipment;
public class WeaponInterface {
    public void selectAutoSpell(int button, boolean adjust, boolean forget) { oracle.Support.emit("selectAutoSpell", button, adjust, forget); }
    public void openAutocastSelect() { oracle.Support.emit("openAutocastSelect"); }
}
''',
    'core/game/node/entity/combat/spell/MagicSpell.java': r'''package core.game.node.entity.combat.spell;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.SpellBookManager.SpellBook;
public class MagicSpell {
    public static boolean castSpell(final Player p, SpellBook book, int spellId, Node target) {
        oracle.Support.emit("castSpell", book == null ? null : book.name(), spellId, oracle.Support.key(target));
        return true;
    }
}
''',
    'content/global/skill/magic/SpellListener.java': r'''package content.global.skill.magic;
public class SpellListener { public static final int NPC = -1, OBJECT = -2, ITEM = -3, PLAYER = -4, NONE = -5, GROUND_ITEM = -6; }
''',
    'content/global/skill/magic/SpellListeners.java': r'''package content.global.skill.magic;
import core.game.node.Node;
import core.game.node.entity.player.Player;
public class SpellListeners {
    public static void run(int button, int type, String book, Player player, Node node) {
        oracle.Support.emit("spellListeners", button, type, book, oracle.Support.key(node));
    }
}
''',
    'content/global/skill/magic/SpellUtils.kt': r'''package content.global.skill.magic

import core.game.node.entity.player.Player

object SpellUtils {
    /* ORIGINAL_SPELL_UTILS */
}
''',
    'core/game/world/map/Location.java': r'''package core.game.world.map;
public class Location {
    final int x, y, z;
    Location(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public int getX() { return x; }
    public int getY() { return y; }
    public int getZ() { return z; }
    @Override public String toString() { return "[" + x + ", " + y + ", " + z + "]"; }
}
''',
    'core/game/world/map/RegionManager.java': r'''package core.game.world.map;
import core.game.node.scenery.Scenery;
public class RegionManager {
    public static Scenery getObject(int x, int y, int z, int id, int shape) {
        oracle.Support.emit("object", x, y, z, id, shape);
        return (Scenery) oracle.Support.node(oracle.Support.object);
    }
    public static Scenery getObject(int x, int y, int z) {
        oracle.Support.emit("objectAt", x, y, z);
        return (Scenery) oracle.Support.node(oracle.Support.objectAt);
    }
}
''',
    'core/game/world/repository/NodeList.java': r'''package core.game.world.repository;
public class NodeList<E> {
    final String name;
    NodeList(String name) { this.name = name; }
    @SuppressWarnings("unchecked")
    public E get(int index) {
        oracle.Support.emit(name, index);
        boolean players = name.equals("player");
        int length = players ? oracle.Support.playerLength : oracle.Support.npcLength;
        if (index <= 0 || index >= length) throw new IndexOutOfBoundsException();
        return (E) oracle.Support.node((players ? oracle.Support.repoPlayers : oracle.Support.repoNpcs).get(index));
    }
}
''',
    'core/game/world/repository/Repository.java': r'''package core.game.world.repository;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
public class Repository {
    public static NodeList<Player> players = new NodeList<>("player");
    public static NodeList<NPC> npcs = new NodeList<>("npc");
}
''',
    'core/game/world/GameWorld.kt': r'''package core.game.world

object GameWorld {
    @JvmStatic val ticks: Int get() = oracle.Support.ticks()
    @JvmStatic val Pulser = PulseRunner()
}

class PulseRunner {
    fun submit(pulse: core.game.system.task.Pulse) { oracle.Support.submit(pulse) }
}
''',
    'core/game/system/task/Pulse.java': r'''package core.game.system.task;
import core.game.node.Node;
public abstract class Pulse {
    public final int number;
    public Pulse() { number = oracle.Support.pulses.size(); oracle.Support.pulses.add(this); }
    public Pulse(int delay, Node... checks) { this(); }
    public abstract boolean pulse();
}
''',
    'core/game/interaction/MovementPulse.java': r'''package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.Entity;
public abstract class MovementPulse extends core.game.system.task.Pulse {
    public MovementPulse(Entity mover, Node destination) { super(); oracle.Support.emit("movement", oracle.Support.key(destination), null, number); }
    public MovementPulse(Entity mover, Node destination, UseWithHandler handler) {
        super();
        oracle.Support.emit("movement", oracle.Support.key(destination), handler == null ? null : ((oracle.Support.UseRec) handler).ref, number);
    }
}
''',
    'core/game/interaction/InteractPlugin.java': r'''package core.game.interaction;

import core.game.container.Container;
import core.game.node.Node;
import core.game.node.entity.impl.PulseType;
import core.game.node.entity.player.Player;
import core.game.system.task.Pulse;
import core.game.world.GameWorld;
import core.tools.Log;

import static core.api.ContentAPIKt.log;

public class InteractPlugin {
    private Option[] options = new Option[8];
    private final Node node;
    public String key;
    public InteractPlugin(Node node) { this.node = node; }
    public void setOptions(Option[] options) { this.options = options; }
    public Option get(int index) { oracle.Support.emit("option", key, index); return options[index]; }
    public void handle(final Player player, final Option option) { oracle.Support.emit("handle", key, oracle.Support.key(option)); }
    public static void handleInvalidInteraction(final Player player, final Node node, final Option option) {
        oracle.Support.emit("invalid", oracle.Support.key(node), option.key);
    }

    /* ORIGINAL_HANDLE_ITEM_OPTION */
}
''',
    'core/game/interaction/Option.java': r'''package core.game.interaction;
public class Option {
    public static final Option NULL = new Option("null", -1);
    static { NULL.key = "NULL"; }
    public String key;
    private final String name;
    private final int index;
    private OptionHandler handler;
    public Option(String name, int index) { this.name = name; this.index = index; }
    public String getName() { return name; }
    public int getIndex() { return index; }
    public OptionHandler getHandler() { return handler; }
    public Option setHandler(OptionHandler handler) { this.handler = handler; return this; }
}
''',
    'core/game/interaction/OptionHandler.java': r'''package core.game.interaction;
public abstract class OptionHandler {
    public abstract boolean handle(core.game.node.entity.player.Player player, core.game.node.Node node, String option);
}
''',
    'core/game/interaction/IntType.java': 'package core.game.interaction;\npublic enum IntType { ITEM, SCENERY, NPC, GROUNDITEM, PLAYER }\n',
    'core/game/interaction/QueueStrength.java': 'package core.game.interaction;\npublic enum QueueStrength { WEAK, NORMAL, STRONG, SOFT }\n',
    'core/game/interaction/InteractionListeners.java': r'''package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import oracle.Support;
public class InteractionListeners {
    // The Kotlin originals check their non-null parameters on entry.
    public static boolean run(int id, IntType type, String option, Player player, Node node) {
        if (type == null || option == null || player == null || node == null) throw new NullPointerException();
        Support.emit("listeners", id, type.name(), option, Support.key(node));
        return Support.answerBool("listeners", false);
    }
    public static boolean run(Node used, Node with, IntType type, Player player) {
        if (used == null || with == null || type == null || player == null) throw new NullPointerException();
        Support.emit("listenersUse", Support.key(used), Support.key(with), type.name());
        return Support.answerBool("listenersUse", false);
    }
    public static boolean run(int id, Player player, Node node, boolean isEquip) {
        if (player == null || node == null) throw new NullPointerException();
        Support.emit("listenersEquip", id, Support.key(node), isEquip);
        return Support.answerBool("listenersEquip:" + isEquip, true);
    }
}
''',
    'core/game/interaction/PluginInteractionManager.java': r'''package core.game.interaction;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import oracle.Support;
public class PluginInteractionManager {
    public static boolean handle(Player player, NPC npc, Option option) {
        Support.emit("pluginNpc", Support.key(npc), Support.key(option));
        return Support.answerBool("pluginNpc", false);
    }
    public static boolean handle(Player player, NodeUsageEvent event) {
        Support.emit("pluginUse", Support.key(event.getUsed()), Support.key(event.getUsedWith()));
        return Support.answerBool("pluginUse", false);
    }
}
''',
    'core/game/interaction/InteractionListener.kt': r'''package core.game.interaction

import core.game.node.Node
import core.game.node.entity.player.Player

interface InteractionListener {
    fun defineListeners()
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(type.name, arrayOf(*option), handler)
    }
}
''',
    'core/game/container/Container.java': r'''package core.game.container;
import core.game.node.item.Item;
import oracle.Support;
public class Container {
    public final String name;
    public Item[] items;
    public Container(String name, int capacity) { this.name = name; items = new Item[capacity]; }
    public void resize(int capacity) { items = java.util.Arrays.copyOf(items, capacity); }
    public int capacity() { return items.length; }
    public Item[] toArray() { return items; }
    public Item get(int slot) {
        Support.emit("get", name, slot);
        if (slot < 0 || slot >= items.length) return null;
        return items[slot];
    }
    public Item replace(Item item, int slot) { return replace(item, slot, true); }
    public Item replace(Item item, int slot, boolean fire) {
        Support.emit("replace", name, Support.key(item), slot, fire);
        Item old = items[slot];
        items[slot] = item;
        return old;
    }
    public void insert(int from, int to) { insert(from, to, true); }
    public void insert(int from, int to, boolean update) {
        Support.emit("insert", name, from, to, update);
        Item temp = items[from];
        if (to > from) for (int i = from; i < to; i++) items[i] = items[i + 1];
        else if (from > to) for (int i = from; i > to; i--) items[i] = items[i - 1];
        items[to] = temp;
    }
    public boolean remove(Item item) {
        Support.emit("remove", name, Support.key(item));
        boolean removed = Support.answerBool("remove:" + name, true);
        if (removed) for (int i = 0; i < items.length; i++) if (items[i] == item) { items[i] = null; break; }
        return removed;
    }
    public boolean add(Item... add) {
        Support.emit("add", name, Support.key(add[0]));
        boolean added = Support.answerBool("add:" + name, true);
        if (added) for (int i = 0; i < items.length; i++) if (items[i] == null) { items[i] = add[0]; break; }
        return added;
    }
    public int freeSlot() {
        Support.emit("freeSlot", name);
        for (int i = 0; i < items.length; i++) if (items[i] == null) return i;
        return -1;
    }
    public void refresh() { Support.emit("refresh", name); }
    public int getMaximumAdd(Item item) { Support.emit("maximumAdd", Support.key(item)); return Support.answerInt("maximumAdd", 28); }
}
''',
    'core/game/container/impl/BankContainer.java': r'''package core.game.container.impl;
import oracle.Support;
public class BankContainer extends core.game.container.Container {
    public int[] tabStartSlot = new int[11];
    public BankContainer(String name, int capacity) { super(name, capacity); }
    public boolean isInsertItems() { return Support.flag("insertItems"); }
    public int getItemsInTab(int tab) { Support.emit("itemsInTab", tab); return Support.answerInt("itemsInTab", 0); }
    public int getTabByItemSlot(int slot) { Support.emit("tabByItemSlot", slot); return Support.answerInt("tabByItemSlot", 0); }
    public void increaseTabStartSlots(int tab) { Support.emit("increaseTabStartSlots", tab); }
    public void decreaseTabStartSlots(int tab) { Support.emit("decreaseTabStartSlots", tab); }
    public void setTabConfigurations() { Support.emit("setTabConfigurations"); }

    /* ORIGINAL_GET_ARRAY_INDEX */
}
''',
    'core/game/container/impl/EquipmentContainer.java': r'''package core.game.container.impl;
import core.game.node.item.Item;
import oracle.Support;
public class EquipmentContainer extends core.game.container.Container {
    /* ORIGINAL_SLOT_WEAPON */
    public EquipmentContainer(String name, int capacity) { super(name, capacity); }
    public boolean add(Item item, int slot, boolean fire, boolean fromInventory) {
        Support.emit("equipAdd", Support.key(item), slot, fire, fromInventory);
        return Support.answerBool("equipAdd", true);
    }
}
''',
    'core/cache/def/Definition.java': r'''package core.cache.def;
import core.tools.StringUtils;
import java.util.HashMap;
import java.util.Map;
public class Definition {
    protected int id;
    protected String name = "null";
    protected String examine;
    protected final Map<String, Object> handlers = new HashMap<String, Object>();
    public Map<String, Object> getHandlers() { return handlers; }
    public String getName() { return name; }
    public void setName(String name) { this.name = name; }
    public void setExamine(String examine) { this.examine = examine; }
    public String examineField() { return examine; }

    @SuppressWarnings("unchecked")
    /* ORIGINAL_GET_CONFIGURATION */

    /* ORIGINAL_DEFINITION_EXAMINE */
}
''',
    'core/cache/def/impl/ItemDefinition.java': r'''package core.cache.def.impl;
public class ItemDefinition extends core.cache.def.Definition {
    private int value = 1, interfaceModelId;
    private boolean unnoted = true;
    public int getValue() { return value; }
    public void setValue(int value) { this.value = value; }
    public int getInterfaceModelId() { return interfaceModelId; }
    public void setInterfaceModelId(int id) { this.interfaceModelId = id; }
    public boolean isUnnoted() { return unnoted; }
    public void setUnnoted(boolean unnoted) { this.unnoted = unnoted; }
    public static ItemDefinition forId(int id) {
        oracle.Support.emit("itemDefinition", id);
        return oracle.Support.missingDefs.contains("item:" + id) ? null : oracle.Support.itemDef(id);
    }

    @Override
    /* ORIGINAL_ITEM_EXAMINE */
}
''',
    'core/cache/def/impl/SceneryDefinition.java': r'''package core.cache.def.impl;
public class SceneryDefinition extends core.cache.def.Definition {
    private int configFileId = -1;
    public void setConfigFileId(int id) { configFileId = id; }
    public static SceneryDefinition forId(int id) {
        oracle.Support.emit("sceneryDefinition", id);
        return oracle.Support.missingDefs.contains("scenery:" + id) ? null : oracle.Support.sceneryDef(id);
    }
    public int[] getChildrenIds() { return null; }

    /* ORIGINAL_GET_CONFIG_FILE */
}
''',
    'core/cache/def/impl/VarbitDefinition.java': r'''package core.cache.def.impl;
public class VarbitDefinition {
    final int id;
    VarbitDefinition(int id) { this.id = id; }
    public int getId() { return id; }
    public static VarbitDefinition forObjectID(int id) {
        oracle.Support.emit("varbit", id);
        Object found = oracle.Support.answer("varbit", id);
        return found == null ? null : new VarbitDefinition(((Number) found).intValue());
    }
}
''',
    'core/cache/def/impl/NPCDefinition.java': r'''package core.cache.def.impl;
import core.game.system.config.NPCConfigParser;
import core.tools.StringUtils;
public class NPCDefinition extends core.cache.def.Definition {
    private int configFileId = -1;
    public int getConfigFileId() { return configFileId; }
    public void setConfigFileId(int id) { configFileId = id; }
    public static NPCDefinition forId(int id) {
        oracle.Support.emit("npcDefinition", id);
        return oracle.Support.missingDefs.contains("npc:" + id) ? null : oracle.Support.npcDef(id);
    }

    /* ORIGINAL_NPC_EXAMINE */
}
''',
    'core/tools/StringUtils.java': r'''package core.tools;
public class StringUtils {
    /* ORIGINAL_IS_PLUS_N */
}
''',
    'core/tools/Log.java': 'package core.tools;\npublic enum Log { FINE, INFO, WARN, ERR }\n',
    'core/tools/SystemLogger.java': 'package core.tools;\npublic class SystemLogger {}\n',
    'core/game/system/config/NPCConfigParser.java': r'''package core.game.system.config;
public class NPCConfigParser {
    /* ORIGINAL_NPC_CONFIG_KEYS */
}
''',
    'core/game/system/config/ItemConfigParser.java': r'''package core.game.system.config;
public class ItemConfigParser {
    /* ORIGINAL_ITEM_CONFIG_KEYS */
}
''',
    'core/plugin/Plugin.java': r'''package core.plugin;
public interface Plugin<T> {
    Plugin<T> newInstance(T arg) throws Throwable;
    Object fireEvent(String identifier, Object... args);
}
''',
    'core/game/event/UseWithEvent.java': 'package core.game.event;\npublic class UseWithEvent { public final int used, with; public UseWithEvent(int used, int with) { this.used = used; this.with = with; } }\n',
    'core/game/event/ItemEquipEvent.java': 'package core.game.event;\npublic class ItemEquipEvent { public final int itemId, slotId; public ItemEquipEvent(int itemId, int slotId) { this.itemId = itemId; this.slotId = slotId; } }\n',
    'core/game/event/ItemUnequipEvent.java': 'package core.game.event;\npublic class ItemUnequipEvent { public final int itemId, slotId; public ItemUnequipEvent(int itemId, int slotId) { this.itemId = itemId; this.slotId = slotId; } }\n',
    'core/game/node/entity/player/link/audio/Audio.java': 'package core.game.node.entity.player.link.audio;\npublic class Audio {}\n',
    'content/global/handlers/item/equipment/brawling_gloves/BrawlingGlovesManager.java': r'''package content.global.handlers.item.equipment.brawling_gloves;
public class BrawlingGlovesManager {
    public static BrawlingGlovesManager getInstance(core.game.node.entity.player.Player player) { return new BrawlingGlovesManager(); }
    public void registerGlove(int id) { oracle.Support.emit("registerGlove", id); }
}
''',
    'content/global/skill/slayer/SlayerEquipmentFlags.java': r'''package content.global.skill.slayer;
public class SlayerEquipmentFlags {
    public static boolean isSlayerEq(int id) { oracle.Support.emit("isSlayerEq", id); return oracle.Support.answerBool("isSlayerEq", false); }
    public static void updateFlags(core.game.node.entity.player.Player player) { oracle.Support.emit("updateSlayerFlags"); }
}
''',
    'core/game/node/entity/combat/graves/GraveController.java': r'''package core.game.node.entity.combat.graves;
public class GraveController {
    public static boolean hasGraveAt(core.game.world.map.Location l) { oracle.Support.emit("grave", oracle.Support.coord(l)); return oracle.Support.answerBool("grave", false); }
}
''',
    'core/game/node/entity/player/info/login/PlayerParser.java': r'''package core.game.node.entity.player.info.login;
public class PlayerParser { public static void save(core.game.node.entity.player.Player player) { oracle.Support.emit("save"); } }
''',
    'content/global/skill/summoning/pet/Pets.java': r'''package content.global.skill.summoning.pet;
public class Pets { public static Object forId(int id) { oracle.Support.emit("pet", id); return oracle.Support.answerBool("pet", false) ? "pet" : null; } }
''',
    'core/game/node/entity/player/info/LogType.java': 'package core.game.node.entity.player.info;\npublic enum LogType { DUPE_ALERT }\n',
    'core/game/node/entity/player/info/PlayerMonitor.java': r'''package core.game.node.entity.player.info;
public class PlayerMonitor {
    public static void log(core.game.node.entity.player.Player player, LogType type, String text) { oracle.Support.emit("monitor", type.name(), text); }
}
''',
    'core/api/ContentAPI.kt': r'''package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.interaction.QueueStrength
import core.game.world.map.Location
import core.tools.Log

fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: Location? = null, radius: Int = 8) {
    oracle.Support.emit("audio", id)
}
fun getUsedOption(player: Player): String {
    oracle.Support.emit("usedOption")
    return oracle.Support.text("usedOption", "INVALID")
}
fun sendMessage(player: Player, message: String) { oracle.Support.emit("sendMessage", message) }
fun closeAllInterfaces(player: Player) { oracle.Support.emit("closeAll") }
fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    oracle.Support.queue(strength.name, script)
}
fun stopExecuting(entity: Entity): Boolean {
    oracle.Support.emit("stopExecuting")
    return oracle.Support.answerBool("stopExecuting", true)
}
fun <T> setAttribute(entity: Entity, attribute: String, value: T) { oracle.Support.emit("setAttribute", attribute, value) }
fun getWorldTicks(): Int = oracle.Support.ticks()
fun addDialogueAction(player: Player, action: core.game.dialogue.DialogueAction) { oracle.Support.addAction(action) }
fun <T> removeItem(player: Player, item: T): Boolean {
    oracle.Support.emit("removeItem", oracle.Support.key(item))
    return oracle.Support.answerBool("removeItem", true)
}
fun <T> getAttribute(entity: Entity, attribute: String, default: T): T = (entity as Player).getAttribute(attribute, default)
fun log(origin: Class<*>, type: Log, message: String) { oracle.Support.emit("log", origin.name, type.name, message) }
''',
}
