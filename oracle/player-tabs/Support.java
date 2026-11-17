package oracle;

import content.data.Quests;
import core.game.component.ComponentDefinition;
import core.game.component.InterfaceType;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.equipment.WeaponInterface;
import core.game.node.entity.combat.spell.CombatSpell;
import core.game.node.entity.combat.spell.MagicSpell;
import core.game.node.entity.impl.Animator.Priority;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.emote.EmoteManager;
import core.game.node.entity.player.link.emote.Emotes;
import core.game.node.entity.player.link.music.MusicEntry;
import core.game.node.entity.player.link.music.MusicPlayer;
import core.game.node.entity.player.link.music.MusicZone;
import core.game.node.entity.player.link.quest.Quest;
import core.game.node.entity.player.link.quest.QuestRepository;
import core.game.node.entity.skill.LevelUp;
import core.game.node.entity.skill.LevelUpJinglesKt;
import core.game.node.item.Item;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.util.*;
import org.json.simple.JSONValue;
import org.json.simple.parser.JSONParser;

/** Reads one case per line, runs its operations against the unchanged tab classes and prints what they did. */
public class Support {
    /** The component definitions whose type, walkability and tab index are part of the state. */
    static final int[] WATCHED = { 92, 90, 310, 319, 406, 797, 740 };

    static Map input;
    static Map tables;
    static Map spec;
    static List<Object> events;
    static int calls, failAt;
    static boolean quiet;
    static Player player;
    static WeaponInterface weapon;

    public static class Failure extends RuntimeException {}

    public static String hex(double v) { return String.format("%016x", Double.doubleToRawLongBits(v)); }
    static int n(Object o) { return ((Number) o).intValue(); }

    public static void emit(String name, Object... args) {
        if (quiet) return;
        List row = new ArrayList();
        row.add(name);
        for (Object a : args) row.add(a);
        events.add(row);
        calls++;
        if (calls == failAt) throw new Failure();
    }

    public static List gfx(Graphics g) { return g == null ? null : Arrays.asList(g.id, g.height, g.delay); }
    public static List anim(Animation a) { return a == null ? null : Arrays.asList(a.id, a.delay, a.priority == null ? null : a.priority.name()); }
    static Animation animation(Object o) {
        if (o == null) return null;
        List a = (List) o;
        return new Animation(n(a.get(0)), n(a.get(1)), Priority.valueOf((String) a.get(2)));
    }
    public static List style(WeaponInterface.AttackStyle s) { return s == null ? null : Arrays.asList(s.getStyle(), s.getBonusType()); }

    public static Object value(Object v) {
        if (v == null || v instanceof Integer || v instanceof Boolean || v instanceof String) return v;
        if (v instanceof core.game.component.Component) return Arrays.asList("component", ((core.game.component.Component) v).getId());
        throw new IllegalStateException("attribute value " + v.getClass());
    }

    static Object unvalue(Object v) {
        if (v instanceof Long) return ((Long) v).intValue();
        return v;
    }

    public static boolean flag(String key) { return Boolean.TRUE.equals(spec.get(key)); }
    public static int ticks() { return n(spec.getOrDefault("ticks", 0L)); }
    public static int duration(int id) {
        Map d = (Map) spec.get("durations");
        Object v = d == null ? null : d.get(String.valueOf(id));
        return v == null ? 1 : n(v);
    }
    static Map item(int id) {
        Map items = (Map) spec.getOrDefault("items", tables.get("items"));
        return items == null ? null : (Map) items.get(String.valueOf(id));
    }
    public static String itemName(int id) {
        Map i = item(id);
        Object v = i == null ? null : i.get("name");
        return v == null ? "null" : (String) v;
    }
    public static Object itemConfig(int id, String key) {
        Map i = item(id);
        if (i == null || !i.containsKey(key)) return null;
        Object v = i.get(key);
        if (key.equals("attack_anims")) {
            List l = (List) v;
            Animation[] out = new Animation[l.size()];
            for (int k = 0; k < out.length; k++) out[k] = animation(l.get(k));
            return out;
        }
        if (v instanceof Long) return ((Long) v).intValue();
        return v;
    }
    public static Integer worn(int slot) {
        Map w = (Map) spec.get("worn");
        Object v = w == null ? null : w.get(String.valueOf(slot));
        return v == null ? null : n(v);
    }
    public static MagicSpell spell(String book, int id) {
        Map r = (Map) spec.getOrDefault("registered", tables.get("registered"));
        List combat = r == null ? null : (List) r.get(book);
        if (combat != null && combat.contains((long) id)) return new CombatSpell(id);
        List plain = r == null ? null : (List) r.get(book + ":plain");
        if (plain != null && plain.contains((long) id)) return new MagicSpell(id);
        return null;
    }
    public static int book() { return n(spec.getOrDefault("book", 192L)); }
    public static int combatLevel() { return n(spec.getOrDefault("combatLevel", 3L)); }
    public static int staticLevel(int slot) {
        Map l = (Map) spec.get("levels");
        Object v = l == null ? null : l.get(String.valueOf(slot));
        return v == null ? 1 : n(v);
    }
    public static int totalLevel() { return n(spec.getOrDefault("totalLevel", 32L)); }
    public static String username() { return (String) spec.getOrDefault("username", "tracer"); }
    public static String rights() { return (String) spec.getOrDefault("rights", "REGULAR_PLAYER"); }

