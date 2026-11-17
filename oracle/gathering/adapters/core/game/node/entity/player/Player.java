package core.game.node.entity.player;
import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.player.link.diary.DiaryType;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import oracle.Support;
public class Player extends Entity {
    public Player(int index) { this.ref = 1000 + index; this.rawId = index; }
    private final Skills skills = new Skills();
    private final Container inventory = new Container(false);
    private final core.game.container.impl.EquipmentContainer equipment = new core.game.container.impl.EquipmentContainer();
    private final Animator animator = new Animator();
    private final Scripts scripts = new Scripts();
    private final PulseManager pulseManager = new PulseManager();
    @Override public boolean isActive() { return Support.emit("player.isActive", Support.playerActive, ref); }
    @Override public Location getLocation() { return Support.emit("player.location", Support.location, ref); }
    public Skills getSkills() { return skills; }
    public Container getInventory() { return inventory; }
    public core.game.container.impl.EquipmentContainer getEquipment() { return equipment; }
    public Animator getAnimator() { return animator; }
    public Scripts getScripts() { return scripts; }
    public PulseManager getPulseManager() { return pulseManager; }
    public boolean animate(Animation a) { Support.emit("player.animate", null, a.id, a.priority == null ? null : a.priority.name()); return true; }
    public void sendMessage(String s) { Support.emit("player.sendMessage", null, s); }
    public void sendMessages(String... s) { Support.emit("player.sendMessages", null, (Object) s); }
    public void dispatch(Object event) { Support.emit("player.dispatch", null, event.toString()); }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fallback) { Object v = Support.attributes.get(key.replace("/save:", "")); return Support.emit("player.getAttribute", v == null ? fallback : (T) v, key, fallback); }
    public void setAttribute(String key, Object value) { Support.emit("player.setAttribute", null, key, value); Support.attributes.put(key.startsWith("/save:") ? key.substring(6) : key, value); }
    public Dispatch getPacketDispatch() { return new Dispatch(); }
    public static class Dispatch { public void sendMessage(String s) { Support.emit("packetDispatch.sendMessage", null, s); } }
    public Familiars getFamiliarManager() { return new Familiars(); }
    public static class Familiars {
        public int getBoost(int skill) { return Support.emit("familiar.getBoost", Support.boosts[skill], skill); }
        public boolean hasFamiliar() { return Support.emit("familiar.hasFamiliar", Support.hasFamiliar); }
        /** null, or [is a Forager, owner token]. */
        public content.global.skill.summoning.familiar.Familiar getFamiliar() {
            content.global.skill.summoning.familiar.Familiar f = Support.familiar;
            Support.emit("familiar.getFamiliar", f == null ? null : Support.list(f instanceof content.global.skill.summoning.familiar.Forager, f.ref));
            return f;
        }
    }
    public core.game.world.map.Direction getDirection() { Support.emit("player.direction", Support.list(Support.direction[0], Support.direction[1])); return new core.game.world.map.Direction(Support.direction[0], Support.direction[1]); }
    public int[] getClocks() { int[] copy = Support.clocks.clone(); org.json.simple.JSONArray a = new org.json.simple.JSONArray(); for (int c : copy) a.add((long) c); Support.emit("player.clocks", a); return copy; }
    public void incrementAttribute(String key) {
        Support.emit("player.incrementAttribute", null, key);
        String k = key.replace("/save:", ""); Object v = Support.attributes.get(k);
        Support.attributes.put(k, (v == null ? 0 : (Integer) v) + 1);
    }
    public void debug(String s) { Support.emit("player.debug", null, s); }
    public SkillTaskData getSkillTasks() { return new SkillTaskData(); }
    public static class SkillTaskData {
        public boolean hasTask() { return Support.emit("skillTasks.hasTask", Support.hasTask); }
        public void decreaseTask(Player p, core.game.node.entity.player.link.skillertasks.SkillTasks task) { Support.emit("skillTasks.decreaseTask", null, task.name()); }
    }
    public Diaries getAchievementDiaryManager() { return new Diaries(); }
    public static class Diaries {
        public Diary getDiary(DiaryType t) { return new Diary(t); }
        public int getArmour() { return Support.emit("diary.getArmour", Support.armour); }
        public void finishTask(Player p, DiaryType t, int level, int index) { Support.emit("diary.finishTask", null, t.name(), level, index); }
    }
    public static class Diary {
        private final DiaryType type;
        Diary(DiaryType t) { type = t; }
        public boolean isComplete(int level) { return Support.emit("diary.isComplete", Support.diaryComplete.contains(type.name() + ":" + level), type.name(), level); }
        public int getLevel() { return Support.emit("diary.getLevel", Support.diaryLevel, type.name()); }
    }
    public Dialogue getDialogueInterpreter() { return new Dialogue(); }
    public static class Dialogue {
        public Object sendDialogue(String... s) { Support.emit("dialogue.sendDialogue", null, (Object) s); return null; }
        public Object sendDialogues(int npc, core.game.dialogue.FacialExpression e, String... s) { Support.emit("dialogue.sendDialogues", null, npc, e.name(), s); return null; }
    }
    public static class Scripts { public void reset() { Support.emit("scripts.reset", null); Support.hasScript = false; } }
    public static class PulseManager {
        public void run(core.game.system.task.Pulse p) { Support.pulses.add(p); Support.emit("pulseManager.run", null, p.getDelay()); }
    }
}
