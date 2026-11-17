package oracle;

import core.cache.def.Definition;
import core.cache.def.impl.ItemDefinition;
import core.game.container.Container;
import core.game.container.ContainerEvent;
import core.game.container.ContainerListener;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import java.lang.reflect.Field;
import java.util.Map;
import org.json.simple.JSONArray;
import org.json.simple.JSONObject;

/** Declared set-up, edits and observation of the original Skills and Container objects of the case's player. */
public final class Real {
    /** One per container: what a listener is told, without acting on it. */
    static final class Listener implements ContainerListener {
        private final int label;
        Listener(int label) { this.label = label; }
        @SuppressWarnings("unchecked")
        @Override public void update(Container c, ContainerEvent event) {
            JSONArray slots = new JSONArray();
            for (int slot : event.getSlots()) { Item x = event.getItems()[slot]; slots.add(Support.list(slot, x.getId(), x.getAmount())); }
            Support.emit("containerListener.update", null, label, slots);
        }
        @Override public void refresh(Container c) { Support.emit("containerListener.refresh", null, label); }
    }
    private static Object field(Class<?> type, String name, Object target) {
        try { Field f = type.getDeclaredField(name); f.setAccessible(true); return f.get(target); } catch (ReflectiveOperationException e) { throw new IllegalStateException(e); }
    }
    private static void put(Class<?> type, String name, Object target, Object value) {
        try { Field f = type.getDeclaredField(name); f.setAccessible(true); f.set(target, value); } catch (ReflectiveOperationException e) { throw new IllegalStateException(e); }
    }
    private static Skills skills() { return Support.player.getSkills(); }
    private static void levels(Object declared) {
        if (declared == null) return;
        double[] experience = (double[]) field(Skills.class, "experience", skills());
        for (Object x : ((JSONObject) declared).entrySet()) {
            Map.Entry<?, ?> e = (Map.Entry<?, ?>) x; JSONArray v = (JSONArray) e.getValue(); int s = Integer.parseInt((String) e.getKey());
            skills().getDynamicLevels()[s] = Support.n(v.get(0)); skills().getStaticLevels()[s] = Support.n(v.get(1));
            experience[s] = skills().getExperienceByLevel(Support.n(v.get(1)));
        }
    }
    public static void reset(JSONObject c) {
        // Definitions first: an Item caches its definition when it is constructed.
        ItemDefinition.getDefinitions().clear();
        for (Map.Entry<Integer, String> e : Support.names.entrySet()) {
            ItemDefinition d = new ItemDefinition();
            put(Definition.class, "id", d, e.getKey()); put(Definition.class, "name", d, e.getValue());
            put(ItemDefinition.class, "stackable", d, Support.stackable.contains(e.getKey()));
            ItemDefinition.getDefinitions().put(e.getKey(), d);
        }
        levels(c.get("levels"));
        if (c.get("xp") != null) {
            double[] experience = (double[]) field(Skills.class, "experience", skills());
            for (Object x : ((JSONObject) c.get("xp")).entrySet()) { Map.Entry<?, ?> e = (Map.Entry<?, ?>) x; experience[Integer.parseInt((String) e.getKey())] = ((Number) e.getValue()).doubleValue(); }
        }
        Container inventory = Support.player.getInventory(), equipment = Support.player.getEquipment();
        if (c.get("inventory") != null) { int slot = 0; for (Object o : (JSONArray) c.get("inventory")) { JSONArray p = (JSONArray) o; inventory.replace(new Item(Support.n(p.get(0)), Support.n(p.get(1))), slot++, false); } }
        for (Map.Entry<Integer, Integer> e : new java.util.TreeMap<>(Support.equipment).entrySet()) equipment.replace(new Item(e.getValue()), e.getKey(), false);
        inventory.refresh(); equipment.refresh();
        inventory.register(new Listener(0)); equipment.register(new Listener(1));
    }
    @SuppressWarnings("unchecked")
    private static JSONArray slots(Container c) {
        JSONArray a = new JSONArray();
        for (Item x : c.toArray()) a.add(x == null ? null : Support.list(x.getId(), x.getAmount(), x.getCharge()));
        return a;
    }
    @SuppressWarnings("unchecked")
    public static void state(JSONObject s) {
        Skills k = skills(); JSONArray levels = new JSONArray();
        for (int i = 0; i < 24; i++) levels.add(Support.list(k.getDynamicLevels()[i], k.getStaticLevels()[i], k.getExperience(i)));
        s.put("realSkills", Support.list(levels, k.getLifepoints(), k.getPrayerPoints(), k.getExperienceGained(), k.getTotalLevel()));
        s.put("realInventory", slots(Support.player.getInventory())); s.put("realEquipment", slots(Support.player.getEquipment()));
    }
    public static void set(JSONObject op) {
        Container inventory = Support.player.getInventory();
        levels(op.get("levels"));
        if (op.get("fill") != null) { int k = Support.n(op.get("fill")); while (28 - inventory.freeSlots() < k) inventory.add(new Item(1), false, -1); }
        if (op.get("take") != null) inventory.remove(new Item(Support.n(op.get("take"))), false);
        if (op.get("give") != null) { JSONArray a = (JSONArray) op.get("give"); inventory.add(new Item(Support.n(a.get(0)), Support.n(a.get(1))), false, -1); }
        if (op.get("equip") != null) { JSONArray a = (JSONArray) op.get("equip"); Support.player.getEquipment().replace(a.get(1) == null ? null : new Item(Support.n(a.get(1))), Support.n(a.get(0)), false); }
    }
}