    static Object field(Object target, Class<?> c, String name) {
        try { Field f = c.getDeclaredField(name); f.setAccessible(true); return f.get(target); } catch (Exception e) { throw new IllegalStateException(e); }
    }

    static void setField(Object target, Class<?> c, String name, Object value) {
        try { Field f = c.getDeclaredField(name); f.setAccessible(true); f.set(target, value); } catch (Exception e) { throw new IllegalStateException(e); }
    }

    static Emotes emote(Object name) { return name == null ? null : Emotes.valueOf((String) name); }
    static Quest quest(Object name) { return QuestRepository.getQuests().get(Quests.valueOf((String) name)); }
    static Object questLabel(Quest q) { return q == null ? null : q.getQuest().name(); }

    static Object run(Map op) throws Throwable {
        String name = (String) op.get("op");
        switch (name) {
            case "set": apply((Map) op.get("fields")); return null;
            // WeaponInterface
            case "weapon_new": weapon = new WeaponInterface(player); return null;
            case "weapon_open": weapon.open(player); return null;
            case "weapon_update": weapon.updateInterface(); return null;
            case "weapon_set_style": return weapon.setAttackStyle(n(op.get("button")));
            case "weapon_select":
                weapon.selectAutoSpell(n(op.get("button")), Boolean.TRUE.equals(op.get("adjust")), Boolean.TRUE.equals(op.get("forget")));
                return null;
            case "weapon_autospell_id": return weapon.getAutospellId(n(op.get("spellId")));
            case "weapon_can_autocast": return weapon.canAutocast(Boolean.TRUE.equals(op.get("message")));
            case "weapon_open_select": weapon.openAutocastSelect(); return null;
            case "weapon_restore_autocast": return weapon.restoreAutocast();
            case "weapon_restore_style": weapon.restoreAttackStyle(n(op.get("index"))); return null;
            case "weapon_interface_for": {
                Object id = op.get("item");
                return WeaponInterface.getWeaponInterface(id == null ? null : new Item(n(id))).name();
            }
            // LevelUp
            case "levelup": LevelUp.levelup(player, n(op.get("slot")), n(op.get("amount"))); return null;
            case "milestones": LevelUp.handleMilestones(player, n(op.get("slot")), n(op.get("amount"))); return null;
            case "flashing": LevelUp.sendFlashingIcons(player, n(op.get("slot"))); return null;
            case "jingle": return LevelUpJinglesKt.getSkillJingle(player, n(op.get("slot")));
            // EmoteManager and Emotes
            case "emote_refresh": player.getEmoteManager().refresh(); return null;
            case "emote_unlock": return player.getEmoteManager().unlock(emote(op.get("emote")));
            case "emote_lock": return player.getEmoteManager().lock(emote(op.get("emote")));
            case "emote_is_unlocked": return player.getEmoteManager().isUnlocked(emote(op.get("emote")));
            case "emote_save_required": return player.getEmoteManager().isSaveRequired();
            case "emote_handle": Emotes.handle(player, n(op.get("button"))); return null;
            case "emote_play": emote(op.get("emote")).play(player); return null;
            case "emote_for_id": { Emotes e = Emotes.forId(n(op.get("button"))); return e == null ? null : e.name(); }
            // MusicPlayer and MusicZone
            case "music_init": player.getMusicPlayer().init(); return null;
            case "music_unlock":
                if (op.get("play") == null) player.getMusicPlayer().unlock(n(op.get("id")));
                else player.getMusicPlayer().unlock(n(op.get("id")), Boolean.TRUE.equals(op.get("play")));
                return null;
            case "music_play": player.getMusicPlayer().play(MusicEntry.forId(n(op.get("id")))); return null;
            case "music_play_default": player.getMusicPlayer().playDefault(); return null;
            case "music_replay": player.getMusicPlayer().replay(); return null;
            case "music_tick": player.getMusicPlayer().tick(); return null;
            case "music_toggle_looping": player.getMusicPlayer().toggleLooping(); return null;
            case "music_set_looping": player.getMusicPlayer().setLooping(Boolean.TRUE.equals(op.get("looping"))); return null;
            case "music_refresh": player.getMusicPlayer().refreshList(); return null;
            case "music_has_unlocked": return player.getMusicPlayer().hasUnlocked(n(op.get("id")));
            case "music_has_air_guitar": return player.getMusicPlayer().hasAirGuitar();
            case "music_zone_enter": {
                Entity e = "npc".equals(op.get("entity")) ? new NPC() : player;
                return new MusicZone(n(op.get("id")), null).enter(e);
            }
            // QuestRepository and Quest
            case "quest_sync": player.getQuestRepository().syncronizeTab(player); return null;
            case "quest_set_stage": player.getQuestRepository().setStage(quest(op.get("quest")), n(op.get("stage"))); return null;
            case "quest_set_stage_nonmonotonic": player.getQuestRepository().setStageNonmonotonic(quest(op.get("quest")), n(op.get("stage"))); return null;
            case "quest_increment": player.getQuestRepository().incrementPoints(n(op.get("value"))); return null;
            case "quest_dock": player.getQuestRepository().dockPoints(n(op.get("value"))); return null;
            case "quest_sync_points": player.getQuestRepository().syncPoints(); return null;
            case "quest_available": return player.getQuestRepository().getAvailablePoints();
            case "quest_for_button": return questLabel(player.getQuestRepository().forButtonId(n(op.get("button"))));
            case "quest_for_index": return questLabel(player.getQuestRepository().forIndex(n(op.get("index"))));
            case "quest_complete_all": return player.getQuestRepository().hasCompletedAll();
            case "quest_is_complete": return player.getQuestRepository().isComplete(Quests.valueOf((String) op.get("quest")));
            case "quest_has_started": return player.getQuestRepository().hasStarted(Quests.valueOf((String) op.get("quest")));
            case "quest_stage": return player.getQuestRepository().getStage(Quests.valueOf((String) op.get("quest")));
            case "quest_config": {
                int[] config = quest(op.get("quest")).getConfig(player, n(op.get("stage")));
                List out = new ArrayList();
                for (int v : config) out.add(v);
                return out;
            }
            default: throw new IllegalStateException("unknown op " + name);
        }
    }

