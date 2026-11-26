"""Recording stand-ins for everything the bot engine reaches outside its own files.

Each is named after the class it stands for and records the original's call under `Class.method` (oracle.R). Plain
fields and getters (locations, ids, names, the active flag, node kinds, item names, the session object) are state and
are not recorded. `/*NAME*/` markers are filled with original text by generate.py.
"""
ADAPTERS = {}


def adapter(name, text):
    ADAPTERS[name] = text.lstrip('\n')


# ---------------------------------------------------------------- nodes
adapter('core/game/node/Node.java', r'''
package core.game.node;
import core.game.interaction.DestinationFlag;
import core.game.interaction.InteractPlugin;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.world.map.Direction;
import core.game.world.map.Location;
/** Fields are case state; setDirection and getInteraction are calls. The four casts are the original's. */
public abstract class Node {
    public String key;
    protected String name;
    protected Location location;
    protected int index;
    protected Direction direction;
    protected boolean active = true;
    protected InteractPlugin interactPlugin;
    protected DestinationFlag destinationFlag;
    public int idValue;
    public Node(String name, Location location) { this.name = name; this.location = location; }
    public String getName() { return name; }
    public Location getLocation() { return location; }
    public int getId() { return idValue; }
    public int getIndex() { return index; }
    public void setIndex(int index) { this.index = index; }
    public boolean isActive() { return active; }
    public void oracleActive(boolean active) { this.active = active; }
    public void oracleLocation(Location location) { this.location = location; }
    public void oracleDirection(Direction direction) { this.direction = direction; }
    public Direction getDirection() { return direction; }
    public void setDirection(Direction direction) { oracle.R.v("Node.setDirection", this, direction); this.direction = direction; }
    public InteractPlugin getInteraction() {
        if (!oracle.R.b(true, "Node.getInteraction", this)) return null;
        if (interactPlugin == null) interactPlugin = new InteractPlugin(this);
        return interactPlugin;
    }
    public NPC asNpc() { return (NPC) this; }
    public Player asPlayer() { return (Player) this; }
    public Scenery asScenery() { return (Scenery) this; }
    public Item asItem() { return (Item) this; }
}
''')

adapter('core/game/node/scenery/Scenery.java', r'''
package core.game.node.scenery;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
public class Scenery extends Node {
    public Scenery(int id, String name, Location location) { super(name, location); this.idValue = id; }
    public Scenery getChild(Player player) { return oracle.R.n("Scenery.getChild", this, player); }
}
''')

adapter('core/game/node/entity/npc/NPC.java', r'''
package core.game.node.entity.npc;
import core.cache.def.impl.NPCDefinition;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
public class NPC extends Entity {
    private final NPCDefinition definition = new NPCDefinition(this);
    public NPC(int id, String name, Location location) { super(name, location); this.idValue = id; }
    public NPCDefinition getDefinition() { return definition; }
}
''')

adapter('core/cache/def/impl/NPCDefinition.java', r'''
package core.cache.def.impl;
public class NPCDefinition {
    private final Object npc;
    public NPCDefinition(Object npc) { this.npc = npc; }
    public boolean hasAction(String action) { return oracle.R.b("NPCDefinition.hasAction", npc, action); }
}
''')

adapter('core/game/node/item/GroundItem.java', r'''
package core.game.node.item;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
public class GroundItem extends Item {
    private final Player dropper;
    public GroundItem(int id, int amount, Location location, Player dropper) { super(id, amount); this.location = location; this.dropper = dropper; }
    public Player getDropper() { return dropper; }
}
''')

adapter('core/game/node/item/GroundItemManager.java', r'''
package core.game.node.item;
import java.util.ArrayList;
import java.util.List;
public final class GroundItemManager {
    private static final List<GroundItem> ITEMS = new ArrayList<GroundItem>() {
        @Override public boolean contains(Object o) { return oracle.R.b("GroundItemManager.contains", o); }
    };
    public static List<GroundItem> getItems() { return ITEMS; }
}
''')

adapter('core/game/node/item/ItemPlugin.java', 'package core.game.node.item;\npublic class ItemPlugin {}\n')
adapter('core/game/node/entity/combat/equipment/DegradableEquipment.java', r'''
package core.game.node.entity.combat.equipment;
public class DegradableEquipment {
    public static int getDropReplacement(int id) { return id; }
}
''')

adapter('core/cache/def/impl/ItemDefinition.java', r'''
package core.cache.def.impl;
import core.game.interaction.OptionHandler;
import core.game.node.item.ItemPlugin;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
/**
 * forId and getName are state (the case's `items`); every other getter is a recorded call. `handlers` answers
 * `equipment_slot` and `requirements` from the case: the requirements are one java.util.HashMap per item, filled in
 * the listed order, so the originals walk it in the JDK's order.
 */
public class ItemDefinition {
    private static final Map<Integer, ItemDefinition> ALL = new HashMap<>();
    public final int id;
    private HashMap<Integer, Integer> requirements;
    private final Map<String, Object> handlers = new HashMap<String, Object>() {
        @Override public Object get(Object key) { return handler((String) key); }
        @Override public Object getOrDefault(Object key, Object fallback) { Object v = handler((String) key); return v == null ? fallback : v; }
    };
    private ItemDefinition(int id) { this.id = id; }
    public static ItemDefinition forId(int id) { return ALL.computeIfAbsent(id, ItemDefinition::new); }
    private Object handler(String key) {
        oracle.R.v("ItemDefinition.handlers.get", id, key);
        Object v = oracle.R.itemSpec(id).get(key);
        if (v instanceof Long) return ((Long) v).intValue();
        if (v instanceof List) {
            if (requirements == null) {
                requirements = new HashMap<>();
                for (Object pair : (List<?>) v) requirements.put(oracle.R.n(((List<?>) pair).get(0)), oracle.R.n(((List<?>) pair).get(1)));
            }
            return requirements;
        }
        return v;
    }
    public String getName() { Object n = oracle.R.itemSpec(id).get("name"); return n == null ? "Item" + id : (String) n; }
    public Map<String, Object> getHandlers() { return handlers; }
    public int getNoteId() { return oracle.R.i(-1, "ItemDefinition.getNoteId", id); }
    public int getAlchemyValue(boolean high) { return oracle.R.i("ItemDefinition.getAlchemyValue", id, high); }
    public boolean isTradeable() { return oracle.R.b(true, "ItemDefinition.isTradeable", id); }
    public int getValue() { return oracle.R.i("ItemDefinition.getValue", id); }
    @SuppressWarnings("unchecked")
    public <T> T getConfiguration(String key) { Object a = oracle.R.ev("ItemDefinition.getConfiguration", id, key); return a == null ? null : (T) (Object) ((Number) a).intValue(); }
    public boolean isUnnoted() { return true; }
    public ItemPlugin getItemPlugin() { return null; }
    public static OptionHandler getOptionHandler(int id, String name) { return null; }
}
''')

