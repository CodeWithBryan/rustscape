"""Recording stand-ins for every class the unmodified familiar framework reaches outside its own files.

Each stand-in method that the original calls records one event through oracle.Support.emit (named after the native
`world::familiars::Host` method) and answers from the case (oracle.Support.answer) or from the few fields a node keeps
(location, visibility, ids, attributes). Plain getters of fields (getLocation, getId, getOriginalId, getName,
isInvisible, getInventory, version, getFamiliarManager) are not recorded. `/*NAME*/` markers are filled with original
text at generation time.
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text


# ---------------------------------------------------------------- nodes and entities
adapter('core/game/node/Node.java', r'''package core.game.node;
import core.game.world.map.Location;
// The node fields the framework reads: key (the case's name), location and visibility.
public abstract class Node {
    public String key;
    protected Location location;
    protected boolean invisible;
    public Node(String name, Location location) { this.location = location; }
    public Location getLocation() { return location; }
    public void setLocationQuiet(Location l) { location = l; }
    public boolean isInvisible() { return invisible; }
    public void setInvisible(boolean invisible) { oracle.Support.emit("set_invisible", key, invisible); this.invisible = invisible; }
    public void setInvisibleQuiet(boolean invisible) { this.invisible = invisible; }
    public int size() { oracle.Support.emit("size", key); return oracle.Support.integer("size", key, 1); }
}
''')

adapter('core/game/node/entity/Entity.java', r'''package core.game.node.entity;
import core.game.node.Node;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.CombatPulse;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Graphics;
import oracle.Stand;
import oracle.Support;
import java.util.HashMap;
import java.util.Map;
// The entity members the framework calls; each records one event.
public abstract class Entity extends Node {
    public final Map<String, Object> attributes = new HashMap<>();
    private Stand.Properties properties;
    public Entity(String name, Location location) { super(name, location); }
    public Stand.Properties getProperties() { if (properties == null) properties = new Stand.Properties(this); return properties; }
    public Stand.PulseManager getPulseManager() { return new Stand.PulseManager(this); }
    public Stand.ZoneMonitor getZoneMonitor() { return new Stand.ZoneMonitor(this); }
    public Stand.Locks getLocks() { return new Stand.Locks(this); }
    public Stand.WalkingQueue getWalkingQueue() { return new Stand.WalkingQueue(this); }
    public Stand.UpdateMasks getUpdateMasks() { return new Stand.UpdateMasks(this); }
    public Stand.ImpactHandler getImpactHandler() { return new Stand.ImpactHandler(this); }
    public boolean isAttackable(Entity entity, CombatStyle style, boolean message) {
        Support.emit("is_attackable", key, Support.key(entity), style.name(), message);
        return Support.bool("is_attackable", key, true);
    }
    public boolean inCombat() { Support.emit("in_combat", key); return Support.bool("in_combat", key, false); }
    public void face(Entity entity) { Support.emit("face", key, entity == null ? null : Support.key(entity)); }
    public void sendChat(String text) { Support.emit("send_chat", key, text); }
    public void graphics(Graphics g) { Support.emit("graphics", key, g.getId()); }
    public boolean isRenderable() { Support.emit("is_renderable", key); return Support.bool("is_renderable", key, true); }
    public void init() { Support.emit("entity_init", key); }
    public void handleTickActions() {}
    public boolean continueAttack(Entity target, CombatStyle style, boolean message) { return true; }
    public void onRegionInactivity() {}
    public CombatSwingHandler getSwingHandler(boolean swing) { throw new IllegalStateException("entity swing handler"); }
    public void finalizeDeath(Entity killer) {}
    public boolean canAttack(Entity target) { return true; }
    public boolean isPoisonImmune() { return false; }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String name, T fail) {
        Support.emit("get_attribute", key, name, Support.valueOf(fail));
        Object v = attributes.get(name);
        return v == null ? fail : (T) v;
    }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String name) {
        Support.emit("get_attribute", key, name);
        return (T) attributes.get(name);
    }
    public void setAttribute(String name, Object value) {
        Support.emit("set_attribute", key, name, Support.valueOf(value));
        attributes.put(name, value);
    }
}
''')

adapter('core/game/node/entity/npc/NPC.java', r'''package core.game.node.entity.npc;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.world.map.Location;
import oracle.Support;
// NPC(id, location): records new_npc and takes the next npc:N key; the name is the definition's (unrecorded).
public class NPC extends Entity {
    private int id;
    private final int originalId;
    private final String name;
    public NPC(int id, Location location) {
        super(null, location);
        Support.emit("new_npc", id);
        if (!Support.quiet) { key = "npc:" + Support.npcKey(); Support.nodes.put(key, this); }
        this.id = id;
        this.originalId = id;
        this.name = Support.npcName(id);
    }
    public int getId() { return id; }
    public int getOriginalId() { return originalId; }
    public String getName() { return name; }
    @Override public void init() { Support.emit("npc_init", key, Support.coord(location)); }
    public void clear() { Support.emit("npc_clear", key); }
    public void transform(int id) { Support.emit("npc_transform", key, id); this.id = id; }
    public oracle.Stand.Task getTask() {
        Support.emit("npc_task", key);
        Object level = Support.answer("npc_task", key, null);
        return level == null ? null : new oracle.Stand.Task((Integer) level);
    }
    @Override public boolean isAttackable(Entity entity, CombatStyle style, boolean message) {
        Support.emit("npc_is_attackable", key, Support.key(entity), style.name(), message);
        return Support.bool("npc_is_attackable", key, true);
    }
    @Override public CombatSwingHandler getSwingHandler(boolean swing) {
        Support.emit("npc_swing_handler", key, swing);
        return new CombatSwingHandler("handler:" + key, null);
    }
}
''')

adapter('core/game/node/entity/player/Player.java', r'''package core.game.node.entity.player;
import content.global.skill.summoning.familiar.FamiliarManager;
import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.CombatStyle;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import oracle.Stand;
import oracle.Support;
// The player members the framework calls. The inventory is the container "inv"; the familiar manager is real.
public class Player extends Entity {
    public int version;
    private final Container inventory;
    private FamiliarManager familiarManager;
    public Player(String key) {
        super(key, null);
        this.key = key;
        this.inventory = new Container(key + ".inv", 28);
    }
    public FamiliarManager getFamiliarManager() { if (familiarManager == null) familiarManager = new FamiliarManager(this); return familiarManager; }
    public Container getInventory() { return inventory; }
    public Stand.Skills getSkills() { return new Stand.Skills(this); }
    public Stand.PacketDispatch getPacketDispatch() { return new Stand.PacketDispatch(this); }
    public Stand.InterfaceManager getInterfaceManager() { return new Stand.InterfaceManager(this); }
    public Stand.DialogueInterpreter getDialogueInterpreter() { return new Stand.DialogueInterpreter(this); }
    public Stand.Appearance getAppearance() { return new Stand.Appearance(this); }
    public Stand.QuestRepository getQuestRepository() { return new Stand.QuestRepository(this); }
    public Stand.SkullManager getSkullManager() { return new Stand.SkullManager(this); }
    public void animate(Animation a) { Support.emit("animate", key, a.getId()); }
    public void visualize(Animation a, Graphics g) { Support.emit("visualize", key, a.getId(), g.getId()); }
    public boolean isActive() { Support.emit("is_active", key); return Support.bool("is_active", key, true); }
    public void sendMessage(String text) { Support.emit("player_message", key, text); }
    public void incrementAttribute(String name) {
        Support.emit("increment_attribute", key, name);
        Object v = attributes.get(name);
        attributes.put(name, (v == null ? 0 : (Integer) v) + 1);
    }
}
''')

adapter('oracle/Stand.java', r'''package oracle;
import content.data.Quests;
import core.game.component.Component;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.combat.CombatPulse;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.player.Player;
import core.game.node.entity.impl.PulseType;
import core.game.system.task.Pulse;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneRestriction;
// The managers an entity hands out; each method records one event.
public class Stand {
    public static class Task { public final int levelReq; public Task(int levelReq) { this.levelReq = levelReq; } }
    public static class Properties {
        final Entity e; CombatPulse combat;
        public Properties(Entity e) { this.e = e; }
        public CombatPulse getCombatPulse() { if (combat == null) combat = new CombatPulse(e); return combat; }
        public boolean isMultiZone() { Support.emit("multi_zone", e.key); return Support.bool("multi_zone", e.key, false); }
        public void setTeleportLocation(Location l) { Support.emit("set_teleport_location", e.key, Support.coord(l)); }
    }
    public static class PulseManager {
        final Entity e; public PulseManager(Entity e) { this.e = e; }
        public void run(Pulse p, PulseType type) {
            if (!(p instanceof core.game.interaction.MovementPulse)) throw new IllegalStateException("pulse kind");
            core.game.interaction.MovementPulse m = (core.game.interaction.MovementPulse) p;
            Support.emit("pulse_run", e.key, "follow", Support.key(m.mover), m.destination == null ? null : Support.key(m.destination), type.name());
            Support.pulses.add(p);
        }
        public void clear() { Support.emit("pulse_clear", e.key); }
        public boolean hasPulseRunning() { Support.emit("has_pulse_running", e.key); return Support.bool("has_pulse_running", e.key, false); }
    }
    public static class ZoneMonitor {
        final Entity e; public ZoneMonitor(Entity e) { this.e = e; }
        public boolean isInZone(String name) { Support.emit("in_zone", e.key, name); return Support.bool("in_zone", e.key, false); }
        public boolean isRestricted(ZoneRestriction r) {
            if (r != ZoneRestriction.FOLLOWERS) throw new IllegalStateException("restriction");
            Support.emit("restricted_followers", e.key); return Support.bool("restricted_followers", e.key, false);
        }
    }
    public static class Locks {
        final Entity e; public Locks(Entity e) { this.e = e; }
        public boolean isLocked(String k) { Support.emit("is_locked", e.key, k); return Support.bool("is_locked", e.key, false); }
        public boolean isInteractionLocked() { Support.emit("interaction_locked", e.key); return Support.bool("interaction_locked", e.key, false); }
    }
    public static class WalkingQueue { final Entity e; public WalkingQueue(Entity e) { this.e = e; } public void update() { Support.emit("walking_update", e.key); } }
    public static class UpdateMasks {
        final Entity e; public UpdateMasks(Entity e) { this.e = e; }
        public void prepare(Entity x) { if (x != e) throw new IllegalStateException("masks"); Support.emit("masks_prepare", e.key); }
    }
    public static class ImpactHandler {
        final Entity e; public ImpactHandler(Entity e) { this.e = e; }
        public void handleImpact(Entity source, int hit, CombatStyle style, BattleState state) {
            Support.emit("handle_impact", e.key, Support.key(source), hit, style.name(), state.key);
        }
    }
    public static class Skills {
        final Player p; public Skills(Player p) { this.p = p; }
        public int updateLevel(int skill, int amount, int maximum) { Support.emit("update_level", p.key, skill, amount, maximum); return 0; }
        public int getStaticLevel(int skill) { Support.emit("static_level", p.key, skill); return Support.integer("static_level", p.key, 99); }
        public int getLevel(int skill) { Support.emit("skill_level", p.key, skill); return Support.integer("skill_level", p.key, 99); }
        public void addExperience(int skill, double experience) { Support.emit("add_experience", p.key, skill, Support.bits(experience), null); }
        public void addExperience(int skill, double experience, boolean playerMod) { Support.emit("add_experience", p.key, skill, Support.bits(experience), playerMod); }
    }
    public static class PacketDispatch { final Player p; public PacketDispatch(Player p) { this.p = p; } public void sendMessage(String text) { Support.emit("packet_message", p.key, text); } }
    public static class DialogueInterpreter {
        final Player p; public DialogueInterpreter(Player p) { this.p = p; }
        public Component sendDialogue(String... messages) {
            if (messages.length != 1) throw new IllegalStateException("dialogue lines");
            Support.emit("send_dialogue", p.key, messages[0]); return null;
        }
    }
    public static class Appearance { final Player p; public Appearance(Player p) { this.p = p; } public void sync() { Support.emit("appearance_sync", p.key); } }
    public static class QuestRepository {
        final Player p; public QuestRepository(Player p) { this.p = p; }
        public boolean isComplete(Quests q) {
            if (q != Quests.WOLF_WHISTLE) throw new IllegalStateException("quest");
            Support.emit("wolf_whistle_complete", p.key); return Support.bool("wolf_whistle_complete", p.key, false);
        }
    }
    public static class SkullManager { final Player p; public SkullManager(Player p) { this.p = p; } public boolean isWilderness() { Support.emit("skull_wilderness", p.key); return Support.bool("skull_wilderness", p.key, false); } }
    public static class InterfaceManager {
        final Player p; public InterfaceManager(Player p) { this.p = p; }
        public void openTab(Component c) { Support.emit("open_tab", p.key, c.getId()); }
        public void setViewedTab(int tab) { Support.emit("set_viewed_tab", p.key, tab); }
        public void openInfoBars() { Support.emit("open_info_bars", p.key); }
        public void removeTabs(int... tabs) { java.util.List l = new java.util.ArrayList(); for (int t : tabs) l.add(t); Support.emit("remove_tabs", p.key, l); }
        public boolean hasMainComponent(int id) { Support.emit("has_main_component", p.key, id); return Support.bool("has_main_component", p.key, false); }
        public boolean close() { Support.emit("close", p.key); return true; }
        public Component open(Component c) {
            Support.emit("open", p.key, c.getId());
            if (!Support.bool("open", p.key, true)) return null;
            c.owner = p; return c;
        }
        public void openSingleTab(Component c) { Support.emit("open_single_tab", p.key, c.getId()); }
        public boolean closeSingleTab() { Support.emit("close_single_tab", p.key); return true; }
    }
}
''')

adapter('core/game/node/entity/combat/CombatPulse.java', r'''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.Support;
// An entity's combat pulse: each member records one event and answers from the case.
public class CombatPulse {
    final Entity e;
    public CombatPulse(Entity e) { this.e = e; }
    public boolean isAttacking() { Support.emit("combat_attacking", e.key); return Support.bool("combat_attacking", e.key, false); }
    public Entity getVictim() { Support.emit("combat_victim", e.key); return (Entity) Support.node(Support.answer("combat_victim", e.key, null)); }
    public void attack(Entity victim) { Support.emit("combat_attack", e.key, Support.key(victim)); }
    public void setNextAttack(int ticks) { Support.emit("combat_next_attack", e.key, ticks); }
    public CombatStyle getStyle() { Support.emit("combat_style", e.key); return CombatStyle.valueOf((String) Support.answer("combat_style", e.key, "MELEE")); }
}
''')

adapter('core/game/node/entity/combat/CombatStyle.java', r'''package core.game.node.entity.combat;
// The three styles; getSwingHandler answers a recording handler of the style.
public enum CombatStyle {
    MELEE, RANGE, MAGIC;
    public CombatSwingHandler getSwingHandler() { return new CombatSwingHandler("style:" + name(), this); }
}
''')

adapter('core/game/node/entity/combat/CombatSwingHandler.java', r'''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.Support;
// A handler object, named by its key; isAccurateImpact records the style's accuracy roll.
public class CombatSwingHandler {
    public final String key;
    final CombatStyle style;
    public CombatSwingHandler(String key, CombatStyle style) { this.key = key; this.style = style; }
    public boolean isAccurateImpact(Entity a, Entity b) {
        Support.emit("accurate_impact", style.name(), Support.key(a), Support.key(b));
        return Support.bool("accurate_impact", null, true);
    }
}
''')

adapter('core/game/node/entity/combat/BattleState.java', r'''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.Support;
// new BattleState(attacker, victim) records battle_state and takes the next state:N key.
public class BattleState {
    public final String key;
    public BattleState(Entity attacker, Entity victim) {
        Support.emit("battle_state", Support.key(attacker), Support.key(victim));
        key = "state:" + Support.stateKey();
    }
    public void setEstimatedHit(int hit) { Support.emit("set_estimated_hit", key, hit); }
    public void neutralizeHits() { Support.emit("neutralize_hits", key); }
    public void setPoisonDamage(int damage) { Support.emit("set_poison_damage", key, damage); }
    public CombatStyle getStyle() { Support.emit("state_style", key); Object s = Support.answer("state_style", key, "MELEE"); return s == null ? null : CombatStyle.valueOf((String) s); }
}
''')

adapter('core/game/node/entity/combat/equipment/WeaponInterface.java', r'''package core.game.node.entity.combat.equipment;
// The original constant.
public class WeaponInterface {
    /*STYLE_DEFENSIVE*/
}
''')

adapter('core/game/node/entity/impl/Projectile.java', r'''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import oracle.Support;
// Projectile.magic(...).send() records one projectile event at send.
public class Projectile {
    final Entity s, t; final int id, a, b, c, d;
    Projectile(Entity s, Entity t, int id, int a, int b, int c, int d) { this.s = s; this.t = t; this.id = id; this.a = a; this.b = b; this.c = c; this.d = d; }
    public static Projectile magic(Entity s, Entity t, int id, int a, int b, int c, int d) { return new Projectile(s, t, id, a, b, c, d); }
    public void send() { Support.emit("projectile", Support.key(s), Support.key(t), id, a, b, c, d); }
}
''')

adapter('core/game/node/entity/impl/PulseType.java', r'''package core.game.node.entity.impl;
public enum PulseType { STANDARD, STRONG }
''')

adapter('core/game/node/entity/skill/SkillBonus.java', r'''package core.game.node.entity.skill;
// A skill id and its bonus.
public class SkillBonus {
    private final int skillId; private final double bonus;
    public SkillBonus(int skillId, double bonus) { this.skillId = skillId; this.bonus = bonus; }
    public int getSkillId() { return skillId; }
    public double getBonus() { return bonus; }
}
''')

adapter('core/game/node/entity/skill/Skills.java', r'''package core.game.node.entity.skill;
// The original skill ids.
public class Skills {
    /*SKILL_IDS*/
}
''')

adapter('core/game/node/item/Item.java', r'''package core.game.node.item;
import core.cache.def.impl.ItemDefinition;
import core.game.node.Node;
import oracle.Support;
// An item value (id, amount, slot); getValue and getName record one event each.
public class Item extends Node {
    private final int id; private final int amount; private int slot = -1;
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { super(null, null); this.id = id; this.amount = amount; }
    public int getId() { return id; }
    public int getAmount() { return amount; }
    public int getSlot() { return slot; }
    public void setSlot(int s) { slot = s; }
    public ItemDefinition getDefinition() { return new ItemDefinition(id); }
    public int getValue() { Support.emit("item_value", Support.item(this)); return Support.integer("item_value:" + id, null, ((Number) Support.itemSpec(id).getOrDefault("value", 1L)).intValue()); }
    public String getName() { Support.emit("item_name", Support.item(this)); return (String) Support.itemSpec(id).getOrDefault("name", "null"); }
}
''')

adapter('core/game/node/item/GroundItem.java', r'''package core.game.node.item;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
public class GroundItem {
    public final Item item; public final Location location; public final int decay; public final Player owner;
    public GroundItem(Item item, Location location, int decay, Player owner) { this.item = item; this.location = location; this.decay = decay; this.owner = owner; }
}
''')

adapter('core/game/node/item/GroundItemManager.java', r'''package core.game.node.item;
import oracle.Support;
public class GroundItemManager {
    public static GroundItem create(GroundItem g) {
        Support.emit("ground_item", Support.item(g.item), Support.coord(g.location), g.decay, Support.key(g.owner)); return g;
    }
}
''')

adapter('core/cache/def/impl/ItemDefinition.java', r'''package core.cache.def.impl;
import oracle.Support;
// An item's definition answers from the case's items table; each read records one event.
public class ItemDefinition {
    final int id;
    public ItemDefinition(int id) { this.id = id; }
    public static ItemDefinition forId(int id) { return new ItemDefinition(id); }
    java.util.Map spec() { return Support.itemSpec(id); }
    public String getName() { Support.emit("item_definition_name", id); return (String) spec().getOrDefault("name", "null"); }
    public boolean isStackable() { Support.emit("item_stackable", id); return Boolean.TRUE.equals(spec().getOrDefault("stackable", false)); }
    public boolean isTradeable() { Support.emit("item_tradeable", id); return Boolean.TRUE.equals(spec().getOrDefault("tradeable", true)); }
    @SuppressWarnings("unchecked")
    public <T> T getConfiguration(String key, T fallback) {
        if (!"bankable".equals(key) || !Boolean.TRUE.equals(fallback)) throw new IllegalStateException("configuration " + key);
        Support.emit("item_bankable", id); return (T) spec().getOrDefault("bankable", true);
    }
}
''')

adapter('core/cache/def/impl/NPCDefinition.java', r'''package core.cache.def.impl;
import oracle.Support;
// forId(id).getName(): one npc_definition_name event; the name is the case's (or "null").
public class NPCDefinition {
    final int id;
    NPCDefinition(int id) { this.id = id; }
    public static NPCDefinition forId(int id) { return new NPCDefinition(id); }
    public String getName() { Support.emit("npc_definition_name", id); return Support.npcName(id); }
}
''')

# ---------------------------------------------------------------- containers
adapter('core/game/container/Container.java', r'''package core.game.container;
import core.game.node.item.Item;
import oracle.Support;
import java.util.List;
// A container named by its key; every member records one event and answers from the case.
public class Container {
    public final String key;
    public Container(int size) {
        Support.emit("new_container", size);
        key = "container:" + Support.containerKey();
    }
    public Container(String key, int size) { this.key = key; }
    public Container register(ContainerListener l) {
        Object owner = null;
        try { java.lang.reflect.Field f = l.getClass().getDeclaredField("player"); f.setAccessible(true); owner = f.get(l); }
        catch (ReflectiveOperationException e) { throw new IllegalStateException(e); }
        Support.emit("register_burden_listener", key, owner == null ? null : Support.key(owner));
        return this;
    }
    public int getAmount(Item item) { Support.emit("get_amount", key, Support.item(item)); return Support.integer("get_amount", key, 0); }
    public int getMaximumAdd(Item item) { Support.emit("maximum_add", key, Support.item(item)); return Support.integer("maximum_add", key, 0); }
    public Item replace(Item item, int slot) { Support.emit("replace", key, Support.item(item), slot, null); return Support.itemOf(Support.answer("replace", key, null)); }
    public Item replace(Item item, int slot, boolean fire) { Support.emit("replace", key, Support.item(item), slot, fire); return Support.itemOf(Support.answer("replace", key, null)); }
    public boolean add(Item item) { Support.emit("add", key, Support.item(item), null); return Support.bool("add", key, true); }
    public boolean add(Item item, boolean fire) { Support.emit("add", key, Support.item(item), fire); return Support.bool("add", key, true); }
    public boolean remove(Item item) { Support.emit("remove", key, Support.item(item), null); return Support.bool("remove", key, true); }
    public boolean remove(Item item, boolean fire) { Support.emit("remove", key, Support.item(item), fire); return Support.bool("remove", key, true); }
    public Item[] toArray() {
        Support.emit("to_array", key);
        List l = (List) Support.answer("to_array", key, new java.util.ArrayList());
        Item[] out = new Item[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = Support.itemOf(l.get(i));
        return out;
    }
    public Item get(int slot) { Support.emit("get", key, slot); return Support.itemOf(Support.answer("get", key, null)); }
    public int capacity() { Support.emit("capacity", key); return Support.integer("capacity", key, 0); }
    public int itemCount() { Support.emit("item_count", key); return Support.integer("item_count", key, 0); }
    public boolean isEmpty() { Support.emit("is_empty", key); return Support.bool("is_empty", key, true); }
    public void clear() { Support.emit("clear", key); }
    public void shift() { Support.emit("shift", key); }
    public void refresh() { Support.emit("refresh", key); }
    public void update() { Support.emit("update", key); }
    public int freeSlots() { Support.emit("free_slots", key); return Support.integer("free_slots", key, 28); }
    public boolean hasSpaceFor(Item item) { Support.emit("has_space_for", key, Support.item(item)); return Support.bool("has_space_for", key, true); }
    public boolean contains(int id, int amount) { Support.emit("contains", key, id, amount); return Support.bool("contains", key, true); }
    public boolean containsAtLeastOneItem(int id) { Support.emit("contains_one", key, id); return Support.bool("contains_one:" + id, key, false); }
    public void parse(org.json.simple.JSONArray data) { Support.emit("parse_container", key, data.toJSONString()); }
}
''')

adapter('core/game/container/ContainerEvent.java', r'''package core.game.container;
import core.game.node.item.Item;
public class ContainerEvent {
    final Item[] items; final int[] slots;
    public ContainerEvent(Item[] items, int[] slots) { this.items = items; this.slots = slots; }
    public Item[] getItems() { return items; }
    public int[] getSlots() { return slots; }
}
''')

adapter('core/game/container/ContainerListener.java', r'''package core.game.container;
public interface ContainerListener {
    void update(Container c, ContainerEvent event);
    void refresh(Container c);
}
''')

adapter('core/game/container/access/InterfaceContainer.java', r'''package core.game.container.access;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import oracle.Support;
import java.util.Arrays;
public class InterfaceContainer {
    public static int generateItems(Player player, Item[] items, String[] options, int interfaceId, int childId, int x, int y, int key) {
        Support.emit("generate_items", player.key, Support.items(items), Arrays.asList(options), interfaceId, childId, x, y, key);
        return 0;
    }
}
''')

adapter('core/net/packet/PacketRepository.java', r'''package core.net.packet;
import core.net.packet.context.ContainerContext;
import oracle.Support;
public class PacketRepository {
    public static void send(Class<?> type, Object context) {
        if (type != core.net.packet.out.ContainerPacket.class) throw new IllegalStateException("packet");
        ContainerContext c = (ContainerContext) context;
        if (c.interfaceId != -1 || c.childId != -2 || c.containerId != 30 || c.split) throw new IllegalStateException("context");
        java.util.List slots = null;
        if (c.slots != null) { slots = new java.util.ArrayList(); for (int s : c.slots) slots.add(s); }
        Support.emit("container_packet", c.player.key, Support.items(c.items), c.length, slots);
    }
}
''')

adapter('core/net/packet/context/ContainerContext.java', r'''package core.net.packet.context;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
// The two constructors BurdenContainerListener uses.
public class ContainerContext {
    public final Player player; public final int interfaceId, childId, containerId; public final Item[] items;
    public final Integer length; public final boolean split; public final int[] slots;
    public ContainerContext(Player player, int interfaceId, int childId, int containerId, Item[] items, int length, boolean split) {
        this.player = player; this.interfaceId = interfaceId; this.childId = childId; this.containerId = containerId; this.items = items; this.length = length; this.split = split; this.slots = null;
    }
    public ContainerContext(Player player, int interfaceId, int childId, int containerId, Item[] items, boolean split, int... slots) {
        this.player = player; this.interfaceId = interfaceId; this.childId = childId; this.containerId = containerId; this.items = items; this.length = null; this.split = split; this.slots = slots;
    }
}
''')

adapter('core/net/packet/out/ContainerPacket.java', 'package core.net.packet.out;\npublic class ContainerPacket {}\n')

# ---------------------------------------------------------------- interfaces, world, tools
adapter('core/game/component/Component.java', r'''package core.game.component;
import core.game.node.entity.player.Player;
import oracle.Support;
// new Component(id); setCloseEvent records and keeps the event for the case's closeEvent operation.
public class Component {
    final int id; public Player owner;
    public Component(int id) { this.id = id; }
    public int getId() { return id; }
    public void setCloseEvent(CloseEvent event) {
        Support.emit("set_close_event", owner.key, id);
        Support.closeEvents.add(new Object[] { event, this });
    }
}
''')

adapter('core/game/interaction/MovementPulse.java', r'''package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.system.task.Pulse;
import core.game.world.map.path.Pathfinder;
// The mover and destination of a movement pulse; the movement itself is not part of the framework.
public abstract class MovementPulse extends Pulse {
    public final Entity mover; public final Node destination;
    public MovementPulse(Entity mover, Node destination, Pathfinder finder) {
        super(1);
        if (finder != Pathfinder.DUMB) throw new IllegalStateException("finder");
        this.mover = mover; this.destination = destination;
    }
}
''')

adapter('core/game/system/task/Pulse.java', r'''package core.game.system.task;
import core.game.node.Node;
// A pulse's delay and checked nodes; GameWorld.getPulser().submit records them.
public abstract class Pulse {
    public final int delay; public final Node[] checks;
    public Pulse(int delay, Node... checks) { this.delay = delay; this.checks = checks; }
    public abstract boolean pulse();
}
''')

adapter('core/game/world/GameWorld.java', r'''package core.game.world;
import core.game.system.task.Pulse;
import oracle.Support;
import java.util.ArrayList;
import java.util.List;
public class GameWorld {
    public static int getTicks() { Support.emit("ticks"); return Support.integer("ticks", null, 100); }
    public static Pulser getPulser() { return new Pulser(); }
    public static class Pulser {
        public void submit(Pulse p) {
            List keys = new ArrayList(); for (core.game.node.Node n : p.checks) keys.add(Support.key(n));
            Support.emit("submit_pulse", p.delay, keys);
            Support.pulses.add(p);
        }
    }
}
''')

adapter('core/game/world/map/Location.java', r'''package core.game.world.map;
// x, y, plane; getDistance is Location.getDistance(first, second) of Location.kt:
/*GET_DISTANCE*/
public class Location {
    final int x, y, z;
    public Location(int x, int y, int z) { this.x = x; this.y = y; this.z = z; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public int getX() { return x; }
    public int getY() { return y; }
    public int getZ() { return z; }
    public double getDistance(Location other) {
        int xDiff = x - other.x;
        int yDiff = y - other.y;
        return Math.sqrt((double) (xDiff * xDiff + yDiff * yDiff));
    }
}
''')

adapter('core/game/world/map/RegionManager.java', r'''package core.game.world.map;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import oracle.Support;
public class RegionManager {
    public static Location getSpawnLocation(Player owner, Node node) {
        Support.emit("spawn_location", Support.key(owner), Support.key(node));
        Object v = Support.answer("spawn_location", null, Support.coord(owner.getLocation()));
        return Support.location(v);
    }
}
''')

adapter('core/game/world/map/path/Pathfinder.java', r'''package core.game.world.map.path;
public class Pathfinder { public static final Pathfinder DUMB = new Pathfinder(); }
''')

adapter('core/game/world/map/zone/ZoneRestriction.java', r'''package core.game.world.map.zone;
public enum ZoneRestriction { FOLLOWERS }
''')

adapter('core/game/world/update/flag/context/Animation.java', r'''package core.game.world.update.flag.context;
public class Animation {
    final int id;
    public Animation(int id) { this.id = id; }
    public static Animation create(int id) { return new Animation(id); }
    public int getId() { return id; }
}
''')

adapter('core/game/world/update/flag/context/Graphics.java', r'''package core.game.world.update.flag.context;
public class Graphics {
    final int id;
    Graphics(int id) { this.id = id; }
    public static Graphics create(int id) { return new Graphics(id); }
    public int getId() { return id; }
}
''')

adapter('core/tools/Log.java', r'''package core.tools;
// The original constants (SystemLogger.kt).
public enum Log { /*LOG_CONSTANTS*/ }
''')

adapter('core/tools/RandomFunction.java', r'''package core.tools;
import oracle.Support;
// The draws the framework makes, each answered by the case.
public class RandomFunction {
    public static int random(int max) { Support.emit("random", max); return Support.integer("random", null, 0); }
    public static int random(int a, int b) { Support.emit("random_range", a, b); return Support.integer("random_range", null, a); }
    public static int randomize(int value) { Support.emit("randomize", value); return Support.integer("randomize", null, value); }
}
''')

adapter('core/game/system/config/ItemConfigParser.java', r'''package core.game.system.config;
// The original constant (a const val of ItemConfigParser.kt's companion).
public class ItemConfigParser { /*BANKABLE*/ }
''')

adapter('core/api/ContentAPIKt.java', r'''package core.api;
import core.game.node.entity.player.Player;
import core.tools.Log;
import oracle.Support;
// The ContentAPI functions the framework calls.
public class ContentAPIKt {
    public static void setVarbit(Player p, int varbit, int value) { Support.emit("set_varbit", p.key, varbit, value); }
    public static void setVarp(Player p, int varp, int value) { Support.emit("set_varp", p.key, varp, value); }
    public static void playAudio(Player p, int sound) { Support.emit("play_audio", p.key, sound); }
    public static void sendMessage(Player p, String text) { Support.emit("api_message", p.key, text); }
    public static void log(Class<?> origin, Log level, String message) { Support.emit("log", origin.getName(), level.name(), message); }
}
''')

adapter('content/data/Quests.java', 'package content.data;\npublic enum Quests { WOLF_WHISTLE }\n')

adapter('content/global/skill/skillcapeperks/SkillcapePerks.java', r'''package content.global.skill.skillcapeperks;
import core.game.node.entity.player.Player;
import oracle.Support;
public enum SkillcapePerks {
    PET_MASTERY;
    public static boolean isActive(SkillcapePerks perk, Player p) {
        Support.emit("pet_mastery", p.key); return Support.bool("pet_mastery", p.key, false);
    }
}
''')

adapter('content/global/skill/summoning/SummoningPouch.java', r'''package content.global.skill.summoning;
import oracle.Support;
import java.util.Map;
// SummoningPouch.get(id) records one pouch event and answers the case's pouch (or null).
public class SummoningPouch {
    final Map m; public boolean abyssal;
    SummoningPouch(Map m) { this.m = m; this.abyssal = Boolean.TRUE.equals(m.get("abyssal")); }
    public static SummoningPouch get(int id) {
        Support.emit("pouch", id);
        Map m = (Map) ((Map) Support.input().getOrDefault("pouches", new java.util.HashMap())).get(Integer.toString(id));
        return m == null ? null : new SummoningPouch(m);
    }
    public int getLevelRequired() { return ((Number) m.get("level")).intValue(); }
    public int getSummonCost() { return ((Number) m.get("cost")).intValue(); }
    public int getNpcId() { return ((Number) m.get("npc")).intValue(); }
    public double getSummonExperience() { return ((Number) m.get("exp")).doubleValue(); }
    public boolean getPeaceful() { return Boolean.TRUE.equals(m.get("peaceful")); }
}
''')

adapter('content/global/skill/summoning/SummoningScroll.java', r'''package content.global.skill.summoning;
import oracle.Support;
import java.util.Map;
public class SummoningScroll {
    final Map m;
    SummoningScroll(Map m) { this.m = m; }
    public static SummoningScroll forPouch(int pouchId) {
        Support.emit("scroll", pouchId);
        Map m = (Map) ((Map) Support.input().getOrDefault("scrolls", new java.util.HashMap())).get(Integer.toString(pouchId));
        return m == null ? null : new SummoningScroll(m);
    }
    public int getItemId() { return ((Number) m.get("item")).intValue(); }
    public double getExperience() { return ((Number) m.get("exp")).doubleValue(); }
}
''')

# ---------------------------------------------------------------- the content classes of the corpus
CASE_MEMBERS = r'''
    @Override public Familiar construct(Player owner, int id) {
        Support.emit("construct", key, Support.key(owner), id);
        Familiar f = Support.create(id, owner);
        Support.kept(f);
        return f;
    }
    @Override protected boolean specialMove(FamiliarSpecial special) {
        Node n = special.getNode();
        Support.emit("special_move", key, n == null ? null : n instanceof Item && n.key == null ? Support.item((Item) n) : Support.key(n),
                     special.getInterfaceID(), special.getComponent(), Support.item(special.getItem()));
        return Support.bool("special_move", key, true);
    }
    @Override public int[] getIds() {
        Support.emit("ids", key);
        List l = (List) Support.classSpec(getOriginalId()).get("ids");
        int[] out = new int[l.size()];
        for (int i = 0; i < out.length; i++) out[i] = ((Number) l.get(i)).intValue();
        return out;
    }
    @Override protected void configureFamiliar() { Support.emit("configure", key); }
    @Override protected String getText() { Support.emit("text", key); return (String) Support.answer("text", key, ""); }
    @Override public void handleFamiliarTick() { Support.emit("familiar_tick", key); super.handleFamiliarTick(); }
    @Override public void adjustPlayerBattle(BattleState state) { Support.emit("adjust_player_battle", key, state.key); }
    void boosts() {
        for (Object b : (List) Support.classSpec(getOriginalId()).getOrDefault("boosts", new java.util.ArrayList())) {
            List l = (List) b; this.boosts.add(new SkillBonus(((Number) l.get(0)).intValue(), ((Number) l.get(1)).doubleValue()));
        }
    }
'''
CASE_IMPORTS = r'''package oracle;
import content.global.skill.summoning.familiar.*;
import core.game.node.Node;
import core.game.node.entity.combat.BattleState;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.SkillBonus;
import core.game.node.item.Item;
import java.util.List;
import java.util.Map;
'''

adapter('oracle/CaseFamiliar.java', CASE_IMPORTS + r'''
// A content class extending Familiar, built from the case's class table (with or without an attack style).
public class CaseFamiliar extends Familiar {
    static Map s(int id) { return Support.classSpec(id); }
    static int i(int id, String k) { return ((Number) s(id).get(k)).intValue(); }
    public CaseFamiliar(Player owner, int id) {
        this(owner, id, s(id).containsKey("style"));
    }
    CaseFamiliar(Player owner, int id, boolean styled) {
        super(owner, id, i(id, "ticks"), i(id, "pouch"), i(id, "cost"), styled ? i(id, "style") : core.game.node.entity.combat.equipment.WeaponInterface.STYLE_DEFENSIVE);
        boosts();
    }
''' + CASE_MEMBERS + '}\n')

adapter('oracle/CaseBeast.java', CASE_IMPORTS + r'''
// A content class extending BurdenBeast.
public class CaseBeast extends BurdenBeast {
    static Map s(int id) { return Support.classSpec(id); }
    static int i(int id, String k) { return ((Number) s(id).get(k)).intValue(); }
    public CaseBeast(Player owner, int id) {
        super(owner, id, i(id, "ticks"), i(id, "pouch"), i(id, "cost"), i(id, "size"), s(id).containsKey("style") ? i(id, "style") : core.game.node.entity.combat.equipment.WeaponInterface.STYLE_DEFENSIVE);
        boosts();
    }
''' + CASE_MEMBERS + '}\n')

adapter('oracle/CaseForager.java', CASE_IMPORTS + r'''
// A content class extending Forager; getRandom is answered by the case (11 by default).
public class CaseForager extends Forager {
    static Map s(int id) { return Support.classSpec(id); }
    static int i(int id, String k) { return ((Number) s(id).get(k)).intValue(); }
    static Item[] items(int id) {
        List l = (List) s(id).get("items");
        Item[] out = new Item[l.size()];
        for (int k = 0; k < out.length; k++) { List e = (List) l.get(k); out[k] = new Item(((Number) e.get(0)).intValue(), ((Number) e.get(1)).intValue()); }
        return out;
    }
    public CaseForager(Player owner, int id) {
        super(owner, id, i(id, "ticks"), i(id, "pouch"), i(id, "cost"), s(id).containsKey("style") ? i(id, "style") : core.game.node.entity.combat.equipment.WeaponInterface.STYLE_DEFENSIVE, items(id));
        boosts();
    }
    @Override public int getRandom() { Support.emit("forager_random", key); return Support.integer("forager_random", key, 11); }
''' + CASE_MEMBERS + '}\n')