    static void apply(Map m) {
        for (Object o : m.entrySet()) {
            Map.Entry en = (Map.Entry) o;
            String key = (String) en.getKey();
            Object v = en.getValue();
            switch (key) {
                case "attributes":
                    for (Object a : ((Map) v).entrySet()) {
                        Map.Entry ae = (Map.Entry) a;
                        if (ae.getValue() == null) player.attributes.remove(ae.getKey());
                        else player.attributes.put((String) ae.getKey(), unvalue(ae.getValue()));
                    }
                    break;
                case "styleIndex": player.getSettings().attackStyleIndex = n(v); break;
                case "specialToggled": player.getSettings().specialToggled = Boolean.TRUE.equals(v); break;
                case "attackStyle": {
                    List l = (List) v;
                    player.getProperties().attackStyle = l == null ? null : new WeaponInterface.AttackStyle(n(l.get(0)), n(l.get(1)));
                    break;
                }
                case "autocast": player.getProperties().autocastSpell = v == null ? null : new CombatSpell(n(v)); break;
                case "combatMilestone": player.getSkills().combatMilestone = n(v); break;
                case "skillMilestone": player.getSkills().skillMilestone = n(v); break;
                case "emotes": {
                    List<Emotes> list = player.getEmoteManager().getEmotes();
                    list.clear();
                    for (Object e : (List) v) list.add(Emotes.values()[n(e)]);
                    break;
                }
                case "music": {
                    Map mm = (Map) v;
                    MusicPlayer mp = player.getMusicPlayer();
                    if (mm.containsKey("unlocked")) {
                        mp.getUnlocked().clear();
                        for (Object id : (List) mm.get("unlocked")) {
                            MusicEntry e = MusicEntry.forId(n(id));
                            mp.getUnlocked().put(e.getIndex(), e);
                        }
                    }
                    if (mm.containsKey("current")) mp.setCurrentMusicId(n(mm.get("current")));
                    if (mm.containsKey("playing")) mp.setPlaying(Boolean.TRUE.equals(mm.get("playing")));
                    if (mm.containsKey("looping")) setField(mp, MusicPlayer.class, "looping", Boolean.TRUE.equals(mm.get("looping")));
                    break;
                }
                case "questStages":
                    for (Object a : ((Map) v).entrySet()) {
                        Map.Entry ae = (Map.Entry) a;
                        if (ae.getValue() == null) player.getQuestRepository().getQuestList().remove(Integer.parseInt((String) ae.getKey()));
                        else player.getQuestRepository().getQuestList().put(Integer.parseInt((String) ae.getKey()), n(ae.getValue()));
                    }
                    break;
                case "questPoints": {
                    QuestRepository r = player.getQuestRepository();
                    r.incrementPoints(n(v) - r.getPoints());
                    break;
                }
                default: spec.put(key, v);
            }
        }
    }