adapter('core/game/system/config/ItemConfigParser.java', r'''
package core.game.system.config;
public final class ItemConfigParser {
    /*GE_PRICE*/
}
''')

# ---------------------------------------------------------------- interaction
adapter('core/game/interaction/DestinationFlag.java', 'package core.game.interaction;\npublic enum DestinationFlag { ENTITY, FOLLOW_ENTITY, LOCATION, OBJECT, ITEM }\n')
adapter('core/game/interaction/OptionHandler.java', 'package core.game.interaction;\npublic abstract class OptionHandler {}\n')
adapter('core/game/interaction/Option.java', r'''
package core.game.interaction;
public class Option {
    private final String name;
    public Option(String name) { this.name = name; }
    public String getName() { return name; }
}
''')

adapter('core/game/interaction/InteractPlugin.java', r'''
package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
/** Options are named by the case; an option named {@code "?"} has a null name. */
public class InteractPlugin {
    private final Node node;
    private final Map<String, Option> options = new HashMap<>();
    public InteractPlugin(Node node) { this.node = node; }
    private Option option(Object name) {
        if (name == null) return null;
        return options.computeIfAbsent((String) name, n -> new Option("?".equals(n) ? null : n));
    }
    public Option[] getOptions() {
        Object a = oracle.R.ev("Interaction.getOptions", node);
        List<?> l = a == null ? java.util.Collections.emptyList() : (List<?>) a;
        Option[] out = new Option[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = option(l.get(i));
        return out;
    }
    public Option get(int index) { return option(oracle.R.ev("Interaction.get", node, index)); }
    public void handle(Player player, Option option) {
        oracle.R.v("Interaction.handle", node, player, option == null ? null : option.getName() == null ? "?" : option.getName());
    }
}
''')

adapter('core/game/interaction/InteractionListeners.kt', r'''
package core.game.interaction
import core.game.node.Node
import core.game.node.entity.player.Player
/** The two forms ScriptAPI calls, with the original's parameter types (a null argument fails on entry). */
object InteractionListeners {
    @JvmStatic
    fun run(used: Node, with: Node, type: IntType, player: Player): Boolean = oracle.R.b("InteractionListeners.runUse", used, with, type, player)
    @JvmStatic
    fun run(id: Int, type: IntType, option: String, player: Player, node: Node): Boolean = oracle.R.b("InteractionListeners.run", id, type, option, player, node)
}
''')

adapter('core/game/interaction/NodeUsageEvent.java', r'''
package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.player.Player;
public class NodeUsageEvent {
    public final Player player;
    public final Node used, with;
    public NodeUsageEvent(Player player, int componentId, Node used, Node with) { this.player = player; this.used = used; this.with = with; }
}
''')
adapter('core/game/interaction/PluginInteractionManager.java', r'''
package core.game.interaction;
import core.game.node.entity.player.Player;
public class PluginInteractionManager {
    public static boolean handle(Player player, NodeUsageEvent event) { return oracle.R.b("PluginInteractionManager.handle", player, event.used, event.with); }
}
''')
adapter('core/game/interaction/UseWithHandler.java', r'''
package core.game.interaction;
public abstract class UseWithHandler {
    public static void run(final NodeUsageEvent event) { oracle.R.v("UseWithHandler.run", event.player, event.used, event.with); }
}
''')

adapter('core/game/interaction/MovementPulse.java', r'''
package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.system.task.Pulse;
/** Construction is recorded; the movement itself is not part of this corpus. */
public abstract class MovementPulse extends Pulse {
    protected MovementPulse() { super(1); }
    public MovementPulse(Entity mover, Node destination) { this(mover, destination, (DestinationFlag) null); }
    public MovementPulse(Entity mover, Node destination, DestinationFlag destinationFlag) {
        super(1);
        oracle.R.v("MovementPulse.new", mover, destination, destinationFlag);
    }
}
''')

adapter('core/game/interaction/Script.java', r'''
package core.game.interaction;
/** What ScriptProcessor.getActiveScript() answers: `execution` is null for the key "active:null". */
public class Script<T> {
    public final Object execution;
    public Script(Object execution) { this.execution = execution; }
    public Object getExecution() { return execution; }
    public static final class Execution {}
}
''')
adapter('core/game/interaction/ScriptProcessor.java', r'''
package core.game.interaction;
public class ScriptProcessor {
    private final Object entity;
    private final Script<?> active = new Script<>(new Script.Execution()), empty = new Script<>(null);
    public ScriptProcessor(Object entity) { this.entity = entity; }
    public Script<?> getActiveScript() {
        Object a = oracle.R.ev("ScriptProcessor.getActiveScript", entity);
        return a == null ? null : "active:null".equals(a) ? empty : active;
    }
    public int getDelay() { return oracle.R.i("ScriptProcessor.getDelay", entity); }
}
''')

# ---------------------------------------------------------------- pulses
adapter('core/game/system/task/Pulse.java', r'''
package core.game.system.task;
import core.game.node.Node;
import java.util.ArrayList;
import java.util.List;
/** The base stop() is recorded; delay and checks are what the constructor was given. */
public abstract class Pulse {
    public boolean running = true;
    private int delay;
    protected Node[] checks;
    public Pulse() { this(1); }
    public Pulse(int delay) { this.delay = delay; }
    public Pulse(int delay, Node... checks) { this.delay = delay; this.checks = checks; }
    public abstract boolean pulse();
    public void stop() { oracle.R.v("Pulse.stop", this); running = false; }
    public boolean isRunning() { return running; }
    public int getDelay() { return delay; }
    public List<Object> checkKeys() {
        List<Object> out = new ArrayList<>();
        if (checks != null) for (Node n : checks) out.add(oracle.R.value(n));
        return out;
    }
}
''')
adapter('core/game/node/entity/impl/PulseType.java', 'package core.game.node.entity.impl;\npublic enum PulseType { STANDARD, STRONG, CUSTOM_1, CUSTOM_2 }\n')
adapter('core/game/node/entity/impl/PulseManager.java', r'''
package core.game.node.entity.impl;
import core.game.system.task.Pulse;
public class PulseManager {
    private final Object entity;
    public PulseManager(Object entity) { this.entity = entity; }
    public void run(Pulse pulse) { oracle.R.pulse("PulseManager.run", entity, pulse, false); }
    public void run(Pulse pulse, PulseType type) { oracle.R.pulse("PulseManager.run", entity, pulse, type == PulseType.STANDARD); }
    public boolean hasPulseRunning() { return oracle.R.b("PulseManager.hasPulseRunning", entity); }
    public Pulse getCurrent() {
        Object a = oracle.R.ev("PulseManager.getCurrent", entity);
        if (a == null) return null;
        return ((String) a).startsWith("foreign:") ? oracle.R.foreign((String) a) : (Pulse) oracle.R.node(a);
    }
    public void clear() { oracle.R.v("PulseManager.clear", entity); }
}
''')
adapter('core/game/world/PulseRunner.java', r'''
package core.game.world;
import core.game.system.task.Pulse;
public class PulseRunner {
    public void submit(Pulse pulse) { oracle.R.pulse("Pulser.submit", null, pulse); }
}
''')

