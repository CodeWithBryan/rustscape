"""Recording adapters for the node classes ContentAPI reaches (Node, Entity, Player, NPC, Item, Scenery and what
hangs off them). Field reads of case state are not recorded; every call that asks or changes the world is recorded
through oracle.R with the original's class and member name."""

ADAPTERS = {
'core/game/node/Node.java': r'''package core.game.node;
import core.game.world.map.Location;
public class Node {
    public String key;
    protected String name;
    protected Location location;
    protected int index;
    protected int size = 1;
    protected boolean active = true;
    public core.game.interaction.DestinationFlag destinationFlag;
    public int nodeId;
    public Node() {}
    public Node(String name, Location location) { this.name = name; this.location = location; }
    public core.game.node.entity.npc.NPC asNpc() { return (core.game.node.entity.npc.NPC) this; }
    public core.game.node.entity.player.Player asPlayer() { return (core.game.node.entity.player.Player) this; }
    public int getId() { return nodeId; }
    public String getName() { return name; }
    public Location getLocation() { return location; }
    public int size() { return size; }
    public boolean isActive() { return active; }
    public void setActive(boolean active) { oracle.R.v(getClass().getSimpleName() + ".setActive", this, active); this.active = active; }
    public void oracleState(String name, Location location, int index, int size) { this.name = name; this.location = location; this.index = index; this.size = size; }
}
''',
'core/game/interaction/DestinationFlag.java': r'''package core.game.interaction;
public class DestinationFlag {
    public static final DestinationFlag LOCATION = new DestinationFlag();
    public static final DestinationFlag ITEM = new DestinationFlag();
}
''',
'core/game/node/entity/Entity.java': r'''package core.game.node.entity;
import core.game.node.Node;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import oracle.R;
public class Entity extends Node {
    public int[] o_clocks = new int[8];
    public final core.game.interaction.ScriptProcessor o_scripts = new core.game.interaction.ScriptProcessor(this);
    public final core.game.system.timer.TimerManager o_timers = new core.game.system.timer.TimerManager(this);
    public final core.game.node.entity.skill.Skills o_skills = new core.game.node.entity.skill.Skills(this);
    public final core.game.node.entity.impl.Animator o_animator = new core.game.node.entity.impl.Animator(this);
    public final core.game.node.entity.combat.ImpactHandler o_impactHandler = new core.game.node.entity.combat.ImpactHandler(this);
    public final core.game.node.entity.lock.ActionLocks o_locks = new core.game.node.entity.lock.ActionLocks(this);
    public final core.game.node.entity.impl.WalkingQueue o_walkingQueue = new core.game.node.entity.impl.WalkingQueue(this);
    public final core.game.node.entity.impl.PulseManager o_pulseManager = new core.game.node.entity.impl.PulseManager(this);
    public final core.game.node.entity.impl.Properties o_properties = new core.game.node.entity.impl.Properties(this);
    public final core.game.node.entity.player.link.TeleportManager o_teleporter = new core.game.node.entity.player.link.TeleportManager(this);
    public final core.game.world.update.UpdateMasks o_updateMasks = new core.game.world.update.UpdateMasks(this);
    public Entity() {}
    public Entity(String name, Location location) { super(name, location); }
    public core.game.interaction.ScriptProcessor getScripts() { return o_scripts; }
    public core.game.system.timer.TimerManager getTimers() { return o_timers; }
    public core.game.node.entity.skill.Skills getSkills() { return o_skills; }
    public core.game.node.entity.impl.Animator getAnimator() { return o_animator; }
    public core.game.node.entity.combat.ImpactHandler getImpactHandler() { return o_impactHandler; }
    public core.game.node.entity.lock.ActionLocks getLocks() { return o_locks; }
    public core.game.node.entity.impl.WalkingQueue getWalkingQueue() { return o_walkingQueue; }
    public core.game.node.entity.impl.PulseManager getPulseManager() { return o_pulseManager; }
    public core.game.node.entity.impl.Properties getProperties() { return o_properties; }
    public core.game.node.entity.player.link.TeleportManager getTeleporter() { return o_teleporter; }
    public core.game.world.update.UpdateMasks getUpdateMasks() { return o_updateMasks; }
    public int[] getClocks() { return o_clocks; }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fallback) {
        Object a = R.ev("Entity.getAttribute", this, key, fallback);
        return a == null ? fallback : (T) R.typed(a);
    }
    public void setAttribute(String key, Object value) { R.v("Entity.setAttribute", this, key, value); }
    public void removeAttribute(String key) { R.v("Entity.removeAttribute", this, key); }
    public void faceLocation(Location location) { R.v("Entity.faceLocation", this, location); }
    public boolean face(Entity entity) { return R.b("Entity.face", this, entity); }
    public boolean faceTemporary(Entity entity, int ticks) { return R.b("Entity.faceTemporary", this, entity, ticks); }
    public boolean animate(Animation animation) { return R.b("Entity.animate", this, animation); }
    public boolean graphics(Graphics graphics) { return R.b("Entity.graphics", this, graphics); }
    public boolean visualize(Animation animation, Graphics graphics) { return R.b("Entity.visualize", this, animation, graphics); }
    public boolean sendChat(String text) { return R.b("Entity.sendChat", this, text); }
    public void lock(int time) { R.v("Entity.lock", this, time); }
    public void unlock() { R.v("Entity.unlock", this); }
    public boolean isPoisonImmune() { return R.b("Entity.isPoisonImmune", this); }
    public int getDragonfireProtection(boolean fire) { return R.i("Entity.getDragonfireProtection", this, fire); }
}
''',
'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
import core.game.container.Container;
import core.game.container.impl.BankContainer;
import core.game.container.impl.EquipmentContainer;
import java.util.HashMap;
import kotlin.Unit;
import kotlin.jvm.functions.Function1;
import oracle.R;
public class Player extends core.game.node.entity.Entity {
    public HashMap<Integer, Integer> varpMap = new HashMap<>();
    public HashMap<Integer, Boolean> saveVarp = new HashMap<>();
    public HashMap<String, Function1<Player, Unit>> logoutListeners = new HashMap<>();
    public boolean useSecondaryBank = false;
    public boolean o_male = true;
    public String o_username = "";
    public final Container o_inventory = new Container(this, "inventory");
    public final EquipmentContainer o_equipment = new EquipmentContainer(this, "equipment");
    public final BankContainer o_bank = new BankContainer(this, "bankPrimary");
    public final BankContainer o_bankSecondary = new BankContainer(this, "bankSecondary");
    public final Container POHStorageProxyContainer = new Container(this, "POHStorageProxyContainer");
    public final content.global.skill.summoning.familiar.FamiliarManager o_familiarManager = new content.global.skill.summoning.familiar.FamiliarManager(this);
    public final content.global.skill.construction.decoration.pohstorage.StorageState o_storageState = new content.global.skill.construction.decoration.pohstorage.StorageState(this);
    public final core.game.node.entity.player.link.IronmanManager o_ironmanManager = new core.game.node.entity.player.link.IronmanManager(this);
    public final core.game.node.entity.player.link.appearance.Appearance o_appearance = new core.game.node.entity.player.link.appearance.Appearance(this);
    public final core.game.node.entity.player.link.PacketDispatch o_packetDispatch = new core.game.node.entity.player.link.PacketDispatch(this);
    public final core.game.node.entity.player.link.InterfaceManager o_interfaceManager = new core.game.node.entity.player.link.InterfaceManager(this);
    public final core.game.dialogue.DialogueInterpreter o_dialogueInterpreter = new core.game.dialogue.DialogueInterpreter(this);
    public final core.game.node.entity.player.link.quest.QuestRepository o_questRepository = new core.game.node.entity.player.link.quest.QuestRepository(this);
    public final content.global.skill.construction.HouseManager o_houseManager = new content.global.skill.construction.HouseManager(this);
    public final core.game.node.entity.player.link.BankPinManager o_bankPinManager = new core.game.node.entity.player.link.BankPinManager(this);
    public final core.game.node.entity.player.link.HintIconManager o_hintIconManager = new core.game.node.entity.player.link.HintIconManager(this);
    public final core.game.node.entity.player.info.PlayerDetails o_details = new core.game.node.entity.player.info.PlayerDetails(this);
    public final core.game.node.entity.player.link.diary.AchievementDiaryManager o_achievementDiaryManager = new core.game.node.entity.player.link.diary.AchievementDiaryManager(this);
    public final core.game.node.entity.player.link.prayer.Prayer o_prayer = new core.game.node.entity.player.link.prayer.Prayer(this);
    public Player() {}
    public Container getInventory() { return o_inventory; }
    public EquipmentContainer getEquipment() { return o_equipment; }
    public BankContainer getBank() { return useSecondaryBank ? o_bankSecondary : o_bank; }
    public BankContainer getBankPrimary() { return o_bank; }
    public BankContainer getBankSecondary() { return o_bankSecondary; }
    public content.global.skill.summoning.familiar.FamiliarManager getFamiliarManager() { return o_familiarManager; }
    public content.global.skill.construction.decoration.pohstorage.StorageState getPOHStorageState() { return o_storageState; }
    public core.game.node.entity.player.link.IronmanManager getIronmanManager() { return o_ironmanManager; }
    public core.game.node.entity.player.link.appearance.Appearance getAppearance() { return o_appearance; }
    public core.game.node.entity.player.link.PacketDispatch getPacketDispatch() { return o_packetDispatch; }
    public core.game.node.entity.player.link.InterfaceManager getInterfaceManager() { return o_interfaceManager; }
    public core.game.dialogue.DialogueInterpreter getDialogueInterpreter() { return o_dialogueInterpreter; }
    public core.game.node.entity.player.link.quest.QuestRepository getQuestRepository() { return o_questRepository; }
    public content.global.skill.construction.HouseManager getHouseManager() { return o_houseManager; }
    public core.game.node.entity.player.link.BankPinManager getBankPinManager() { return o_bankPinManager; }
    public core.game.node.entity.player.link.HintIconManager getHintIconManager() { return o_hintIconManager; }
    public core.game.node.entity.player.info.PlayerDetails getDetails() { return o_details; }
    public core.game.node.entity.player.link.diary.AchievementDiaryManager getAchievementDiaryManager() { return o_achievementDiaryManager; }
    public core.game.node.entity.player.link.prayer.Prayer getPrayer() { return o_prayer; }
    public boolean isMale() { return o_male; }
    public String getUsername() { return o_username; }
    public void sendMessages(String... messages) { R.v("Player.sendMessages", this, messages); }
    public void sendMessage(String message) { R.v("Player.sendMessage", this, message); }
}
''',
'core/game/node/entity/npc/NPC.java': r'''package core.game.node.entity.npc;
import oracle.R;
public class NPC extends core.game.node.entity.Entity {
    public boolean o_invisible;
    public NPC() {}
    public boolean isInvisible() { return o_invisible; }
    public void setInvisible(boolean invisible) { R.v("NPC.setInvisible", this, invisible); this.o_invisible = invisible; }
    public void clear() { R.v("NPC.clear", this); }
    public NPC transform(int id) { R.v("NPC.transform", this, id); return this; }
    public NPC transformWithHpCarryover(int id) { R.v("NPC.transformWithHpCarryover", this, id); return this; }
    public void reTransform() { R.v("NPC.reTransform", this); }
    public core.cache.def.impl.NPCDefinition getDefinition() { return core.cache.def.impl.NPCDefinition.forId(getId()); }
}
''',
'core/game/node/item/Item.java': r'''package core.game.node.item;
import core.cache.def.impl.ItemDefinition;
public class Item extends core.game.node.Node {
    protected int idHash;
    protected int amount;
    protected ItemDefinition definition;
    public Item() { super("null", null); this.idHash = -1 << 16 | 1000; }
    public Item(int id) { this(id, 1, 1000); }
    public Item(int id, int amount) { this(id, amount, 1000); }
    public Item(int id, int amount, int charge) {
        super(oracle.R.newItem(id, amount, charge), null);
        oracle.R.fresh(this, "item");
        this.key = oracle.R.label(this);
        super.index = -1;
        this.idHash = id << 16 | charge;
        this.amount = amount;
        this.definition = ItemDefinition.forId(id);
    }
    public void oracleItem(int id, int amount, int charge, int slot) {
        this.idHash = id << 16 | charge; this.amount = amount; this.index = slot; this.definition = ItemDefinition.forId(id);
        this.name = definition == null ? null : definition.getName();
    }
    public int getId() { return idHash >> 16 & 0xFFFF; }
    public int getAmount() { return amount; }
    public int getCharge() { return idHash & 0xFFFF; }
    public void setCharge(int charge) { oracle.R.v("Item.setCharge", this, charge); this.idHash = (idHash >> 16 & 0xFFFF) << 16 | charge; }
    public ItemDefinition getDefinition() { return definition; }
    public int getSlot() { return index; }
    /* ORIGINAL_ITEM_TO_STRING */
    public int getNoteChange() {
        int noteId = definition.getNoteId();
        if (noteId > -1) {
            return noteId;
        }
        return getId();
    }
}
''',
'core/game/node/item/GroundItem.java': r'''package core.game.node.item;
public class GroundItem extends Item {
    public GroundItem() {}
}
''',
'core/game/node/item/GroundItemManager.java': r'''package core.game.node.item;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import oracle.R;
import java.util.List;
public class GroundItemManager {
    public static GroundItem create(Item item, Player player) { return R.n("GroundItemManager.create", item, player); }
    public static GroundItem create(Item item, Location location, Player player) { return R.n("GroundItemManager.create", item, location, player); }
    public static GroundItem destroy(GroundItem item) { return R.n("GroundItemManager.destroy", item); }
    public static List<GroundItem> getItems() { return R.list("GroundItemManager.getItems"); }
}
''',
'core/game/node/scenery/Scenery.java': r'''package core.game.node.scenery;
import core.game.world.map.Location;
import oracle.R;
public class Scenery extends core.game.node.Node {
    public int o_rotation, o_type = 10, o_charge = 1000;
    public Scenery() {}
    public Scenery(int id, Location location, int type, int rotation) {
        super(null, location);
        R.v("Scenery", id, location, type, rotation);
        R.fresh(this, "scenery");
        this.key = R.label(this);
        this.nodeId = id; this.o_type = type; this.o_rotation = rotation;
    }
    public int getRotation() { return o_rotation; }
    public int getType() { return o_type; }
    public Scenery transform(int id, int rotation, Location location) { return R.n("Scenery.transform", this, id, rotation, location); }
    public int getCharge() { return o_charge; }
    public void setCharge(int charge) { R.v("Scenery.setCharge", this, charge); this.o_charge = charge; }
    public core.cache.def.impl.SceneryDefinition getDefinition() { return core.cache.def.impl.SceneryDefinition.forId(getId()); }
}
''',
'core/game/node/scenery/SceneryBuilder.java': r'''package core.game.node.scenery;
import oracle.R;
public class SceneryBuilder {
    public static boolean replace(Scenery remove, Scenery construct) { return R.b("SceneryBuilder.replace", remove, construct); }
    public static boolean replace(Scenery remove, Scenery construct, int restoreTicks) { return R.b("SceneryBuilder.replace", remove, construct, restoreTicks); }
    public static Object add(Scenery object) { R.v("SceneryBuilder.add", object); return null; }
    public static boolean remove(Scenery object) { return R.b("SceneryBuilder.remove", object); }
}
''',
'core/game/container/Container.java': r'''package core.game.container;
import core.game.node.item.Item;
import oracle.R;
import java.util.ArrayList;
public class Container {
    public final Object owner;
    public final String name;
    public String key;
    public Container(Object owner, String name) { this.owner = owner; this.name = name; }
    public boolean add(Item item) { return R.b("Container.add", this, item); }
    public boolean add(Item item, boolean fireListener) { return R.b("Container.add", this, item, fireListener); }
    public boolean remove(Item item) { return R.b("Container.remove", this, item); }
    public boolean remove(Item item, int slot, boolean fireListener) { return R.b("Container.remove", this, item, slot, fireListener); }
    public Item replace(Item item, int slot) { return R.n("Container.replace", this, item, slot); }
    public Item replace(Item item, int slot, boolean fireListener) { return R.n("Container.replace", this, item, slot, fireListener); }
    public void update() { R.v("Container.update", this); }
    public Item get(int slot) { return R.n("Container.get", this, slot); }
    public boolean contains(int itemId, int amount) { return R.b("Container.contains", this, itemId, amount); }
    public boolean hasSpaceFor(Item item) { return R.b("Container.hasSpaceFor", this, item); }
    public ArrayList<Item> getAll(Item item) { return new ArrayList<>(R.<Item>list("Container.getAll", this, item)); }
    public int freeSlots() { return R.i("Container.freeSlots", this); }
    public int itemCount() { return R.i("Container.itemCount", this); }
    public boolean containItems(int... itemIds) { return R.b("Container.containItems", this, itemIds); }
    public int getAmount(int id) { return R.i("Container.getAmount", this, id); }
    public boolean isEmpty() { return R.b("Container.isEmpty", this); }
    public boolean canAdd(Item item) { return R.b("Container.canAdd", this, item); }
    public Item[] toArray() { return R.<Item>list("Container.toArray", this).toArray(new Item[0]); }
}
''',
'core/game/container/impl/EquipmentContainer.java': r'''package core.game.container.impl;
public class EquipmentContainer extends core.game.container.Container {
    public EquipmentContainer(Object owner, String name) { super(owner, name); }
}
''',
'core/game/container/impl/BankContainer.java': r'''package core.game.container.impl;
import oracle.R;
public class BankContainer extends core.game.container.Container {
    public BankContainer(Object owner, String name) { super(owner, name); }
    public void open() { R.v("BankContainer.open", this); }
    public void openDepositBox() { R.v("BankContainer.openDepositBox", this); }
}
''',
'core/cache/def/impl/ItemDefinition.java': r'''package core.cache.def.impl;
import java.util.HashMap;
import java.util.Map;
public class ItemDefinition {
    public int id = -1, noteId = -1;
    public String name;
    public boolean stackable, unnoted = true;
    public String[] actions = new String[0];
    public Map<String, Object> configurations = new HashMap<>();
    public static ItemDefinition forId(int itemId) { return oracle.R.itemDefinition(itemId); }
    public String getName() { return name; }
    public boolean isStackable() { return stackable; }
    public boolean isUnnoted() { return unnoted; }
    public int getNoteId() { return noteId; }
    public boolean hasAction(String optionName) { return oracle.R.b("ItemDefinition.hasAction", id, optionName); }
    @SuppressWarnings("unchecked")
    public <V> V getConfiguration(String key, V fallback) {
        V value = (V) configurations.get(key);
        return value == null ? fallback : value;
    }
}
''',
'core/cache/def/impl/NPCDefinition.java': r'''package core.cache.def.impl;
public class NPCDefinition {
    public int id;
    public static NPCDefinition forId(int id) { NPCDefinition d = new NPCDefinition(); d.id = id; return d; }
    public boolean hasAction(String optionName) { return oracle.R.b("NPCDefinition.hasAction", id, optionName); }
}
''',
'core/cache/def/impl/SceneryDefinition.java': r'''package core.cache.def.impl;
public class SceneryDefinition {
    public int id;
    public static SceneryDefinition forId(int id) { return oracle.R.ev("SceneryDefinition.forId", id) == null ? null : sceneryDefinition(id); }
    static SceneryDefinition sceneryDefinition(int id) { SceneryDefinition d = new SceneryDefinition(); d.id = id; return d; }
    public boolean hasAction(String optionName) { return oracle.R.b("SceneryDefinition.hasAction", id, optionName); }
    public int[] getChildrenIds() { return oracle.R.ints("SceneryDefinition.getChildrenIds", id); }
}
''',
'core/cache/def/impl/AnimationDefinition.java': r'''package core.cache.def.impl;
public class AnimationDefinition {
    public int id;
    public static AnimationDefinition forId(int id) { AnimationDefinition d = new AnimationDefinition(); d.id = id; return d; }
    public int getCycles() { return oracle.R.i("AnimationDefinition.getCycles", id); }
    public int getDurationTicks() { return oracle.R.i("AnimationDefinition.getDurationTicks", id); }
}
''',
'core/cache/def/impl/VarbitDefinition.java': r'''package core.cache.def.impl;
public class VarbitDefinition {
    public int id, varpId, startBit, endBit;
    public static VarbitDefinition forId(int id) { return oracle.R.varbit(id); }
    public int getVarpId() { return varpId; }
    public int getStartBit() { return startBit; }
    public int getMask() {
        int mask = 0;
        for (int i = startBit; i <= endBit; i++)
            mask |= (1 << (i - startBit));
        return mask;
    }
}
''',
'core/game/world/update/flag/context/Animation.java': r'''package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
import java.util.Arrays;
public class Animation implements oracle.R.Describable {
    final int id;
    final Priority priority;
    Object object;
    public Animation(int id) { this(id, Priority.MID); }
    public Animation(int id, Priority priority) { this.id = id; this.priority = priority; }
    public int getId() { return id; }
    public Priority getPriority() { return priority; }
    public void setObject(Object object) { this.object = object; }
    public core.cache.def.impl.AnimationDefinition getDefinition() { return core.cache.def.impl.AnimationDefinition.forId(id); }
    public Object describe() { return Arrays.asList("anim", id, priority.name(), oracle.R.value(object)); }
}
''',
'core/game/world/update/flag/context/Graphics.java': r'''package core.game.world.update.flag.context;
import java.util.Arrays;
public class Graphics implements oracle.R.Describable {
    final int id, height, delay;
    public Graphics(int id) { this(id, 0, 0); }
    public Graphics(int id, int height) { this(id, height, 0); }
    public Graphics(int id, int height, int delay) { this.id = id; this.height = height; this.delay = delay; }
    public static void send(Graphics graphics, core.game.world.map.Location location) { oracle.R.v("Graphics.send", graphics, location); }
    public Object describe() { return Arrays.asList("gfx", id, height, delay); }
}
''',
'core/game/node/entity/impl/Animator.java': r'''package core.game.node.entity.impl;
import core.game.world.update.flag.context.Animation;
public class Animator {
    public final Object owner;
    public Animator(Object owner) { this.owner = owner; }
    public static enum Priority { LOW, MID, HIGH, VERY_HIGH }
    public boolean animate(Animation animation) { return oracle.R.b("Animator.animate", owner, animation); }
    public void forceAnimation(Animation animation) { oracle.R.v("Animator.forceAnimation", owner, animation); }
}
''',
'core/game/node/entity/impl/WalkingQueue.java': r'''package core.game.node.entity.impl;
public class WalkingQueue {
    public final Object owner;
    public WalkingQueue(Object owner) { this.owner = owner; }
    public void reset() { oracle.R.v("WalkingQueue.reset", owner); }
}
''',
'core/game/node/entity/impl/PulseManager.java': r'''package core.game.node.entity.impl;
public class PulseManager {
    public final Object owner;
    public PulseManager(Object owner) { this.owner = owner; }
    public void run(core.game.system.task.Pulse pulse) { oracle.R.pulse("PulseManager.run", owner, pulse); }
    public void clear() { oracle.R.v("PulseManager.clear", owner); }
}
''',
'core/game/node/entity/impl/Properties.java': r'''package core.game.node.entity.impl;
import core.game.world.map.Location;
public class Properties {
    public final Object owner;
    public final core.game.node.entity.combat.CombatPulse o_combatPulse;
    public Properties(Object owner) { this.owner = owner; this.o_combatPulse = new core.game.node.entity.combat.CombatPulse(owner); }
    public void setTeleportLocation(Location location) { oracle.R.v("Properties.setTeleportLocation", owner, location); }
    public Location getTeleportLocation() { return oracle.R.n("Properties.getTeleportLocation", owner); }
    public core.game.node.entity.combat.CombatPulse getCombatPulse() { return o_combatPulse; }
}
''',
'core/game/node/entity/impl/ForceMovement.java': r'''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
public class ForceMovement {
    final Entity entity; final Location destination; final int start, end;
    public ForceMovement(Entity entity, Location destination, int startSpeed, int animationSpeed) {
        this.entity = entity; this.destination = destination; this.start = startSpeed; this.end = animationSpeed;
    }
    public void run() { oracle.R.v("ForceMovement.run", entity, destination, start, end); }
}
''',
'core/game/node/entity/impl/Projectile.java': r'''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
import oracle.R;
public class Projectile {
    public static Projectile create(Entity source, Entity victim, int projectileId) { return R.n("Projectile.create", source, victim, projectileId); }
    public static Projectile create(Location start, Location destination, int projectileId, int startHeight, int endHeight, int startDelay, int speed, int angle, int distance) {
        return R.n("Projectile.create", start, destination, projectileId, startHeight, endHeight, startDelay, speed, angle, distance);
    }
    public void send() { R.v("Projectile.send", this); }
}
''',
'core/game/node/entity/lock/ActionLocks.java': r'''package core.game.node.entity.lock;
public class ActionLocks {
    public final Object owner;
    public ActionLocks(Object owner) { this.owner = owner; }
    public void lockMovement(int ticks) { oracle.R.v("ActionLocks.lockMovement", owner, ticks); }
    public void lockInteractions(int ticks) { oracle.R.v("ActionLocks.lockInteractions", owner, ticks); }
}
''',
'core/game/node/entity/combat/ImpactHandler.java': r'''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
public class ImpactHandler {
    public final Object owner;
    public ImpactHandler(Object owner) { this.owner = owner; }
    public static enum HitsplatType { MISS, NORMAL, POISON, DISEASE, NORMAL_1, VENOM }
    public Object manualHit(Entity source, int hit, HitsplatType type) { oracle.R.v("ImpactHandler.manualHit", owner, source, hit, type); return null; }
}
''',
'core/game/node/entity/skill/Skills.java': r'''package core.game.node.entity.skill;
import oracle.R;
public class Skills {
    /* ORIGINAL_SKILL_NAME */
    public final Object owner;
    public Skills(Object owner) { this.owner = owner; }
    public int getLevel(int slot) { return R.i("Skills.getLevel", owner, slot); }
    public int getStaticLevel(int slot) { return R.i("Skills.getStaticLevel", owner, slot); }
    public void addExperience(int slot, double experience) { R.v("Skills.addExperience", owner, slot, experience); }
    public void setLevel(int slot, int level) { R.v("Skills.setLevel", owner, slot, level); }
    public int heal(int health) { return R.i("Skills.heal", owner, health); }
    public int getMasteredSkills() { return R.i("Skills.getMasteredSkills", owner); }
    public void incrementPrayerPoints(double amount) { R.v("Skills.incrementPrayerPoints", owner, amount); }
    public void decrementPrayerPoints(double amount) { R.v("Skills.decrementPrayerPoints", owner, amount); }
}
''',
}