    static Map snapshot() {
        Map m = new LinkedHashMap();
        Map attributes = new TreeMap();
        for (Map.Entry<String, Object> a : player.attributes.entrySet()) attributes.put(a.getKey(), value(a.getValue()));
        m.put("attributes", attributes);
        m.put("styleIndex", player.getSettings().attackStyleIndex);
        m.put("specialToggled", player.getSettings().specialToggled);
        m.put("attackStyle", style(player.getProperties().attackStyle));
        m.put("attackAnimation", anim(player.getProperties().attackAnimation));
        CombatSpell autocast = player.getProperties().autocastSpell;
        m.put("autocast", autocast == null ? null : autocast.getSpellId());
        core.game.component.Component tab = player.getInterfaceManager().tabs[0];
        m.put("tab0", tab == null ? null : tab.getId());
        if (weapon != null) {
            WeaponInterface.WeaponInterfaces current = weapon.getWeaponInterface();
            Animation[] anims = (Animation[]) field(weapon, WeaponInterface.class, "attackAnimations");
            List a = null;
            if (anims != null) { a = new ArrayList(); for (Animation x : anims) a.add(anim(x)); }
            m.put("weapon", Arrays.asList(current == null ? null : current.name(), weapon.isSpecialBar(), a, weapon.getId()));
        } else m.put("weapon", null);
        List emotes = new ArrayList();
        for (Emotes e : player.getEmoteManager().getEmotes()) emotes.add(e.ordinal());
        m.put("emotes", emotes);
        MusicPlayer mp = player.getMusicPlayer();
        List unlocked = new ArrayList();
        for (Map.Entry<Integer, MusicEntry> e : mp.getUnlocked().entrySet()) unlocked.add(Arrays.asList(e.getKey(), e.getValue().getId()));
        m.put("music", Arrays.asList(unlocked, mp.getCurrentMusicId(), mp.isPlaying(), mp.isLooping()));
        QuestRepository r = player.getQuestRepository();
        List stages = new ArrayList();
        for (Map.Entry<Integer, Integer> e : r.getQuestList().entrySet()) stages.add(Arrays.asList(e.getKey(), e.getValue()));
        m.put("quests", Arrays.asList(r.getPoints(), stages));
        m.put("milestones", Arrays.asList(player.getSkills().combatMilestone, player.getSkills().skillMilestone));
        Map definitions = new TreeMap();
        for (int id : WATCHED) {
            ComponentDefinition d = ComponentDefinition.getDefinitions().get(id);
            if (d != null) definitions.put(String.valueOf(id), Arrays.asList(d.getType() == null ? null : d.getType().name(), d.isWalkable(), d.getTabIndex()));
        }
        m.put("definitions", definitions);
        return m;
    }

    static String error(Throwable t) {
        if (t instanceof Failure) return "Binding";
        if (t instanceof NullPointerException) return "Null";
        if (t instanceof IndexOutOfBoundsException) return "Index";
        if (t instanceof ClassCastException) return "Cast";
        if (t instanceof IllegalStateException) return "IllegalState: " + t.getMessage();
        return t.getClass().getName() + ": " + t.getMessage();
    }