# ---------------------------------------------------------------- entity and player
adapter('core/game/node/entity/Entity.java', r'''
package core.game.node.entity;
import core.game.interaction.ScriptProcessor;
import core.game.node.Node;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.impl.Properties;
import core.game.node.entity.impl.PulseManager;
import core.game.node.entity.impl.WalkingQueue;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneMonitor;
import core.game.world.update.UpdateMasks;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import java.util.ArrayList;
import java.util.List;
import oracle.R;
/** Every member the engine reaches on an entity, recorded under the class the original call belongs to. */
public class Entity extends Node {
    public ScriptProcessor scripts = new ScriptProcessor(this);
    private final Skills skills = new Skills(this);
    private final Properties properties = new Properties(this);
    private final PulseManager pulseManager = new PulseManager(this);
    private final WalkingQueue walkingQueue = new WalkingQueue(this);
    private final ImpactHandler impactHandler = new ImpactHandler(this);
    private final Animator animator = new Animator(this);
    private final UpdateMasks updateMasks = new UpdateMasks(this);
    private final ZoneMonitor zoneMonitor = new ZoneMonitor(this);
    public Entity(String name, Location location) { super(name, location); }
    public void setLocation(Location location) { R.v("Node.setLocation", this, location); this.location = location; }
    public Skills getSkills() { return skills; }
    public Properties getProperties() { return properties; }
    public PulseManager getPulseManager() { return pulseManager; }
    public WalkingQueue getWalkingQueue() { return walkingQueue; }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public Animator getAnimator() { return animator; }
    public UpdateMasks getUpdateMasks() { return updateMasks; }
    public ZoneMonitor getZoneMonitor() { return zoneMonitor; }
    public boolean isPlayer() { return this instanceof core.game.node.entity.player.Player; }
    public void init() {}
    public void tick() {}
    public void update() {}
    public void reset() {}
    public void clear() { R.v("Entity.clear", this); }
    public void finalizeDeath(Entity killer) {}
    public void fullRestore() { R.v("Player.fullRestore", this); }
    public boolean face(Entity entity) { R.v("Entity.face", this, entity); return true; }
    public boolean faceLocation(Location location) { R.v("Entity.faceLocation", this, location); return true; }
    public boolean inCombat() { return R.b("Entity.inCombat", this); }
    public void attack(Node target) { R.v("Entity.attack", this, target); }
    public boolean isTeleBlocked() { return R.b("Entity.isTeleBlocked", this); }
    public void lock() { R.v("Entity.lock", this); }
    public void lock(int ticks) { R.v("Entity.lockFor", this, ticks); }
    public void unlock() { R.v("Entity.unlock", this); }
    public boolean visualize(Animation animation, Graphics graphics) { R.v("Entity.visualize", this, animation.id, graphics.id); return true; }
    public boolean sendChat(String message) { R.v("Entity.sendChat", this, message); return true; }
    public void setAttribute(String key, Object value) { R.v("Entity.setAttribute", this, key, value); }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        Object a = R.ev("Entity.getAttribute", this, key);
        if (a == null) return fail;
        List<Object> out = new ArrayList<>();
        for (Object k : (List<?>) a) out.add(R.node(k));
        return (T) out;
    }
}
''')

adapter('core/game/node/entity/player/Player.java', r'''
package core.game.node.entity.player;
import content.global.skill.construction.HouseManager;
import content.global.skill.summoning.familiar.FamiliarManager;
import core.game.container.Container;
import core.game.container.impl.EquipmentContainer;
import core.game.dialogue.DialogueInterpreter;
import core.game.node.entity.Entity;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.PlayerFlags;
import core.game.node.entity.player.link.Settings;
import core.game.node.entity.player.link.SkullManager;
import core.game.node.entity.player.link.appearance.Appearance;
import core.game.node.entity.player.link.prayer.Prayer;
import core.game.node.entity.player.link.quest.QuestRepository;
import core.game.system.communication.CommunicationInfo;
import core.game.world.GameWorld;
import core.game.world.repository.Repository;
import core.game.world.update.UpdateSequence;
import core.net.IoSession;
import core.tools.StringUtils;
import core.worker.ManagementEvents;
import java.util.HashMap;
import kotlin.Unit;
import kotlin.jvm.functions.Function1;
import oracle.R;
import proto.management.ClanLeaveNotification;
import proto.management.PlayerStatusUpdate;
/**
 * clear, finishClear, sendLogoutEvents, setPlaying, isArtificial, getName and getUsername are the original text.
 * The constructor, init, tick, reset, finalizeDeath and checkForWealthUpdate record their call (init also leaves the
 * player playing, as the login configuration it stands for does); every manager is a recording stand-in.
 */
public class Player extends Entity {
    private final PlayerDetails details;
    protected boolean artificial;
    private boolean playing;
    public HashMap<String, Function1<Player, Unit>> logoutListeners = new HashMap<>();
    private final Appearance appearance = new Appearance(this);
    private final EquipmentContainer equipment = new EquipmentContainer(this);
    private final Container inventory = new Container(this, "inventory");
    private final Container bank = new Container(this, "bank");
    private final InterfaceManager interfaceManager = new InterfaceManager(this);
    private final Settings settings = new Settings(this);
    private final PlayerFlags playerFlags = new PlayerFlags(this);
    private final SkullManager skullManager = new SkullManager(this);
    private final Prayer prayer = new Prayer(this);
    private final DialogueInterpreter dialogueInterpreter = new DialogueInterpreter(this);
    private final PacketDispatch packetDispatch = new PacketDispatch(this);
    private final QuestRepository questRepository = new QuestRepository(this);
    private final FamiliarManager familiarManager = new FamiliarManager(this);
    private final CommunicationInfo communication = new CommunicationInfo(this);

    public Player(PlayerDetails details) {
        super(details.getUsername(), core.ServerConstants.START_LOCATION);
        this.details = details;
        details.owner = this;
        if (!R.quiet) {
            // A constructor that fails here leaves an object nothing holds: it is tracked only once it exists.
            R.v("Player.new", details.getUsername());
            this.key = "bot#" + (R.bots.size() + 1);
            R.nodes.put(this.key, this);
            R.bots.add(this);
        }
    }

    @Override public void init() { R.v("Player.init", this); playing = true; }
    @Override public void tick() { R.v("Player.tick", this); }
    @Override public void reset() { R.v("Player.reset", this); }
    @Override public void finalizeDeath(Entity killer) { R.v("Player.finalizeDeath", this, killer); }

    /*PLAYER_CLEAR*/

    /*PLAYER_FINISH_CLEAR*/

    /*PLAYER_SEND_LOGOUT_EVENTS*/

    private void checkForWealthUpdate(boolean force) { R.v("Player.checkForWealthUpdate", this, force); }

    /*PLAYER_SET_PLAYING*/

    /*PLAYER_IS_PLAYING*/

    /*PLAYER_IS_ARTIFICIAL*/

    /*PLAYER_GET_NAME*/

    /*PLAYER_GET_USERNAME*/

    public void oracleArtificial(boolean artificial) { this.artificial = artificial; }
    public PlayerDetails getDetails() { return details; }
    public IoSession getSession() { return details.getSession(); }
    public Appearance getAppearance() { return appearance; }
    public EquipmentContainer getEquipment() { return equipment; }
    public Container getInventory() { return inventory; }
    public Container getBank() { return bank; }
    public InterfaceManager getInterfaceManager() { return interfaceManager; }
    public Settings getSettings() { return settings; }
    public PlayerFlags getPlayerFlags() { return playerFlags; }
    public SkullManager getSkullManager() { return skullManager; }
    public Prayer getPrayer() { return prayer; }
    public DialogueInterpreter getDialogueInterpreter() { return dialogueInterpreter; }
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public QuestRepository getQuestRepository() { return questRepository; }
    public CommunicationInfo getCommunication() { return communication; }
    public boolean hasModalOpen() { return R.b("Player.hasModalOpen", this); }
    public void debug(String text) { R.v("Player.debug", this, text); }
}
''')