    @SuppressWarnings("unchecked")
    static void reset(Map input) throws Exception {
        ComponentDefinition.getDefinitions().clear();
        Map defs = (Map) input.get("definitions");
        if (defs != null) {
            for (Object o : defs.entrySet()) {
                Map.Entry en = (Map.Entry) o;
                List l = (List) en.getValue();
                ComponentDefinition d = ComponentDefinition.forId(Integer.parseInt((String) en.getKey()));
                d.setType(l.get(0) == null ? null : InterfaceType.valueOf((String) l.get(0)));
                d.setWalkable(Boolean.TRUE.equals(l.get(1)));
                d.setTabIndex(n(l.get(2)));
            }
        }
        MusicEntry.getSongs().clear();
        for (Object o : (List) input.getOrDefault("songs", Collections.emptyList())) {
            List l = (List) o;
            MusicEntry.getSongs().put(n(l.get(0)), new MusicEntry(n(l.get(0)), (String) l.get(1), n(l.get(2))));
        }
        QuestRepository.getQuests().clear();
        late.clear();
        for (Object o : (List) input.getOrDefault("quests", Collections.emptyList())) {
            List l = (List) o;
            List cl = (List) l.get(4);
            int[] configs = new int[cl.size()];
            for (int i = 0; i < configs.length; i++) configs[i] = n(cl.get(i));
            // The row's ordinal and display name are the case's reading of Quests.kt: check them against the compiled enum.
            Quests q = Quests.valueOf((String) l.get(0));
            if (q.ordinal() != n(l.get(7)) || !q.toString().equals(l.get(8))) throw new IllegalStateException("quest row " + l);
            CaseQuest quest = new CaseQuest(q, n(l.get(1)), n(l.get(2)), n(l.get(3)), configs, Boolean.TRUE.equals(l.get(5)), (List) l.get(6));
            // A late quest is registered after the player's repository was constructed, so its stage is missing.
            if (Boolean.TRUE.equals(l.get(9))) late.add(quest);
            else QuestRepository.register(quest);
        }
    }

    static List<Quest> late = new ArrayList<>();

    /** The Quests table the cases and the native test read: every constant's name and display name, in declaration order. */
    static void checkQuestTable() {
        List table = (List) tables.get("quests");
        Quests[] values = Quests.values();
        if (table.size() != values.length) throw new IllegalStateException("quest table size");
        for (int i = 0; i < values.length; i++) {
            List row = (List) table.get(i);
            if (!values[i].name().equals(row.get(0)) || !values[i].toString().equals(row.get(1))) throw new IllegalStateException("quest table " + row);
        }
    }

    public static void main(String[] args) throws Exception {
        tables = (Map) new JSONParser().parse(new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(args[0])), StandardCharsets.UTF_8));
        checkQuestTable();
        events = new ArrayList<>();
        BufferedReader reader = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        JSONParser parser = new JSONParser();
        String line;
        StringBuilder out = new StringBuilder();
        while ((line = reader.readLine()) != null) {
            if (line.isEmpty()) continue;
            input = (Map) parser.parse(line);
            Map original = (Map) parser.parse(line);
            events = new ArrayList<>();
            quiet = true;
            reset(input);
            spec = new HashMap();
            player = new Player();
            player.emoteManager = new EmoteManager(player);
            player.musicPlayer = new MusicPlayer(player);
            player.questRepository = new QuestRepository(player);
            weapon = null;
            for (Quest q : late) QuestRepository.register(q);
            apply((Map) input.get("player"));
            quiet = false;
            List results = new ArrayList();
            Map previous = snapshot();
            Map initial = previous;
            for (Object o : (List) input.get("ops")) {
                Map op = (Map) o;
                events = new ArrayList<>();
                calls = 0;
                failAt = op.containsKey("failAt") ? n(op.get("failAt")) : -1;
                Map step = new LinkedHashMap();
                Object result = null;
                String error = null;
                try {
                    result = run(op);
                } catch (Throwable t) {
                    error = error(t);
                }
                step.put("result", result);
                step.put("error", error);
                step.put("events", events);
                Map state = snapshot();
                // The whole state after the operation, or null when it equals the state before it.
                step.put("state", state.equals(previous) ? null : state);
                previous = state;
                results.add(step);
            }
            Map row = new LinkedHashMap();
            row.put("input", original);
            row.put("expected", results);
            row.put("initial", initial);
            out.append(JSONValue.toJSONString(row)).append('\n');
        }
        System.out.write(out.toString().getBytes(StandardCharsets.UTF_8));
        System.out.flush();
    }
}