adapter('core/game/node/entity/player/info/PlayerDetails.java', r'''
package core.game.node.entity.player.info;
import core.net.IoSession;
public class PlayerDetails {
    public Object owner;
    private final String username;
    private IoSession session;
    public PlayerDetails(String username) { this.username = username; }
    public String getUsername() { return username; }
    public IoSession getSession() { return session; }
    public void setSession(IoSession session) {
        this.session = session;
        oracle.R.v("PlayerDetails.setSession", owner, session.getClass().getSimpleName());
    }
    public void save() { oracle.R.v("PlayerDetails.save", owner); }
}
''')

adapter('core/net/IoSession.java', r'''
package core.net;
import java.nio.ByteBuffer;
import java.nio.channels.SelectionKey;
import java.util.concurrent.ExecutorService;
/** The members ArtificialSession overrides record their call here, so an override that was not one would show. */
public class IoSession {
    private Object object;
    public IoSession(SelectionKey key, ExecutorService service) {}
    public String getRemoteAddress() { oracle.R.v("IoSession.getRemoteAddress"); return null; }
    public void write(Object context) { write(context, false); }
    public void write(Object context, boolean instant) { oracle.R.v("IoSession.write"); }
    public void queue(ByteBuffer buffer) { oracle.R.v("IoSession.queue"); }
    public void write() { oracle.R.v("IoSession.flush"); }
    public void disconnect() { oracle.R.v("IoSession.disconnect"); }
    public Object getObject() { return object; }
    public void setObject(Object object) { this.object = object; }
}
''')

SIMPLE = r'''
package %(package)s;
public class %(name)s {
    private final Object owner;
    public %(name)s(Object owner) { this.owner = owner; }
%(body)s
}
'''


def simple(path, body):
    package, name = path.rsplit('/', 1)
    adapter(path + '.java', SIMPLE % dict(package=package.replace('/', '.'), name=name, body=body.strip('\n')))


simple('core/game/node/entity/skill/Skills', r'''
    /*SKILL_IDS*/
    public void setStaticLevel(int skill, int level) { oracle.R.v("Skills.setStaticLevel", owner, skill, level); }
    public void setLevel(int skill, int level) { oracle.R.v("Skills.setLevel", owner, skill, level); }
    public int getStaticLevel(int skill) { return oracle.R.i(1, "Skills.getStaticLevel", owner, skill); }
    public int getLevel(int skill) { return oracle.R.i(1, "Skills.getLevel", owner, skill); }
    public double getExperience(int skill) { return oracle.R.d("Skills.getExperience", owner, skill); }
    public void addExperience(int skill, double experience) { oracle.R.v("Skills.addExperience", owner, skill, experience); }
    public int levelFromXP(double experience) { return oracle.R.i("Skills.levelFromXP", owner, experience); }
    public int getLifepoints() { return oracle.R.i(10, "Skills.getLifepoints", owner); }
    public void setLifepoints(int lifepoints) { oracle.R.v("Skills.setLifepoints", owner, lifepoints); }
    public void updateCombatLevel() { oracle.R.v("Skills.updateCombatLevel", owner); }
    public int getHighestCombatSkillId() { return oracle.R.i("Skills.getHighestCombatSkillId", owner); }
    public void copy(Skills other) { oracle.R.v("Skills.copy", owner, other.owner); }
''')
simple('core/game/node/entity/impl/Properties', r'''
    private final core.game.node.entity.combat.CombatPulse combatPulse = new core.game.node.entity.combat.CombatPulse(this);
    public Object owner() { return owner; }
    public void setSpawnLocation(core.game.world.map.Location location) { oracle.R.v("Properties.setSpawnLocation", owner, location); }
    public core.game.world.map.Location getTeleportLocation() { return null; }
    public void setTeleportLocation(core.game.world.map.Location location) { oracle.R.v("Properties.setTeleportLocation", owner, location); }
    public boolean isMultiZone() { return oracle.R.b("Properties.isMultiZone", owner); }
    public core.game.node.entity.combat.CombatPulse getCombatPulse() { return combatPulse; }
''')
adapter('core/game/node/entity/combat/CombatPulse.java', r'''
package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import core.game.node.entity.impl.Properties;
public class CombatPulse {
    private final Properties properties;
    public CombatPulse(Properties properties) { this.properties = properties; }
    public Entity getVictim() { return oracle.R.n("CombatPulse.getVictim", properties.owner()); }
    public void delayNextAttack(int ticks) { oracle.R.v("CombatPulse.delayNextAttack", properties.owner(), ticks); }
}
''')
simple('core/game/node/entity/combat/ImpactHandler', r'''
    public int getDisabledTicks() { return 0; }
    public void setDisabledTicks(int ticks) { oracle.R.v("ImpactHandler.setDisabledTicks", owner, ticks); }
''')
simple('core/game/node/entity/impl/Animator', r'''
    public void reset() { oracle.R.v("Animator.reset", owner); }
''')
simple('core/game/node/entity/impl/WalkingQueue', r'''
    public boolean isMoving() { return oracle.R.b("WalkingQueue.isMoving", owner); }
    public void reset() { oracle.R.v("WalkingQueue.reset", owner); }
''')
simple('core/game/world/update/UpdateMasks', r'''
    public boolean register(core.game.world.update.flag.EntityFlag flag, Object context) { oracle.R.v("UpdateMasks.register", owner, flag, context); return true; }
''')
simple('core/game/world/map/zone/ZoneMonitor', r'''
    public boolean clear() { oracle.R.v("ZoneMonitor.clear", owner); return true; }
''')
simple('core/game/node/entity/player/link/appearance/Appearance', r'''
    public Gender getGender() { return Gender.MALE; }
    public void setGender(Gender gender) { oracle.R.v("Appearance.setGender", owner, gender); }
    public void sync() { oracle.R.v("Appearance.sync", owner); }
    public void copy(Appearance other) { oracle.R.v("Appearance.copy", owner, other.owner); }
    public void parse(org.json.simple.JSONObject json) { oracle.R.v("Appearance.parse", owner, json.toJSONString()); }
''')
adapter('core/game/node/entity/player/link/appearance/Gender.java', 'package core.game.node.entity.player.link.appearance;\npublic enum Gender { MALE, FEMALE }\n')
simple('core/game/node/entity/player/link/InterfaceManager', r'''
    public void openDefaultTabs() { oracle.R.v("InterfaceManager.openDefaultTabs", owner); }
    public void closeChatbox() { oracle.R.v("InterfaceManager.closeChatbox", owner); }
    public void openChatbox(int id) { oracle.R.v("InterfaceManager.openChatbox", owner, id); }
    public boolean close() { oracle.R.v("InterfaceManager.close", owner); return true; }
    public void closeSingleTab() { oracle.R.v("InterfaceManager.closeSingleTab", owner); }
    public void openOverlay(core.game.component.Component component) { oracle.R.v("InterfaceManager.openOverlay", owner, component.id); }
''')
adapter('core/game/component/Component.java', 'package core.game.component;\npublic class Component { public final int id; public Component(int id) { this.id = id; } }\n')
simple('core/game/node/entity/player/link/Settings', r'''
    public void setRunToggled(boolean enabled) { oracle.R.v("Settings.setRunToggled", owner, enabled); }
''')
simple('core/game/node/entity/player/link/PlayerFlags', r'''
    public boolean isUpdateSceneGraph() { return oracle.R.b("PlayerFlags.isUpdateSceneGraph", owner); }
    public void setLastSceneGraph(core.game.world.map.Location location) { oracle.R.v("PlayerFlags.setLastSceneGraph", owner, location); }
''')
simple('core/game/node/entity/player/link/SkullManager', r'''
    public boolean isWilderness() { return oracle.R.b("SkullManager.isWilderness", owner); }
    public void setLevel(int level) { oracle.R.v("SkullManager.setLevel", owner, level); }
''')
simple('core/game/node/entity/player/link/prayer/Prayer', r'''
    public boolean toggle(PrayerType type) { oracle.R.v("Prayer.toggle", owner, type); return true; }
''')
adapter('core/game/node/entity/player/link/prayer/PrayerType.java', 'package core.game.node.entity.player.link.prayer;\npublic enum PrayerType { THICK_SKIN, PROTECT_FROM_MELEE, PIETY }\n')
simple('core/game/dialogue/DialogueInterpreter', r'''
    public boolean close() { oracle.R.v("DialogueInterpreter.close", owner); return true; }
''')
simple('core/game/node/entity/player/link/PacketDispatch', r'''
    public void sendInterfaceConfig(int interfaceId, int childId, boolean hide) { oracle.R.v("PacketDispatch.sendInterfaceConfig", owner, interfaceId, childId, hide); }
    public void sendString(String text, int interfaceId, int childId) { oracle.R.v("PacketDispatch.sendString", owner, text, interfaceId, childId); }
''')
simple('core/game/node/entity/player/link/quest/QuestRepository', r'''
    public Quest getQuest(content.data.Quests quest) {
        Object a = oracle.R.ev("QuestRepository.getQuest", owner, quest);
        return a == null ? null : new Quest((String) a);
    }
    public void setStage(Quest quest, int stage) { oracle.R.v("QuestRepository.setStage", owner, quest == null ? null : quest.label, stage); }
''')
adapter('core/game/node/entity/player/link/quest/Quest.java', 'package core.game.node.entity.player.link.quest;\npublic class Quest { public final String label; public Quest(String label) { this.label = label; } }\n')
simple('content/global/skill/summoning/familiar/FamiliarManager', r'''
    public boolean hasFamiliar() { return oracle.R.b("FamiliarManager.hasFamiliar", owner); }
    public Familiar getFamiliar() { return new Familiar(owner); }
''')
simple('content/global/skill/summoning/familiar/Familiar', r'''
    public void clear() { oracle.R.v("Familiar.clear", owner); }
''')
simple('core/game/system/communication/CommunicationInfo', r'''
    public ClanRepository getClan() { return oracle.R.ev("CommunicationInfo.getClan", owner) == null ? null : new ClanRepository(); }
''')
adapter('core/game/system/communication/ClanRepository.java', r'''
package core.game.system.communication;
public class ClanRepository {
    public String getOwner() { return (String) oracle.R.ev("ClanRepository.getOwner"); }
}
''')
adapter('content/global/skill/construction/HouseManager.java', r'''
package content.global.skill.construction;
import core.game.node.entity.player.Player;
public final class HouseManager {
    public static void leave(Player player) { oracle.R.v("HouseManager.leave", player); }
}
''')
adapter('core/game/world/update/UpdateSequence.java', r'''
package core.game.world.update;
import core.game.node.entity.player.Player;
import java.util.ArrayList;
import java.util.List;
public final class UpdateSequence {
    private static final List<Player> PLAYERS = new ArrayList<Player>() {
        @Override public boolean remove(Object o) { return oracle.R.b("UpdateSequence.renderablePlayers.remove", o); }
    };
    public static List<Player> getRenderablePlayers() { return PLAYERS; }
}
''')
adapter('core/worker/ManagementEvents.java', r'''
package core.worker;
public final class ManagementEvents {
    public static void publish(proto.management.PlayerStatusUpdate event) { oracle.R.v("ManagementEvents.publish", "status", event.username, event.world, event.notifyFriendsOnly); }
    public static void publish(proto.management.ClanLeaveNotification event) { oracle.R.v("ManagementEvents.publish", "clanLeave", event.username, event.world, event.clanName); }
}
''')
adapter('proto/management/PlayerStatusUpdate.java', r'''
package proto.management;
/** The generated builder's shape; a null string is refused as the generated code refuses it. */
public final class PlayerStatusUpdate {
    public String username; public int world; public boolean notifyFriendsOnly;
    public static Builder newBuilder() { return new Builder(); }
    public static final class Builder {
        private final PlayerStatusUpdate value = new PlayerStatusUpdate();
        public Builder setUsername(String username) { if (username == null) throw new NullPointerException(); value.username = username; return this; }
        public Builder setWorld(int world) { value.world = world; return this; }
        public Builder setNotifyFriendsOnly(boolean only) { value.notifyFriendsOnly = only; return this; }
        public PlayerStatusUpdate build() { return value; }
    }
}
''')
adapter('proto/management/ClanLeaveNotification.java', r'''
package proto.management;
public final class ClanLeaveNotification {
    public String username, clanName; public int world;
    public static Builder newBuilder() { return new Builder(); }
    public static final class Builder {
        private final ClanLeaveNotification value = new ClanLeaveNotification();
        public Builder setUsername(String username) { if (username == null) throw new NullPointerException(); value.username = username; return this; }
        public Builder setWorld(int world) { value.world = world; return this; }
        public Builder setClanName(String clanName) { if (clanName == null) throw new NullPointerException(); value.clanName = clanName; return this; }
        public ClanLeaveNotification build() { return value; }
    }
}
''')

# ---------------------------------------------------------------- containers
adapter('core/game/container/Container.java', r'''
package core.game.container;
import core.game.node.item.Item;
import java.util.List;
import oracle.R;
/** One of a player's containers; its label is `player.inventory`, `player.equipment` or `player.bank`. */
public class Container {
    private final core.game.node.Node owner;
    private final String which;
    public Container(core.game.node.Node owner, String which) { this.owner = owner; this.which = which; }
    public String label() { return R.label(owner) + "." + which; }
    public Item get(int slot) { return R.itemAnswer("Container.get", label(), slot); }
    public boolean add(Item item) { return R.b(true, "Container.add", label(), item); }
    public Item replace(Item item, int slot) { R.v("Container.replace", label(), item, slot); return null; }
    public boolean remove(Item item) { return R.b(true, "Container.remove", label(), item); }
    public void clear() { R.v("Container.clear", label()); }
    public void refresh() { R.v("Container.refresh", label()); }
    public Item[] toArray() {
        Object a = R.ev("Container.toArray", label());
        List<?> l = a == null ? java.util.Collections.emptyList() : (List<?>) a;
        Item[] out = new Item[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = R.itemOf(l.get(i));
        return out;
    }
    public int capacity() { return R.i("Container.capacity", label()); }
    public int getAmount(int id) { return R.i("Container.getAmount", label(), id); }
    public int getAmount(Item item) { return R.i("Container.getAmountItem", label(), item); }
    public Item getItem(Item item) { return R.itemAnswer("Container.getItem", label(), item); }
    public boolean containsItem(Item item) { return R.b("Container.containsItem", label(), item); }
    public boolean contains(int id, int amount) { return R.b("Container.contains", label(), id, amount); }
    public boolean hasSpaceFor(Item item) { return R.b(true, "Container.hasSpaceFor", label(), item); }
    public int getMaximumAdd(Item item) { return R.i("Container.getMaximumAdd", label(), item); }
    public void copy(Container other) { R.v("Container.copy", label(), other.label()); }
    public void parse(org.json.simple.JSONArray json) { R.v("Container.parse", label(), json.toJSONString()); }
}
''')
adapter('core/game/container/impl/EquipmentContainer.java', r'''
package core.game.container.impl;
import core.game.container.Container;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import oracle.R;
public final class EquipmentContainer extends Container {
    /*EQUIPMENT_SLOTS*/
    public EquipmentContainer(Player player) { super(player, "equipment"); }
    public boolean add(Item item, boolean fire, boolean fromInventory) { return R.b(true, "EquipmentContainer.add3", label(), item, fire, fromInventory); }
    public boolean add(Item newItem, int inventorySlot, boolean fire, boolean fromInventory) { return R.b(true, "EquipmentContainer.add4", label(), newItem, inventorySlot, fire, fromInventory); }
}
''')

# ---------------------------------------------------------------- world
adapter('core/ServerConstants.kt', r'''
package core
import core.game.world.map.Location
class ServerConstants {
    companion object {
        @JvmField
        var BOT_DATA_PATH: String? = null
        @JvmField
        var SERVER_GE_NAME: String = ""
        @JvmField
        var MAX_PATHFIND_DISTANCE = 25
        @JvmField
        var START_LOCATION: Location = Location(2524, 5002, 0)
    }
}
''')
adapter('core/Server.kt', r'''
package core
object Server {
    @JvmStatic
    var running = false
}
''')
adapter('core/game/world/GameWorld.kt', r'''
package core.game.world
import core.api.LogoutListener
object GameWorld {
    @JvmStatic
    var settings: GameSettings? = null
    @JvmStatic
    var Pulser = PulseRunner()
    @JvmStatic
    val logoutListeners = ArrayList<LogoutListener>()
}
class GameSettings(var ge_announcement_limit: Int) {
    val worldId: Int get() = oracle.R.i("GameSettings.getWorldId")
}
''')
adapter('core/api/ContentAPI.kt', r'''
package core.api
import core.cache.def.impl.ItemDefinition
import core.game.node.entity.player.Player
import core.tools.Log
/** The three core.api functions the engine calls, and the two listener interfaces it names. */
fun log(origin: Class<*>, type: Log, message: String) { oracle.R.v("log", type, message) }
fun getWorldTicks(): Int = oracle.R.i("getWorldTicks")
fun itemDefinition(id: Int): ItemDefinition = ItemDefinition.forId(id)
interface TickListener { fun tick() }
interface LogoutListener { fun logout(player: Player) }
''')
adapter('core/tools/Log.java', 'package core.tools;\npublic enum Log { /*LOG_CONSTANTS*/ }\n')
adapter('core/tools/RandomFunction.java', r'''
package core.tools;
import oracle.R;
/** Every draw is a recorded call answered by the case. */
public class RandomFunction {
    public static int random(int maxValue) { return R.i("RandomFunction.random", maxValue); }
    public static int random(int a, int b) { return R.i("RandomFunction.random2", a, b); }
    public static double random(double a, double b) { return R.d("RandomFunction.randomDouble", a, b); }
    public static int getRandom(int maxValue) { return R.i("RandomFunction.getRandom", maxValue); }
    public static boolean roll(int chance) { return R.b("RandomFunction.roll", chance); }
    public static int linearDecreaseRand(int max) { return R.i("RandomFunction.linearDecreaseRand", max); }
    public static int normalRandDist(int max) { return R.i("RandomFunction.normalRandDist", max); }
}
''')
adapter('core/tools/StringUtils.java', r'''
package core.tools;
public final class StringUtils {
    /*FORMAT_DISPLAY_NAME*/
}
''')
adapter('core/game/bots/Random.kt', r'''
// Oracle-only stand-ins in the engine's own package: a class named Random and two random() extensions declared
// here are what the unchanged sources resolve `Random()` (their star import of java.util) and `.random()` (the
// standard library's default import) to. Their draws are recorded calls answered by the case.
package core.game.bots

class Random {
    fun nextInt(bound: Int): Int = oracle.R.i("Random.nextInt", bound)
    fun nextInt(): Int = oracle.R.i("Random.nextInt0")
}

fun <T> Array<out T>.random(): T {
    if (isEmpty()) throw NoSuchElementException("Array is empty.")
    return get(oracle.R.i("kotlin.Random.index", size))
}

fun <T> Collection<T>.random(): T {
    if (isEmpty()) throw NoSuchElementException("Collection is empty.")
    return elementAt(oracle.R.i("kotlin.Random.index", size))
}
''')
adapter('core/game/world/repository/Repository.java', r'''
package core.game.world.repository;
import core.game.node.entity.player.Player;
import java.util.HashMap;
import java.util.Map;
/** `players` is the unchanged NodeList. The disconnection queue keeps what Player.clear writes; it is state. */
public final class Repository {
    public static NodeList<Player> players = new NodeList<>(2000);
    private static final DisconnectionQueue QUEUE = new DisconnectionQueue();
    public static NodeList<Player> getPlayers() { return players; }
    public static void sendGrandExchangeNews(String text) { oracle.R.v("Repository.sendGrandExchangeNews", text); }
    public static DisconnectionQueue getDisconnectionQueue() { return QUEUE; }
    public static final class DisconnectionQueue {
        public final Map<String, Object[]> queue = new HashMap<>();
        public void remove(String name) { queue.remove(name); }
        public void add(Player player, boolean clear) { if (queue.get(player.getName()) == null) queue.put(player.getName(), new Object[] { player, clear }); }
    }
}
''')
adapter('core/game/world/map/RegionManager.java', r'''
package core.game.world.map;
import core.game.node.entity.npc.NPC;
import core.game.node.scenery.Scenery;
import java.util.List;
public final class RegionManager {
    public static List<NPC> getLocalNPCs(Location location, int distance) { return oracle.R.list("RegionManager.getLocalNPCs", location, distance); }
    public static Scenery getObject(int x, int y, int z) { return oracle.R.n("RegionManager.getObject", x, y, z); }
    public static Region forId(int regionId) { oracle.R.v("RegionManager.forId", regionId); return new Region(regionId); }
    public static Location getTeleportLocation(Location location, int radius) { return location; }
    public static int getClippingFlag(int z, int x, int y) { throw new IllegalStateException("not part of this corpus"); }
}
''')
adapter('core/game/world/map/Region.java', r'''
package core.game.world.map;
import core.game.node.Node;
import core.game.node.scenery.Scenery;
import java.util.ArrayList;
public class Region {
    public final int id;
    public Region(int id) { this.id = id; }
    public ArrayList<Scenery> assembleObjectList(int z) { return new ArrayList<Scenery>(oracle.R.<Scenery>list("Region.assembleObjectList", id, z)); }
    public ArrayList<Node> assembleNodeList(int z) { return new ArrayList<Node>(oracle.R.<Node>list("Region.assembleNodeList", id, z)); }
    public RegionChunk[][][] getChunks() { return null; }
}
''')
adapter('core/game/world/map/RegionChunk.java', 'package core.game.world.map;\npublic class RegionChunk {}\n')
adapter('core/game/world/map/MapDistance.java', r'''
package core.game.world.map;
public enum MapDistance {
    RENDERING(15);
    private final int distance;
    MapDistance(int distance) { this.distance = distance; }
    public int getDistance() { return distance; }
}
''')
adapter('core/game/world/map/Point.java', r'''
package core.game.world.map;
public class Point {
    private final int x, y;
    public Point(int x, int y) { this.x = x; this.y = y; }
    public int getX() { return x; }
    public int getY() { return y; }
}
''')
adapter('core/game/world/map/path/ClipMaskSupplier.java', 'package core.game.world.map.path;\npublic interface ClipMaskSupplier { int getClippingFlag(int z, int x, int y); }\n')
adapter('core/game/world/map/path/Pathfinder.java', r'''
package core.game.world.map.path;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
public class Pathfinder implements oracle.R.Describable {
    public static final Pathfinder SMART = new Pathfinder("SMART");
    public static final Pathfinder DUMB = new Pathfinder("DUMB");
    final String name;
    Pathfinder(String name) { this.name = name; }
    public Object describe() { return name; }
    public static Path find(Entity mover, Node destination) { return new Path(oracle.R.ev("Pathfinder.find", mover, destination)); }
    public static Path find(Entity mover, Node destination, boolean near, Pathfinder finder) { return new Path(oracle.R.ev("Pathfinder.find", mover, destination, near, finder)); }
    public static Path find(Location start, Node destination, boolean near, Pathfinder finder) { throw new IllegalStateException("not part of this corpus"); }
}
''')
adapter('core/game/world/map/path/Path.java', r'''
package core.game.world.map.path;
import core.game.world.map.Point;
import java.util.ArrayDeque;
import java.util.Deque;
/** The answer of a find is whether the path only moves near; the path is labelled in the order paths are found. */
public class Path {
    private final boolean moveNear;
    public Path(Object answer) { this.moveNear = Boolean.TRUE.equals(answer); oracle.R.fresh(this, "path"); }
    public boolean isMoveNear() { return moveNear; }
    public boolean isSuccessful() { return true; }
    public Deque<Point> getPoints() { return new ArrayDeque<>(); }
    public void walk(core.game.node.entity.Entity entity) { oracle.R.v("Path.walk", this, entity); }
}
''')
adapter('core/game/world/map/zone/impl/WildernessZone.java', r'''
package core.game.world.map.zone.impl;
import core.game.node.entity.Entity;
public final class WildernessZone {
    public static int getWilderness(Entity e) { return oracle.R.i("WildernessZone.getWilderness", e); }
}
''')
adapter('core/net/packet/context/MessageContext.java', 'package core.net.packet.context;\npublic class MessageContext {}\n')
adapter('content/region/misc/tutisland/handlers/iface/CharacterDesign.java', r'''
package content.region.misc.tutisland.handlers.iface;
import core.game.node.entity.player.Player;
public final class CharacterDesign {
    public static void randomize(Player player, boolean head) { oracle.R.v("CharacterDesign.randomize", player, head); }
}
''')
adapter('core/game/world/update/flag/EntityFlag.java', 'package core.game.world.update.flag;\npublic enum EntityFlag { Chat, Animate, FaceEntity }\n')
adapter('core/game/world/update/flag/context/Animation.java', 'package core.game.world.update.flag.context;\npublic class Animation { public final int id; public Animation(int id) { this.id = id; } }\n')
adapter('core/game/world/update/flag/context/Graphics.java', 'package core.game.world.update.flag.context;\npublic class Graphics { public final int id; public Graphics(int id) { this.id = id; } }\n')
adapter('core/game/world/update/flag/context/ChatMessage.java', r'''
package core.game.world.update.flag.context;
import core.game.node.entity.player.Player;
public class ChatMessage implements oracle.R.Describable {
    private final String text;
    public ChatMessage(Player player, String text, int effects, int numChars) { this.text = text; }
    public Object describe() { return text; }
}
''')
adapter('core/game/ge/GrandExchange.kt', r'''
package core.game.ge
class GrandExchange {
    companion object {
        fun addBotOffer(itemID: Int, amount: Int): Boolean = oracle.R.b("GrandExchange.addBotOffer", itemID, amount)
    }
}
''')
adapter('core/game/ge/GrandExchangeOffer.kt', r'''
package core.game.ge
import core.game.node.entity.player.Player
/** The fields ScriptAPI.buyFromGE sets and reads; plain state. */
class GrandExchangeOffer {
    var itemID = 0
    var sell = false
    var offeredValue = 0
    var amount = 0
    var completedAmount = 0
    var player: Player? = null
}
''')
adapter('kotlinx/coroutines/Coroutines.kt', r'''
// Oracle-only stand-in for the two kotlinx.coroutines symbols ScriptAPI.kt imports. The pinned compiler artifacts do
// not include kotlinx-coroutines. A launched block is recorded and kept; the case runs it later on the calling
// thread, with a hook that runs the pulse its latch waits for.
package kotlinx.coroutines

import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine

interface CoroutineScope

object GlobalScope : CoroutineScope

fun CoroutineScope.launch(block: suspend CoroutineScope.() -> Unit) {
    val scope = this
    oracle.R.launch(Runnable {
        var outcome: Result<Unit>? = null
        block.startCoroutine(scope, Continuation(EmptyCoroutineContext) { outcome = it })
        val finished = outcome ?: throw IllegalStateException("oracle coroutine adapter cannot suspend")
        finished.getOrThrow()
    })
}
''')

# ---------------------------------------------------------------- consumables
adapter('core/game/consumable/Consumable.java', r'''
package core.game.consumable;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
public abstract class Consumable implements oracle.R.Describable {
    public void consume(Item item, Player player) { oracle.R.v("Consumable.consume", this, item, player); }
}
''')
adapter('core/game/consumable/ConsumableEffect.java', 'package core.game.consumable;\npublic abstract class ConsumableEffect {}\n')
adapter('core/game/consumable/Food.java', r'''
package core.game.consumable;
import java.util.Map;
import java.util.TreeMap;
public class Food extends Consumable {
    private final int[] ids;
    private final ConsumableEffect effect;
    public Food(int[] ids, ConsumableEffect effect) { this.ids = ids; this.effect = effect; }
    public Object describe() {
        Map<String, Object> m = new TreeMap<>();
        m.put("food", ids);
        m.put("heal", ((content.data.consumables.effects.HealingEffect) effect).amount);
        return m;
    }
}
''')
adapter('content/data/consumables/effects/HealingEffect.java', r'''
package content.data.consumables.effects;
public class HealingEffect extends core.game.consumable.ConsumableEffect {
    public final int amount;
    public HealingEffect(int amount) { this.amount = amount; }
}
''')
adapter('content/data/consumables/Consumables.java', r'''
package content.data.consumables;
import core.game.consumable.Consumable;
/** getConsumableById answers a constant (the case's label for it) or null. */
public final class Consumables {
    private final Consumable consumable;
    private Consumables(String label) { this.consumable = new Consumable() { public Object describe() { return label; } }; }
    public Consumable getConsumable() { return consumable; }
    public static Consumables getConsumableById(int itemId) {
        Object a = oracle.R.ev("Consumables.getConsumableById", itemId);
        return a == null ? null : new Consumables((String) a);
    }
}
''')

# ---------------------------------------------------------------- content the engine names
adapter('content/global/bots/Bots.kt', r'''
package content.global.bots
import core.game.bots.Script
/** The two content scripts the engine tests for by class. Their bodies are recorded calls. */
class Idler : Script() {
    override fun tick() { oracle.R.v("Script.tick", oracle.Support.scriptLabel(this)) }
    override fun newInstance(): Script = oracle.Support.newInstance(this)
}
class Adventurer : Script() {
    enum class State { EXPLORE, TELEPORTING }
    var state = State.EXPLORE
    override fun tick() { oracle.R.v("Script.tick", oracle.Support.scriptLabel(this)) }
    override fun newInstance(): Script = oracle.Support.newInstance(this)
}
''')
adapter('oracle/CaseScript.java', r'''
package oracle;
import core.game.bots.Script;
/** An ordinary content script. */
public class CaseScript extends Script {
    @Override public void tick() { R.v("Script.tick", Support.scriptLabel(this)); }
    @Override public Script newInstance() { return Support.newInstance(this); }
}
''')
adapter('content/minigame/pestcontrol/bots/PestControlBots.kt', r'''
package content.minigame.pestcontrol.bots
import core.game.world.map.Location
/** Construction is the recorded call; the classes themselves are content. */
class PestControlTestBot(l: Location) { init { oracle.R.v("PestControlTestBot.new", false, l) } }
class PestControlTestBot2(l: Location) { init { oracle.R.v("PestControlTestBot.new", true, l) } }
''')
