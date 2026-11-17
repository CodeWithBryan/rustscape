"""Recording adapters for the global-handlers oracle.

Each entry is a source file compiled with the unchanged originals listed in generate.py. `/* ORIGINAL_* */` markers are
replaced by original text read from the pinned sources at generation time. Everything else records the call in
oracle.Support and answers from the case. Field names differ from the getters' Kotlin properties so that Kotlin reads go
through the getters, as in the original.
"""

J = 'oracle.Support'

ADAPTERS = {
    # ------------------------------------------------------------------ nodes
    'core/game/node/Node.java': r'''package core.game.node;
import core.game.world.map.Location;
public class Node {
    public String key, nodeName;
    public int nodeId;
    public Location nodeLocation;
    public boolean nodeActive = true;
    public int getId() { return nodeId; }
    public String getName() { return nodeName; }
    public Location getLocation() { return nodeLocation; }
    public boolean isActive() { return nodeActive; }
    public core.game.node.entity.npc.NPC asNpc() { return (core.game.node.entity.npc.NPC) this; }
    public core.game.node.scenery.Scenery asScenery() { return (core.game.node.scenery.Scenery) this; }
    public core.game.node.item.Item asItem() { return (core.game.node.item.Item) this; }
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
    public core.game.system.timer.TimerManager timers = new core.game.system.timer.TimerManager();
    final core.game.node.entity.impl.Properties props = new core.game.node.entity.impl.Properties();
    final core.game.node.entity.player.link.TeleportManager teleporter = new core.game.node.entity.player.link.TeleportManager();
    public core.game.node.entity.impl.Properties getProperties() { return props; }
    public core.game.node.entity.player.link.TeleportManager getTeleporter() { return teleporter; }
    public boolean face(Entity entity) { Support.emit("face", key, Support.key(entity)); return true; }
    public void faceLocation(Location l) { Support.emit("faceLocation", key, Support.coord(l)); }
    public boolean inCombat() { Support.emit("inCombat", key); return Support.answerBool("inCombat:" + key, false); }
    public void attack(final Node node) { Support.emit("attack", Support.key(node)); }
    public void lock() { Support.emit("lockAll", key); }
    public void lock(int time) { Support.emit("lockTicks", key, time); }
    public void unlock() { Support.emit("unlock", key); }
    public boolean animate(Animation animation) { Support.emit("animatePlayer", key, animation == null ? null : animation.oracleId()); return true; }
    public boolean visualize(Animation animation, Graphics graphics) { Support.emit("visualize", key, animation.oracleId(), graphics.oracleId()); return true; }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k, T fail) { Support.emit("getAttribute", key, k); return attrs.containsKey(k) ? (T) attrs.get(k) : fail; }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k) { Support.emit("getAttribute", key, k); return (T) attrs.get(k); }
    public void setAttribute(String k, Object v) { Support.emit("setAttribute", key, k, Support.value(v)); attrs.put(k, v); }
    public void removeAttribute(String k) { Support.emit("removeAttribute", key, k); attrs.remove(k); }
    public Map<String, Object> getAttributes() { Support.emit("attributes", key); return attrs; }
}
''',
    'core/game/node/entity/impl/Properties.java': r'''package core.game.node.entity.impl;
import oracle.Support;
public class Properties {
    final core.game.node.entity.combat.CombatPulse pulse = new core.game.node.entity.combat.CombatPulse();
    public Object getAutocastSpell() { Support.emit("autocastSpell"); return Support.answerBool("autocastSpell", false) ? "spell" : null; }
    public core.game.node.entity.combat.CombatPulse getCombatPulse() { return pulse; }
    public core.game.world.update.flag.context.Animation getAttackAnimation() {
        Support.emit("attackAnimation");
        return core.game.world.update.flag.context.Animation.create(Support.answerInt("attackAnimation", 422));
    }
    public int getCurrentCombatLevel() { Support.emit("combatLevel"); return Support.answerInt("combatLevel", 3); }
    public core.game.node.entity.combat.AttackStyle getAttackStyle() { return new core.game.node.entity.combat.AttackStyle(); }
    public void setTeleportLocation(core.game.world.map.Location l) { Support.emit("setTeleportLocation", Support.coord(l)); }
}
''',
    'core/game/node/entity/impl/PulseType.java': 'package core.game.node.entity.impl;\npublic enum PulseType { STANDARD, STRONG }\n',
    'core/game/node/entity/impl/PulseManager.java': r'''package core.game.node.entity.impl;
import core.game.system.task.Pulse;
public class PulseManager {
    public void run(Pulse pulse) { oracle.Support.emit("run", oracle.Support.track(pulse), null); }
    public void run(Pulse pulse, PulseType pulseType) { oracle.Support.emit("run", oracle.Support.track(pulse), pulseType == null ? null : pulseType.name()); }
}
''',
    'core/game/node/entity/combat/CombatPulse.java': r'''package core.game.node.entity.combat;
public class CombatPulse {
    public core.game.node.entity.Entity getVictim() { oracle.Support.emit("victim"); return (core.game.node.entity.Entity) oracle.Support.node(oracle.Support.answer("victim", null)); }
}
''',
    'core/game/node/entity/combat/AttackStyle.java': r'''package core.game.node.entity.combat;
public class AttackStyle {
    public int getStyle() { oracle.Support.emit("attackStyle"); return oracle.Support.answerInt("attackStyle", 0); }
}
''',
    'core/game/node/entity/combat/BattleState.java': 'package core.game.node.entity.combat;\npublic class BattleState {}\n',
    'core/game/node/entity/combat/CombatStyle.java': r'''package core.game.node.entity.combat;
public enum CombatStyle {
    MELEE;
    public CombatSwingHandler getSwingHandler() { return new CombatSwingHandler(); }
}
''',
    'core/game/node/entity/combat/CombatSwingHandler.java': r'''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
import oracle.Support;
public class CombatSwingHandler {
    public CombatSwingHandler getSpecial(int itemId) {
        Support.emit("getSpecial", itemId);
        Object label = Support.answer("getSpecial", null);
        return label == null ? null : new Support.SpecialHandler((String) label);
    }
    public int swing(Entity entity, Entity victim, BattleState state) { throw new IllegalStateException("base swing"); }
}
''',
    'core/game/node/entity/combat/DeathTask.java': r'''package core.game.node.entity.combat;
import core.game.container.Container;
import core.game.node.entity.player.Player;
public class DeathTask {
    public static Container[] getContainers(Player player) {
        oracle.Support.emit("deathContainers");
        return new Container[] { oracle.Support.containers.get("kept"), oracle.Support.containers.get("lost") };
    }
}
''',
    'core/game/node/entity/combat/equipment/WeaponInterface.java': r'''package core.game.node.entity.combat.equipment;
import core.game.component.Component;
import oracle.Support;
import org.rs09.consts.Components;
public class WeaponInterface extends Component {
    public static final int STYLE_ACCURATE = 0;
    public static final int STYLE_AGGRESSIVE = 1;
    public static final int STYLE_CONTROLLED = 2;
    public static final int STYLE_DEFENSIVE = 3;
    public WeaponInterface() { super(92); }
    public boolean isSpecialBar() { Support.emit("isSpecialBar"); return Support.answerBool("isSpecialBar", false); }
    public boolean setAttackStyle(int button) { Support.emit("setAttackStyle", button); return Support.answerBool("setAttackStyle", false); }
    public void openAutocastSelect() { Support.emit("openAutocastSelect"); }
    public void selectAutoSpell(int buttonId, boolean adjustAttackStyle, boolean forget) { Support.emit("selectAutoSpell", buttonId, adjustAttackStyle, forget); }
    public void updateInterface() { Support.emit("updateWeaponInterface"); }
    public static enum WeaponInterfaces {
        /* ORIGINAL_WEAPON_INTERFACES */;
        private final int interfaceId;
        WeaponInterfaces(int interfaceId) { this.interfaceId = interfaceId; }
        public int getInterfaceId() { return interfaceId; }
    }
}
''',
    'core/game/node/entity/combat/spell/MagicSpell.java': r'''package core.game.node.entity.combat.spell;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.SpellBookManager.SpellBook;
public class MagicSpell {
    public static boolean castSpell(final Player p, SpellBook book, int spellId, Node target) {
        oracle.Support.emit("castSpell", book == null ? null : book.name(), spellId, oracle.Support.key(target));
        return oracle.Support.answerBool("castSpell", false);
    }
}
''',
    'core/game/node/entity/npc/NPC.java': r'''package core.game.node.entity.npc;
import core.game.world.map.Location;
public class NPC extends core.game.node.entity.Entity {
    public NPC() {}
    public NPC(int id, Location location) { nodeId = id; nodeLocation = location; }
}
''',
    'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
import core.game.container.Container;
import core.game.container.impl.BankContainer;
import core.game.container.impl.EquipmentContainer;
import core.game.node.entity.player.link.*;
import core.game.node.entity.player.link.diary.AchievementDiaryManager;
import core.game.node.entity.player.link.music.MusicPlayer;
import core.game.node.entity.player.link.prayer.Prayer;
import core.game.node.entity.player.link.quest.QuestRepository;
import core.game.node.entity.player.link.request.RequestManager;
import core.game.node.entity.impl.PulseManager;
import core.game.node.entity.lock.ActionLocks;
import core.game.node.entity.skill.Skills;
import core.game.system.communication.CommunicationInfo;
import core.game.world.map.zone.ZoneMonitor;
import content.global.skill.summoning.familiar.FamiliarManager;
import oracle.Support;
public class Player extends core.game.node.entity.Entity {
    final ZoneMonitor zoneMonitor = new ZoneMonitor();
    final Settings settings = new Settings();
    final IronmanManager ironmanManager = new IronmanManager();
    final SkullManager skullManager = new SkullManager();
    final Prayer prayer = new Prayer();
    final FamiliarManager familiarManager = new FamiliarManager();
    final PacketDispatch packetDispatch = new PacketDispatch();
    final MusicPlayer musicPlayer = new MusicPlayer();
    final QuestRepository questRepository = new QuestRepository();
    final AchievementDiaryManager diaryManager = new AchievementDiaryManager();
    final InterfaceManager interfaceManager = new InterfaceManager();
    final CommunicationInfo communication = new CommunicationInfo();
    final PlayerDetails details = new PlayerDetails();
    final ActionLocks locks = new ActionLocks();
    final SavedData savedData = new SavedData();
    final PulseManager pulseManager = new PulseManager();
    final RequestManager requestManager = new RequestManager();
    final Skills skills = new Skills();
    final core.game.dialogue.DialogueInterpreter dialogueInterpreter = new core.game.dialogue.DialogueInterpreter();
    public ZoneMonitor getZoneMonitor() { return zoneMonitor; }
    public Settings getSettings() { return settings; }
    public IronmanManager getIronmanManager() { return ironmanManager; }
    public SkullManager getSkullManager() { return skullManager; }
    public Prayer getPrayer() { return prayer; }
    public FamiliarManager getFamiliarManager() { return familiarManager; }
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public MusicPlayer getMusicPlayer() { return musicPlayer; }
    public QuestRepository getQuestRepository() { return questRepository; }
    public AchievementDiaryManager getAchievementDiaryManager() { return diaryManager; }
    public InterfaceManager getInterfaceManager() { return interfaceManager; }
    public CommunicationInfo getCommunication() { return communication; }
    public PlayerDetails getDetails() { return details; }
    public ActionLocks getLocks() { return locks; }
    public SavedData getSavedData() { return savedData; }
    public GlobalData getGlobalData() { return savedData.getGlobalData(); }
    public PulseManager getPulseManager() { return pulseManager; }
    public RequestManager getRequestManager() { return requestManager; }
    public Skills getSkills() { return skills; }
    public core.game.dialogue.DialogueInterpreter getDialogueInterpreter() { return dialogueInterpreter; }
    public Container getInventory() { return Support.containers.get("inventory"); }
    public EquipmentContainer getEquipment() { return (EquipmentContainer) Support.containers.get("equipment"); }
    public BankContainer getBank() { return Support.bank; }
    public boolean isTeleporting() { Support.emit("isTeleporting"); return Support.answerBool("isTeleporting", false); }
    public boolean isAdmin() { Support.emit("isAdmin"); return Support.answerBool("isAdmin", false); }
    public boolean isArtificial() { Support.emit("isArtificial"); return Support.answerBool("isArtificial", false); }
    public void toggleDebug() { Support.emit("toggleDebug"); }
    public void debug(String text) { Support.emit("debug", text); }
    public String getUsername() { Support.emit("username"); return Support.text("username", "tracer"); }
    public void sendMessage(String text) { Support.emit("message", text); }
    public void updateSceneGraph(boolean login) { Support.emit("updateSceneGraph", login); }
    @SuppressWarnings("unchecked")
    public <T> T getExtension(Class<?> type) {
        Support.emit("weaponInterface");
        Object v = Support.answer("weaponInterface", "present");
        if ("STAFF".equals(v)) return (T) core.game.node.entity.combat.equipment.WeaponInterface.WeaponInterfaces.STAFF;
        return "present".equals(v) ? (T) new core.game.node.entity.combat.equipment.WeaponInterface() : null;
    }
}
''',
    'core/game/node/entity/player/info/Rights.java': 'package core.game.node.entity.player.info;\npublic enum Rights { REGULAR_PLAYER, PLAYER_MODERATOR, ADMINISTRATOR }\n',
    'core/game/node/entity/player/info/login/LoginConfiguration.java': r'''package core.game.node.entity.player.info.login;
public class LoginConfiguration {
    public static void configureGameWorld(core.game.node.entity.player.Player player) { oracle.Support.emit("configureGameWorld"); }
}
''',
    'core/game/node/entity/player/link/PlayerDetails.java': r'''package core.game.node.entity.player.link;
import core.game.node.entity.player.info.Rights;
public class PlayerDetails {
    public Rights getRights() { oracle.Support.emit("rights"); return Rights.valueOf((String) oracle.Support.answer("rights", "REGULAR_PLAYER")); }
}
''',
    'core/game/node/entity/player/link/Settings.java': r'''package core.game.node.entity.player.link;
import oracle.Support;
public class Settings {
    public void toggleRun() { Support.emit("toggleRun"); }
    public void toggleChatEffects() { Support.emit("toggleChatEffects"); }
    public void toggleSplitPrivateChat() { Support.emit("toggleSplitPrivateChat"); }
    public void toggleMouseButton() { Support.emit("toggleMouseButton"); }
    public void toggleAcceptAid() { Support.emit("toggleAcceptAid"); }
    public void toggleRetaliating() { Support.emit("toggleRetaliating"); }
    public void toggleSpecialBar() { Support.emit("toggleSpecialBar"); }
    public boolean isSpecialToggled() { Support.emit("isSpecialToggled"); return Support.answerBool("isSpecialToggled", false); }
}
''',
    'core/game/node/entity/player/link/IronmanMode.java': 'package core.game.node.entity.player.link;\npublic enum IronmanMode { NONE, STANDARD, HARDCORE, ULTIMATE }\n',
    'core/game/node/entity/player/link/IronmanManager.java': r'''package core.game.node.entity.player.link;
public class IronmanManager {
    public boolean checkRestriction(IronmanMode mode) {
        oracle.Support.emit("checkRestriction", mode.name());
        return oracle.Support.answerBool("checkRestriction", false);
    }
}
''',
    'core/game/node/entity/player/link/SkullManager.java': r'''package core.game.node.entity.player.link;
public class SkullManager {
    public boolean isSkulled() { oracle.Support.emit("isSkulled"); return oracle.Support.answerBool("isSkulled", false); }
}
''',
    'core/game/node/entity/player/link/SavedData.java': r'''package core.game.node.entity.player.link;
public class SavedData {
    final GlobalData global = new GlobalData();
    final QuestData quests = new QuestData();
    public GlobalData getGlobalData() { return global; }
    public QuestData getQuestData() { return quests; }
}
''',
    'core/game/node/entity/player/link/GlobalData.java': r'''package core.game.node.entity.player.link;
import oracle.Support;
public class GlobalData {
    public void setDisableDeathScreen(boolean b) { Support.emit("setDisableDeathScreen", b); }
    public int getLootSharePoints() { Support.emit("lootSharePoints"); return Support.intValue("lootSharePoints", 0); }
    public void setLootSharePoints(int v) { Support.emit("setLootSharePoints", v); Support.values.put("lootSharePoints", (long) v); }
    public long getLootShareDelay() { Support.emit("lootShareDelay"); return Support.longValue("lootShareDelay", 0L); }
    public void setLootShareDelay(long v) {
        // The original reads the clock itself; the delay is recorded relative to it.
        long now = System.currentTimeMillis();
        long d = v - now;
        Object shown = d > 86_390_000L && d <= 86_400_000L ? "now+86400000" : Long.valueOf(v);
        Support.emit("setLootShareDelay", shown);
        Support.values.put("lootShareDelay", shown);
    }
}
''',
    'core/game/node/entity/player/link/QuestData.java': r'''package core.game.node.entity.player.link;
public class QuestData {
    public boolean getDragonSlayerAttribute(String name) { oracle.Support.emit("dragonSlayerAttribute", name); return oracle.Support.answerBool("dragonSlayerAttribute", false); }
}
''',
    'core/game/node/entity/player/link/PacketDispatch.java': r'''package core.game.node.entity.player.link;
import oracle.Support;
public class PacketDispatch {
    public void sendMessage(String message) { Support.emit("packetMessage", message); }
    public void sendMessage(final String message, int ticks) { Support.emit("packetMessageDelay", message, ticks); }
    public void sendIfaceSettings(int settingsHash, int childId, int interfaceId, int offset, int length) { Support.emit("ifaceSettings", settingsHash, childId, interfaceId, offset, length); }
    public void sendRunScript(int id, String string, Object... objects) { Support.emit("runScript", id, string, Support.values(objects)); }
    public void sendRunScript(int scriptId, Object... params) { Support.emit("runScriptParams", scriptId, Support.values(params)); }
    public void sendString(String string, int interfaceId, int lineId) { Support.emit("packetString", string, interfaceId, lineId); }
    public void sendInterfaceConfig(int interfaceId, int childId, boolean hide) { Support.emit("interfaceConfig", interfaceId, childId, hide); }
    public void sendScriptConfigs(int id, int value, String type, Object... params) { Support.emit("scriptConfigs", id, value, type, Support.values(params)); }
}
''',
    'core/game/node/entity/player/link/InterfaceManager.java': r'''package core.game.node.entity.player.link;
import core.game.component.Component;
import oracle.Support;
public class InterfaceManager {
    public Component getOpened() { Support.emit("opened"); return Support.answerBool("opened", false) ? new Component(Support.answerInt("openedId", 1)) : null; }
    public Component open(Component component) { Support.emit("open", component.getId()); return Support.answerBool("openNull", false) ? null : component; }
    public boolean close() { Support.emit("closeManager"); return Support.answerBool("closeManager", true); }
    public boolean closeSingleTab() { Support.emit("closeSingleTab"); return true; }
    public void removeTabs(int... tabs) { Support.emit("removeTabs", Support.values(tabs)); }
    public void openDefaultTabs() { Support.emit("openDefaultTabs"); }
    public void openTab(int slot, Component component) { Support.emit("openTab", slot, component.getId()); }
    public void openTab(Component component) { if (component instanceof core.game.node.entity.combat.equipment.WeaponInterface) Support.emit("openTabWeapon"); else Support.emit("openTabComponent", component.getId()); }
    public Component openWindowsPane(Component windowsPane) { Support.emit("openWindowsPane", windowsPane.getId(), null); return windowsPane; }
    public void openWindowsPane(Component windowsPane, int type) { Support.emit("openWindowsPane", windowsPane.getId(), type); }
    public boolean isResizable() { Support.emit("isResizable"); return Support.answerBool("isResizable", false); }
}
''',
    'core/game/node/entity/player/link/TeleportManager.java': r'''package core.game.node.entity.player.link;
import core.game.world.map.Location;
public class TeleportManager {
    public boolean send(Location location, TeleportType type) {
        oracle.Support.emit("teleport", oracle.Support.coord(location), type.name());
        return oracle.Support.answerBool("teleport", false);
    }
    public static enum TeleportType { NORMAL, TELE_OTHER }
}
''',
    'core/game/node/entity/player/link/SpellBookManager.java': r'''package core.game.node.entity.player.link;
public class SpellBookManager {
    public static enum SpellBook {
        /* ORIGINAL_SPELL_BOOKS */;
        private final int interfaceId;
        SpellBook(int interfaceId) { this.interfaceId = interfaceId; }
        public int getInterfaceId() { return interfaceId; }
    }
}
''',
    'core/game/node/entity/player/link/emote/Emotes.java': r'''package core.game.node.entity.player.link.emote;
public class Emotes {
    public static void handle(core.game.node.entity.player.Player player, int buttonId) { oracle.Support.emit("emote", buttonId); }
}
''',
    'core/game/node/entity/player/link/music/MusicEntry.java': r'''package core.game.node.entity.player.link.music;
import java.util.Map;
public class MusicEntry {
    final int songId, songIndex;
    public MusicEntry(int id, String name, int index) { songId = id; songIndex = index; }
    public int getId() { return songId; }
    public int getIndex() { return songIndex; }
    public static Map<Integer, MusicEntry> getSongs() { oracle.Support.emit("songs"); return oracle.Support.songs; }
}
''',
    'core/game/node/entity/player/link/music/MusicPlayer.java': r'''package core.game.node.entity.player.link.music;
import oracle.Support;
import java.util.Map;
public class MusicPlayer {
    public void toggleLooping() { Support.emit("toggleLooping"); }
    public Map<Integer, MusicEntry> getUnlocked() { return Support.unlocked; }
    public void play(MusicEntry entry) { Support.emit("play", entry == null ? null : entry.getId()); }
    public void unlock(int id) { Support.emit("unlock", id); }
}
''',
    'core/game/node/entity/player/link/prayer/PrayerType.java': r'''package core.game.node.entity.player.link.prayer;
public enum PrayerType {
    /* ORIGINAL_PRAYER_TYPES */;
    public static PrayerType get(int button) {
        oracle.Support.emit("prayerType", button);
        Object v = oracle.Support.answer("prayerType", null);
        return v == null ? null : valueOf((String) v);
    }
}
''',
    'core/game/node/entity/player/link/prayer/Prayer.java': r'''package core.game.node.entity.player.link.prayer;
public class Prayer {
    public boolean get(PrayerType type) { oracle.Support.emit("prayerActive", type.name()); return oracle.Support.answerBool("prayerActive", false); }
    public final boolean toggle(final PrayerType type) { oracle.Support.emit("prayerToggle", type.name()); return oracle.Support.answerBool("prayerToggle", false); }
}
''',
    'core/game/node/entity/player/link/quest/Quest.java': r'''package core.game.node.entity.player.link.quest;
public class Quest {
    final String label;
    public Quest(String label) { this.label = label; }
    public int getStage(core.game.node.entity.player.Player player) { oracle.Support.emit("questStage", label); return oracle.Support.answerInt("questStage", 0); }
    public void drawJournal(core.game.node.entity.player.Player player, int stage) { oracle.Support.emit("drawJournal", label, stage); }
}
''',
    'core/game/node/entity/player/link/quest/QuestRepository.java': r'''package core.game.node.entity.player.link.quest;
import oracle.Support;
public class QuestRepository {
    public Quest forButtonId(int buttonId) { Support.emit("questForButton", buttonId); Object v = Support.answer("questForButton", null); return v == null ? null : new Quest((String) v); }
    public void syncronizeTab(core.game.node.entity.player.Player player) { Support.emit("questSyncTab"); }
    public boolean isComplete(content.data.Quests quest) { Support.emit("questComplete", quest.name()); return Support.answerBool("questComplete:" + quest.name(), false); }
    public boolean hasCompletedAll() { Support.emit("hasCompletedAll"); return Support.answerBool("hasCompletedAll", false); }
}
''',
    'content/data/Quests.java': 'package content.data;\npublic enum Quests { RUNE_MYSTERIES, DRUIDIC_RITUAL, WOLF_WHISTLE }\n',
    'core/game/node/entity/player/link/diary/DiaryType.java': r'''package core.game.node.entity.player.link.diary;
public enum DiaryType {
    /* ORIGINAL_DIARY_TYPES */;
    public static DiaryType forChild(int child) {
        oracle.Support.emit("diaryForChild", child);
        Object v = oracle.Support.answer("diaryForChild", null);
        return v == null ? null : valueOf((String) v);
    }
}
''',
    'core/game/node/entity/player/link/diary/AchievementDiary.java': r'''package core.game.node.entity.player.link.diary;
public class AchievementDiary {
    final DiaryType type;
    AchievementDiary(DiaryType type) { this.type = type; }
    public void open(core.game.node.entity.player.Player player) { oracle.Support.emit("diaryOpen", type == null ? null : type.name()); }
}
''',
    'core/game/node/entity/player/link/diary/AchievementDiaryManager.java': r'''package core.game.node.entity.player.link.diary;
public class AchievementDiaryManager {
    public void openTab() { oracle.Support.emit("diaryOpenTab"); }
    public void finishTask(core.game.node.entity.player.Player player, DiaryType type, int level, int index) { oracle.Support.emit("finishTask", type.name(), level, index); }
    public AchievementDiary getDiary(DiaryType type) {
        oracle.Support.emit("getDiary", type == null ? null : type.name());
        return oracle.Support.answerBool("getDiary", true) ? new AchievementDiary(type) : null;
    }
}
''',
    'core/game/node/entity/player/link/request/RequestType.kt': r'''package core.game.node.entity.player.link.request

open class RequestType(val label: String) {
    companion object {
        @JvmField val TRADE = RequestType("TRADE")
        @JvmField val ASSIST = RequestType("ASSIST")
    }
}
''',
    'core/game/node/entity/player/link/request/RequestManager.java': r'''package core.game.node.entity.player.link.request;
import core.game.node.entity.player.Player;
public class RequestManager {
    public boolean request(Player target, RequestType type) {
        if (target == null || type == null) throw new NullPointerException();
        oracle.Support.emit("request", oracle.Support.key(target), type.getLabel());
        return oracle.Support.answerBool("request", true);
    }
}
''',
    'core/game/node/entity/player/link/request/assist/AssistSession.java': r'''package core.game.node.entity.player.link.request.assist;
import core.game.node.entity.player.Player;
import oracle.Support;
public class AssistSession {
    public static AssistSession getExtension(Player player) { Support.emit("assistSession"); return Support.answerBool("assistSession", false) ? new AssistSession() : null; }
    public Player getPlayer() { Support.emit("assistPlayer"); return (Player) Support.node(Support.answer("assistPlayer", "p")); }
    public void toggleButton(byte index) { Support.emit("assistToggle", (int) index); }
    public void refresh() { Support.emit("assistRefresh"); }
}
''',
    'core/game/node/entity/lock/ActionLocks.java': r'''package core.game.node.entity.lock;
import oracle.Support;
public class ActionLocks {
    public void lock(String key, int time) { Support.emit("lock", key, time); }
    public boolean isLocked(String key) { Support.emit("isLocked", key); return Support.answerBool("isLocked", false); }
    public boolean isInteractionLocked() { Support.emit("interactionLocked"); return Support.answerBool("interactionLocked", false); }
    public boolean isMovementLocked() { Support.emit("movementLocked"); return Support.answerBool("movementLocked", false); }
}
''',
    'core/game/node/entity/skill/Skills.java': r'''package core.game.node.entity.skill;
public class Skills {
    /* ORIGINAL_SKILL_IDS */
    public void addExperience(int slot, double experience) { oracle.Support.emit("addExperience", slot, experience); }
}
''',
    'core/game/node/entity/skill/LevelUp.java': r'''package core.game.node.entity.skill;
public class LevelUp {
    public static void sendFlashingIcons(core.game.node.entity.player.Player player, int skill) { oracle.Support.emit("flashingIcons", skill); }
}
''',
    'core/game/world/map/zone/ZoneMonitor.java': r'''package core.game.world.map.zone;
public class ZoneMonitor {
    public boolean canLogout() { oracle.Support.emit("canLogout"); return oracle.Support.answerBool("canLogout", true); }
    public int getType() { oracle.Support.emit("zoneType"); return oracle.Support.answerInt("zoneType", 0); }
}
''',
    'core/game/node/item/Item.java': r'''package core.game.node.item;
import core.cache.def.impl.ItemDefinition;
import core.game.interaction.OptionHandler;
import oracle.Support;
public class Item extends core.game.node.Node {
    int itemAmount = 1;
    public Item() {}
    public Item(int id) { this(id, 1); }
    public Item(int id, int amount) { nodeId = id; itemAmount = amount; }
    public void oracleAmount(int amount) { itemAmount = amount; }
    public int getAmount() { return itemAmount; }
    public ItemDefinition getDefinition() { return new ItemDefinition(this); }
    public int getNoteChange() { Support.emit("noteChange", Support.key(this)); return Support.answerInt("noteChange:" + key, nodeId); }
    public OptionHandler getOperateHandler() {
        Support.emit("operateHandler", Support.key(this));
        return Support.answerBool("operateHandler", false) ? new Support.OperateHandler() : null;
    }
}
''',
    'core/cache/def/impl/ItemDefinition.java': r'''package core.cache.def.impl;
import core.game.node.item.Item;
import oracle.Support;
public class ItemDefinition {
    final Item item;
    public ItemDefinition(Item item) { this.item = item; }
    public String getExamine() { Support.emit("examine", Support.key(item)); return (String) Support.answer("examine:" + item.key, "examine"); }
    public String[] getOptions() {
        Support.emit("inventoryOptions", Support.key(item));
        Object v = Support.answers.get("options:" + item.key);
        if (v == null) return null;
        java.util.List l = (java.util.List) v;
        return (String[]) l.toArray(new String[0]);
    }
    public boolean isUnnoted() { Support.emit("isUnnoted", Support.key(item)); return Support.answerBool("isUnnoted:" + item.key, true); }
}
''',
    'core/cache/def/impl/NPCDefinition.java': 'package core.cache.def.impl;\npublic class NPCDefinition {}\n',
    'core/cache/def/impl/SceneryDefinition.java': r'''package core.cache.def.impl;
import java.util.*;
import oracle.Support;
public class SceneryDefinition {
    public static Map<Integer, SceneryDefinition> DEFINITIONS = new TreeMap<>();
    public static Map<String, Object> OPTION_HANDLERS = new TreeMap<>();
    final int defId;
    final Map<String, Object> handlerMap;
    SceneryDefinition(int id) {
        defId = id;
        handlerMap = new TreeMap<String, Object>() {
            @Override public Object put(String k, Object v) { Support.emit("sceneryHandler", defId, k, Support.label(v)); return super.put(k, v); }
        };
    }
    public static SceneryDefinition forId(int id) { return DEFINITIONS.computeIfAbsent(id, SceneryDefinition::new); }
    public Map<String, Object> getHandlers() { return handlerMap; }
    public int[] getChildrenIds() { return null; }
    public int sizeX = 1, sizeY = 1;
    int walkingFlag;
    String[] definitionOptions;
    public int getWalkingFlag() { return walkingFlag; }
    public String[] getOptions() { return definitionOptions; }
    public static SceneryDefinition oracleShape(int sx, int sy, int flag, String[] options) {
        SceneryDefinition d = new SceneryDefinition(-1);
        d.sizeX = sx; d.sizeY = sy; d.walkingFlag = flag; d.definitionOptions = options;
        return d;
    }
    public static boolean setOptionHandler(String name, core.game.interaction.OptionHandler handler) {
        Support.emit("sceneryOptionHandler", name, Support.label(handler));
        return OPTION_HANDLERS.put(name, handler) != null;
    }
}
''',
    'core/game/node/scenery/Scenery.java': r'''package core.game.node.scenery;
import core.cache.def.impl.SceneryDefinition;
public class Scenery extends core.game.node.Node {
    public int oracleRotation, oracleSizeX = 1, oracleSizeY = 1, oracleWalkingFlag;
    public String[] oracleOptions = new String[0];
    public int getRotation() { return oracleRotation; }
    public SceneryDefinition getDefinition() { return SceneryDefinition.oracleShape(oracleSizeX, oracleSizeY, oracleWalkingFlag, oracleOptions); }
}
''',
    # ------------------------------------------------------------------ components and containers
    'core/game/component/Component.java': r'''package core.game.component;
import core.game.node.entity.player.Player;
public class Component {
    protected int id;
    CloseEvent event;
    public Component(int id) { this.id = id; }
    public int getId() { return id; }
    public void open(Player player) { oracle.Support.emit("componentOpen", id); }
    public CloseEvent getCloseEvent() { return event; }
    public Component setCloseEvent(CloseEvent closeEvent) {
        oracle.Support.emit("setCloseEvent", id, oracle.Support.track(closeEvent));
        this.event = closeEvent;
        return this;
    }
}
''',
    'core/game/component/ComponentDefinition.java': r'''package core.game.component;
import java.util.*;
public class ComponentDefinition {
    public static Map<Integer, ComponentDefinition> DEFINITIONS = new TreeMap<>();
    final int defId;
    ComponentPlugin componentPlugin;
    ComponentDefinition(int id) { defId = id; }
    public static ComponentDefinition forId(int componentId) { return DEFINITIONS.computeIfAbsent(componentId, ComponentDefinition::new); }
    public static void put(int id, ComponentPlugin plugin) { ComponentDefinition.forId(id).setPlugin(plugin); }
    public ComponentPlugin getPlugin() { return componentPlugin; }
    public void setPlugin(ComponentPlugin plugin) { oracle.Support.emit("componentPlugin", defId, oracle.Support.label(plugin)); componentPlugin = plugin; }
}
''',
    'core/game/container/Container.java': r'''package core.game.container;
import core.game.node.item.Item;
import oracle.Support;
public class Container {
    public final String name;
    public Item[] items;
    public Container(String name, int capacity) { this.name = name; items = new Item[capacity]; }
    public Item get(int slot) { Support.emit("get", name, slot); if (slot < 0 || slot >= items.length) return null; return items[slot]; }
    public Item getNew(int slot) { Support.emit("getNew", name, slot); Item item = items[slot]; return item != null ? item : new Item(0); }
    public int itemCount() { Support.emit("itemCount", name); int n = 0; for (Item i : items) if (i != null) n++; return n; }
    public int getId(int slot) { Support.emit("containerId", name, slot); if (slot >= items.length) return -1; Item item = items[slot]; return item != null ? item.getId() : -1; }
    public boolean isEmpty() { Support.emit("isEmpty", name); for (Item i : items) if (i != null) return false; return true; }
    public Item[] toArray() { Support.emit("toArray", name); return items; }
    public int getAmount(Item item) {
        Support.emit("getAmount", name, Support.key(item));
        if (item == null) return 0;
        int count = 0;
        for (Item i : items) if (i != null && i.getId() == item.getId()) count += i.getAmount();
        return count;
    }
    public void refresh() { Support.emit("refresh", name); }
    public boolean containItems(int... ids) { Support.emit("containItems", name, Support.values(ids)); return Support.answerBool("containItems", false); }
    public boolean add(Item item) { Support.emit("add", name, item.getId(), item.getAmount()); return true; }
}
''',
    'core/game/container/impl/EquipmentContainer.java': r'''package core.game.container.impl;
public class EquipmentContainer extends core.game.container.Container {
    public EquipmentContainer(String name, int capacity) { super(name, capacity); }
    public static void update(core.game.node.entity.player.Player player) { oracle.Support.emit("equipmentUpdate"); }
}
''',
    'core/game/container/impl/BankContainer.java': r'''package core.game.container.impl;
import oracle.Support;
public class BankContainer {
    public void addItem(int slot, int amount) { Support.emit("bankAddItem", slot, amount); }
    public void refreshDepositBoxInterface() { Support.emit("refreshDepositBox"); }
}
''',
    'core/game/container/access/InterfaceContainer.java': r'''package core.game.container.access;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import oracle.Support;
public class InterfaceContainer {
    public static int generateItems(Player player, Item[] itemArray, String[] options, int interfaceIndex, int childIndex, int x, int y) {
        Support.emit("generateItems", Support.keys(itemArray), Support.values(options), interfaceIndex, childIndex, x, y, null);
        return 0;
    }
    public static int generateItems(Player player, Item[] itemArray, String[] options, int interfaceIndex, int childIndex, int x, int y, int key) {
        Support.emit("generateItems", Support.keys(itemArray), Support.values(options), interfaceIndex, childIndex, x, y, key);
        return 0;
    }
}
''',
    'core/game/global/action/EquipHandler.java': r'''package core.game.global.action;
public class EquipHandler {
    public static void unequip(core.game.node.entity.player.Player player, int slot, int itemId) { oracle.Support.emit("unequip", slot, itemId); }
}
''',
    # ------------------------------------------------------------------ interaction registries
    'core/game/interaction/Option.java': r'''package core.game.interaction;
public class Option {
    public static final Option _P_ATTACK = new Option("Attack", 0);
    public static final Option _P_FOLLOW = new Option("Follow", 2);
    public static final Option _P_TRADE = new Option("Trade with", 3);
    public static final Option _P_GIVETO = new Option("Give-to", 3);
    public static final Option _P_PICKPOCKET = new Option("Pickpocket", 4);
    public static final Option _P_EXAMINE = new Option("Examine", 7);
    public static final Option _P_ASSIST = new Option("Req Assist", 6);
    public static final Option NULL = new Option("null", 0);
    final String optionName;
    final int optionIndex;
    OptionHandler optionHandler;
    public Option(String name, int index) { optionName = name; optionIndex = index; }
    public String getName() { return optionName; }
    public int getIndex() { return optionIndex; }
    public OptionHandler getHandler() { return optionHandler; }
    public Option setHandler(OptionHandler handler) { oracle.Support.emit("setHandler", optionName, oracle.Support.label(handler)); optionHandler = handler; return this; }
    public void oracleClear() { optionHandler = null; }
    public static Option[] oracleAll() { return new Option[] { _P_ATTACK, _P_FOLLOW, _P_TRADE, _P_GIVETO, _P_PICKPOCKET, _P_EXAMINE, _P_ASSIST, NULL }; }
}
''',
    'core/game/interaction/DestinationFlag.java': r'''package core.game.interaction;
public class DestinationFlag {
    final String flagName;
    DestinationFlag(String name) { flagName = name; }
    public String oracleName() { return flagName; }
    public static final DestinationFlag FOLLOW_ENTITY = new DestinationFlag("FOLLOW_ENTITY");
}
''',
    'core/game/interaction/MovementPulse.java': r'''package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.system.task.Pulse;
import oracle.Support;
public abstract class MovementPulse extends Pulse {
    protected Entity mover;
    protected Node destination;
    public MovementPulse(Entity mover, Node destination, DestinationFlag destinationFlag) {
        super(1, mover, destination);
        this.mover = mover;
        this.destination = destination;
        Support.emit("movementPulse", Support.key(mover), Support.key(destination), destinationFlag.oracleName(), Support.track(this));
    }
    @Override public void stop() { Support.emit("pulseStop", Support.track(this)); super.stop(); }
}
''',
    'core/game/interaction/InteractionListeners.java': r'''package core.game.interaction;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import oracle.Support;
public class InteractionListeners {
    public static boolean run(int id, IntType type, String option, Player player, Node node) {
        if (type == null || option == null || player == null || node == null) throw new NullPointerException();
        Support.emit("listeners", id, type.name(), option, Support.key(node));
        return Support.answerBool("listeners:" + option, false);
    }
}
''',
    'core/game/interaction/InteractionListener.kt': r'''package core.game.interaction

import core.api.ContentInterface
import core.game.node.Node
import core.game.node.entity.player.Player

interface InteractionListener : ContentInterface {
    fun defineListeners()
    fun on(id: Int, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(intArrayOf(id), type.name, arrayOf(*option), handler)
    }
    fun on(ids: IntArray, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(ids, type.name, arrayOf(*option), handler)
    }
    @Deprecated("Don't use") fun on(option: String, type: IntType, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(null, type.name, arrayOf(option), handler)
    }
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(null, type.name, arrayOf(*option), handler)
    }
    fun onUseAnyWith(type: IntType, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        oracle.Support.onUseAnyWith(type.name, with, handler)
    }
    fun defineInteraction(type: IntType, ids: IntArray, vararg options: String, persistent: Boolean = false, allowedDistance: Int = 1, handler: (player: Player, node: Node, state: Int) -> Boolean) {
        oracle.Support.defineInteraction(type.name, ids, arrayOf(*options), persistent, allowedDistance, handler)
    }
}
''',
    'core/game/interaction/InterfaceListeners.kt': r'''package core.game.interaction

import core.game.component.Component
import core.game.node.entity.player.Player

object InterfaceListeners {
    val buttonListeners = java.util.TreeMap<String, (Player, Component, Int, Int, Int, Int) -> Boolean>()
    val openListeners = java.util.TreeMap<String, (Player, Component) -> Boolean>()

    @JvmStatic
    fun add(componentID: Int, buttonID: Int, handler: (Player, Component, Int, Int, Int, Int) -> Boolean) {
        oracle.Support.emit("ifaceOn", componentID, buttonID, oracle.Support.label(handler))
        buttonListeners["$componentID:$buttonID"] = handler
    }

    @JvmStatic
    fun add(componentID: Int, handler: (Player, Component, Int, Int, Int, Int) -> Boolean) {
        oracle.Support.emit("ifaceOn", componentID, null, oracle.Support.label(handler))
        buttonListeners["$componentID"] = handler
    }

    @JvmStatic
    fun addOpenListener(componentID: Int, handler: (Player, Component) -> Boolean) {
        oracle.Support.emit("ifaceOnOpen", componentID, oracle.Support.label(handler))
        openListeners["$componentID"] = handler
    }

    @JvmStatic
    fun addCloseListener(componentID: Int, handler: (Player, Component) -> Boolean) {
        oracle.Support.emit("ifaceOnClose", componentID, oracle.Support.label(handler))
        openListeners["close:$componentID"] = handler
    }

    /** InterfaceListeners.run's lookup, then the handler (the dispatch itself is world/interface_listeners). */
    @JvmStatic
    fun oracleButton(player: Player, component: Component, opcode: Int, buttonID: Int, slot: Int, itemID: Int): Any {
        val method = buttonListeners["${component.id}:$buttonID"] ?: buttonListeners["${component.id}"] ?: return "none"
        return method.invoke(player, component, opcode, buttonID, slot, itemID)
    }

    @JvmStatic
    fun oracleOpen(player: Player, component: Component, close: Boolean): Any {
        val method = openListeners[if (close) "close:${component.id}" else "${component.id}"] ?: return "none"
        return method.invoke(player, component)
    }

    @JvmStatic
    fun oracleReset() { buttonListeners.clear(); openListeners.clear() }
}
''',
    'core/api/ContentInterface.java': 'package core.api;\npublic interface ContentInterface {}\n',
    # ------------------------------------------------------------------ world
    'core/game/system/task/PulseRunner.java': r'''package core.game.system.task;
public class PulseRunner {
    public void submit(Pulse pulse) { oracle.Support.emit("submit", oracle.Support.track(pulse)); }
}
''',
    'core/game/world/GameSettings.java': r'''package core.game.world;
public class GameSettings {
    public boolean isDevMode() { oracle.Support.emit("devMode"); return oracle.Support.answerBool("devMode", false); }
    public int getWorldId() { return oracle.Support.intValue("worldId", 1); }
}
''',
    'core/game/world/GameWorld.kt': r'''package core.game.world

import core.game.system.task.PulseRunner

object GameWorld {
    @JvmStatic
    var settings: GameSettings? = null
    @JvmStatic
    val ticks: Int get() = oracle.Support.intValue("ticks", 0)
    @JvmStatic
    var Pulser = PulseRunner()
}
''',
    'core/game/world/map/Location.java': r'''package core.game.world.map;
public class Location {
    final int lx, ly, lz;
    public Location(int x, int y) { this(x, y, 0); }
    public Location(int x, int y, int z) { lx = x; ly = y; lz = z; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public int getX() { return lx; }
    public int getY() { return ly; }
    public int getZ() { return lz; }
    public Location transform(Direction dir) { return new Location(lx + dir.getStepX(), ly + dir.getStepY(), lz); }
    // Kotlin: Location(x + diffX, y + diffY, (this.z + z) % 4)
    public Location transform(int diffX, int diffY, int z) { return new Location(lx + diffX, ly + diffY, (lz + z) % 4); }
    @Override public boolean equals(Object o) { return o instanceof Location && ((Location) o).lx == lx && ((Location) o).ly == ly && ((Location) o).lz == lz; }
    @Override public int hashCode() { return (lz << 28) | (lx << 14) | ly; }
}
''',
    'core/game/world/repository/Repository.kt': r'''package core.game.world.repository

import core.game.node.entity.npc.NPC
import core.game.world.map.Location

object Repository {
    @JvmStatic
    val disconnectionQueue = DisconnectionQueue()
    @JvmStatic
    fun findNPC(l: Location): NPC? {
        oracle.Support.emit("findNpc", oracle.Support.coord(l))
        return oracle.Support.node(oracle.Support.answer("findNpc:" + l.x + "," + l.y + "," + l.z, null)) as NPC?
    }
}

class DisconnectionQueue {
    fun add(player: core.game.node.entity.player.Player) { oracle.Support.emit("disconnect") }
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
    'core/game/world/update/flag/context/Graphics.java': r'''package core.game.world.update.flag.context;
public class Graphics {
    final int graphicsId;
    Graphics(int id) { graphicsId = id; }
    public static Graphics create(int id) { return new Graphics(id); }
    public int oracleId() { return graphicsId; }
}
''',
    'core/game/system/SystemManager.java': r'''package core.game.system;
public class SystemManager {
    public static SystemConfig getSystemConfig() { return new SystemConfig(); }
}
''',
    'core/game/system/SystemConfig.java': r'''package core.game.system;
public class SystemConfig {
    public boolean validLogin(core.game.node.entity.player.Player player) { oracle.Support.emit("validLogin"); return oracle.Support.answerBool("validLogin", true); }
}
''',
    'core/game/system/timer/TimerManager.java': r'''package core.game.system.timer;
public class TimerManager {
    public RSTimer getTimer(String identifier) {
        oracle.Support.emit("getTimer", identifier);
        return oracle.Support.answerBool("getTimer", false) ? new RSTimer() : null;
    }
}
''',
    'core/game/system/timer/RSTimer.java': 'package core.game.system.timer;\npublic class RSTimer {}\n',
    'core/game/system/communication/ClanRank.java': r'''package core.game.system.communication;
public enum ClanRank { NONE, FRIEND, RECRUIT, CORPORAL, SERGEANT, LIEUTENANT, CAPTAIN, GENERAL, OWNER, ADMINISTRATOR }
''',
    'core/game/system/communication/ClanRepository.java': r'''package core.game.system.communication;
import core.game.node.entity.player.Player;
import oracle.Support;
public class ClanRepository {
    public static void openSettings(Player player) { Support.emit("clanOpenSettings"); }
    public static ClanRepository get(String owner, boolean create) { Support.emit("clanGet", owner, create); return Support.answerBool("clanNull", false) ? null : new ClanRepository(); }
    public String getName() { Support.emit("clanName"); return Support.text("clanName", "Chat disabled"); }
    public void setName(String name) { Support.emit("clanSetName", name); Support.values.put("clanName", name); }
    ClanRank rank(String field) { Support.emit("clanGetReq", field); Object v = Support.values.get("clan:" + field); return v == null ? null : ClanRank.valueOf((String) v); }
    void setRank(String field, ClanRank r) { Support.emit("clanSet", field, r == null ? null : r.name()); Support.values.put("clan:" + field, r == null ? null : r.name()); }
    public ClanRank getJoinRequirement() { return rank("join"); }
    public void setJoinRequirement(ClanRank r) { setRank("join", r); }
    public ClanRank getMessageRequirement() { return rank("message"); }
    public void setMessageRequirement(ClanRank r) { setRank("message", r); }
    public ClanRank getKickRequirement() { return rank("kick"); }
    public void setKickRequirement(ClanRank r) { setRank("kick", r); }
    public ClanRank getLootRequirement() { return rank("loot"); }
    public void setLootRequirement(ClanRank r) { setRank("loot", r); }
    public void updateSettings(Player player) { Support.emit("clanUpdateSettings"); }
    public void update() { Support.emit("clanUpdate"); }
    public void delete() { Support.emit("clanDelete"); }
}
''',
    'core/game/system/communication/CommunicationInfo.java': r'''package core.game.system.communication;
import core.game.node.entity.player.Player;
import oracle.Support;
public class CommunicationInfo {
    public void toggleLootshare(Player player) { Support.emit("toggleLootshare"); }
    public String getClanName() { Support.emit("commClanName"); return Support.text("commClanName", ""); }
    public void setClanName(String name) { Support.emit("commSetClanName", name); Support.values.put("commClanName", name); }
    void set(String field, ClanRank r) { Support.emit("commSet", field, r == null ? null : r.name()); }
    public ClanRank getJoinRequirement() { return null; }
    public ClanRank getMessageRequirement() { return null; }
    public ClanRank getKickRequirement() { return null; }
    public ClanRank getLootRequirement() { return null; }
    public void setJoinRequirement(ClanRank r) { set("join", r); }
    public void setMessageRequirement(ClanRank r) { set("message", r); }
    public void setKickRequirement(ClanRank r) { set("kick", r); }
    public void setLootRequirement(ClanRank r) { set("loot", r); }
}
''',
    'core/game/activity/ActivityManager.java': r'''package core.game.activity;
public class ActivityManager {
    public static boolean start(core.game.node.entity.player.Player player, String name, boolean login, Object... args) {
        oracle.Support.emit("activityStart", name, login);
        return true;
    }
}
''',
    'core/game/bots/AIPlayer.java': 'package core.game.bots;\npublic class AIPlayer extends core.game.node.entity.player.Player {}\n',
    'core/game/bots/AIRepository.java': r'''package core.game.bots;
public class AIRepository {
    public static void sendBotInfo(core.game.node.entity.player.Player player, AIPlayer bot) { oracle.Support.emit("botInfo", oracle.Support.key(bot)); }
}
''',
    'core/game/dialogue/FacialExpression.java': 'package core.game.dialogue;\npublic enum FacialExpression { NEUTRAL, ANNOYED }\n',
    'core/game/dialogue/DialogueInterpreter.java': r'''package core.game.dialogue;
public class DialogueInterpreter {
    public static boolean contains(int id) { oracle.Support.emit("dialogueContains", id); return oracle.Support.answerBool("dialogueContains", false); }
    public core.game.component.Component sendOptions(Object title, String... options) { oracle.Support.emit("sendOptions", title, oracle.Support.values(options)); return null; }
    public void setDialogue(DialoguePlugin dialogue) { oracle.Support.emit("setDialogue", oracle.Support.track(dialogue)); }
}
''',
    'core/game/dialogue/DialoguePlugin.java': r'''package core.game.dialogue;
import core.game.node.entity.player.Player;
public abstract class DialoguePlugin implements core.plugin.Plugin<Player> {
    protected Player player;
    protected DialogueInterpreter interpreter;
    protected int stage;
    public DialoguePlugin() {}
    public DialoguePlugin(Player player) { this.player = player; this.interpreter = player == null ? null : player.getDialogueInterpreter(); }
    public abstract DialoguePlugin newInstance(Player player);
    public abstract boolean open(Object... args);
    public abstract boolean handle(int interfaceId, int buttonId);
    public abstract int[] getIds();
    public void end() { oracle.Support.emit("dialogueEnd"); }
    public Object fireEvent(String identifier, Object... args) { return null; }
}
''',
    'core/game/world/map/RegionManager.java': r'''package core.game.world.map;
import oracle.Support;
public class RegionManager {
    static String at(Location l) { return l.getX() + "," + l.getY() + "," + l.getZ(); }
    public static core.game.node.scenery.Scenery getObject(Location l) {
        Support.emit("getObject", Support.coord(l));
        return (core.game.node.scenery.Scenery) Support.node(Support.answer("object:" + at(l), null));
    }
    public static boolean isTeleportPermitted(Location l) { Support.emit("teleportPermitted", Support.coord(l)); return Support.answerBool("teleport:" + at(l), true); }
    public static int getClippingFlag(int z, int x, int y) { Support.emit("clippingFlag", java.util.Arrays.asList(x, y, z)); return Support.answerInt("clip:" + x + "," + y + "," + z, 0); }
}
''',
    'core/game/world/map/path/ClipMaskSupplier.java': 'package core.game.world.map.path;\npublic interface ClipMaskSupplier { int getClippingFlag(int z, int x, int y); }\n',
    'core/game/world/map/Point.java': 'package core.game.world.map;\npublic class Point { public final int px, py; public Point(int x, int y) { px = x; py = y; } }\n',
    'content/region/kandarin/feldip/quest/zogreflesheaters/ZogreFleshEatersListeners.java': r'''package content.region.kandarin.feldip.quest.zogreflesheaters;
public class ZogreFleshEatersListeners {
    public static void ladderMakesSithikTurnIntoOgre(core.game.node.entity.player.Player player) { oracle.Support.emit("zogreLadder"); }
}
''',
    'core/net/amsc/WorldCommunicator.java': r'''package core.net.amsc;
public class WorldCommunicator {
    public static boolean isEnabled() { oracle.Support.emit("wcEnabled"); return oracle.Support.answerBool("wcEnabled", false); }
}
''',
    'core/net/amsc/MSPacketRepository.java': r'''package core.net.amsc;
import core.game.node.entity.player.Player;
import core.game.system.communication.ClanRank;
public class MSPacketRepository {
    public static void sendClanRename(Player player, String name) { oracle.Support.emit("msClanRename", name); }
    public static void setClanSetting(Player player, int type, ClanRank rank) { oracle.Support.emit("msClanSetting", type, rank == null ? null : rank.name()); }
}
''',
    'core/plugin/Initializable.java': 'package core.plugin;\npublic @interface Initializable {}\n',
    'core/plugin/PluginType.java': 'package core.plugin;\npublic enum PluginType { ACTION, DIALOGUE, LOGIN }\n',
    'core/plugin/PluginManifest.java': r'''package core.plugin;
import java.lang.annotation.*;
@Retention(RetentionPolicy.RUNTIME)
public @interface PluginManifest { PluginType type() default PluginType.ACTION; }
''',
    'core/ServerConstants.java': r'''package core;
public class ServerConstants {
    public static String SERVER_NAME = "2009scape";
    public static boolean BANK_BOOTH_QUICK_OPEN = false;
    public static boolean BANK_BOOTH_NOTE_ENABLED = true;
    public static boolean BANK_BOOTH_NOTE_UIM = true;
}
''',
    'core/tools/Log.kt': 'package core.tools\n\nenum class Log { FINE, INFO, WARN, ERR, DEBUG }\n',
    'core/tools/SystemLogger.kt': 'package core.tools\n\nobject SystemLogger\n',
    'core/tools/DialogueConst.kt': 'package core.tools\n\nconst val END_DIALOGUE = 1000000\n',
    'core/tools/RandomFunction.java': r'''package core.tools;
public class RandomFunction {
    public static int random(int a, int b) { oracle.Support.emit("random", a, b); return oracle.Support.answerInt("random", a); }
}
''',
    'core/tools/StringUtils.java': r'''package core.tools;
public class StringUtils {
    /* ORIGINAL_FORMAT_DISPLAY_NAME */
}
''',
    # ------------------------------------------------------------------ content reached through services
    'content/global/skill/magic/SpellListener.java': r'''package content.global.skill.magic;
public class SpellListener { public static final int NPC = -1, OBJECT = -2, ITEM = -3, PLAYER = -4, NONE = -5, GROUND_ITEM = -6; }
''',
    'content/global/skill/magic/SpellListeners.kt': r'''package content.global.skill.magic

import core.game.node.Node
import core.game.node.entity.player.Player

object SpellListeners {
    @JvmStatic
    fun run(button: Int, type: Int, book: String, player: Player, node: Node? = null) {
        oracle.Support.emit("spellListeners", button, type, book, oracle.Support.key(node))
    }
}
''',
    'content/global/skill/summoning/familiar/Familiar.java': r'''package content.global.skill.summoning.familiar;
public class Familiar extends core.game.node.entity.npc.NPC {
    public boolean isBurdenBeast() { oracle.Support.emit("isBurdenBeast"); return oracle.Support.answerBool("isBurdenBeast", false); }
}
''',
    'content/global/skill/summoning/familiar/BurdenBeast.java': r'''package content.global.skill.summoning.familiar;
public class BurdenBeast extends Familiar {
    public core.game.container.Container getContainer() { return oracle.Support.containers.get("bob"); }
}
''',
    'content/global/skill/summoning/familiar/FamiliarManager.java': r'''package content.global.skill.summoning.familiar;
public class FamiliarManager {
    public boolean hasFamiliar() { oracle.Support.emit("hasFamiliar"); return oracle.Support.answerBool("hasFamiliar", false); }
    public Familiar getFamiliar() { return oracle.Support.answerBool("beastObject", true) ? new BurdenBeast() : new Familiar(); }
}
''',
    'content/region/misc/tutisland/handlers/iface/CharacterDesign.java': r'''package content.region.misc.tutisland.handlers.iface;
public class CharacterDesign {
    public static void handleButtons(core.game.node.entity.player.Player player, int buttonId) { oracle.Support.emit("characterDesign", buttonId); }
}
''',
    'content/global/handlers/iface/tabs/QuestTabUtils.kt': r'''package content.global.handlers.iface.tabs

import core.game.node.entity.player.Player

/** Not ported: the quest requirement tables (core.game.requirement) are content; this records the call. */
object QuestTabUtils {
    @JvmStatic
    fun showRequirementsInterface(player: Player, button: Int) {
        oracle.Support.emit("questRequirements", button)
    }
}
''',
    'content/global/handlers/iface/bank/BankUtils.kt': r'''package content.global.handlers.iface.bank

import core.game.node.entity.player.Player

/** Native (world/input_prompt.rs): records the call. */
object BankUtils {
    fun transferX(player: Player, slot: Int, withdraw: Boolean, after: (() -> Unit)? = null) {
        oracle.Support.emit("transferX", slot, withdraw, oracle.Support.track(after))
    }
}
''',
    'content/global/handlers/npc/BankerNPC.java': r'''package content.global.handlers.npc;
import core.game.node.Node;
import core.game.node.entity.player.Player;
public class BankerNPC extends core.game.node.entity.npc.NPC {
    public static boolean checkLunarIsleRestriction(Player player, Node node) {
        oracle.Support.emit("lunarRestriction", oracle.Support.key(node));
        return oracle.Support.answerBool("lunarRestriction", false);
    }
}
''',
    # ------------------------------------------------------------------ ContentAPI
    'core/api/ContentAPI.kt': r'''package core.api

import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.IronmanMode
import core.game.node.item.Item
import core.game.system.task.Pulse
import core.game.world.map.Location
import core.tools.Log
import oracle.Support

fun sendMessage(player: Player, message: String) { Support.emit("apiMessage", message) }
fun openInterface(player: Player, id: Int) { Support.emit("openInterface", id) }
fun closeInterface(player: Player) { Support.emit("closeInterface") }
fun openSingleTab(player: Player, component: Int) { Support.emit("openSingleTab", component) }
fun closeTabInterface(player: Player) { Support.emit("closeTabInterface") }
fun setInterfaceText(player: Player, string: String, iface: Int, child: Int) { Support.emit("setInterfaceText", string, iface, child) }
fun sendDialogue(player: Player, message: String) { Support.emit("dialogue", message) }
@JvmOverloads
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) { Support.emit("setVarp", varpIndex, value, save) }
@JvmOverloads
fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) { Support.emit("setVarbit", varbitId, value, save) }
@Suppress("UNCHECKED_CAST")
fun <T> getAttribute(entity: Entity, attribute: String, default: T): T {
    Support.emit("apiGetAttribute", Support.key(entity), attribute)
    return if (entity.attrs.containsKey(attribute)) entity.attrs[attribute] as T else default
}
fun <T> setAttribute(entity: Entity, attribute: String, value: T) {
    Support.emit("apiSetAttribute", Support.key(entity), attribute, Support.value(value))
    entity.attrs[attribute] = value
}
fun removeAttribute(entity: Entity, attribute: String) {
    Support.emit("apiRemoveAttribute", Support.key(entity), attribute)
    entity.attrs.remove(attribute)
}
fun removeAttributes(entity: Entity, vararg attributes: String) {
    Support.emit("apiRemoveAttributes", Support.key(entity), Support.values(attributes))
    for (a in attributes) entity.attrs.remove(a)
}
fun runTask(entity: Entity, delay: Int = 0, repeatTimes: Int = 1, task: () -> Unit) {
    Support.emit("runTask", delay, repeatTimes, Support.track(task))
}
fun runWorldTask(task: () -> Unit): Pulse {
    Support.emit("runWorldTask", Support.track(task))
    return Support.NOTHING
}
fun sendInputDialogue(player: Player, numeric: Boolean, prompt: String, handler: (value: Any) -> Unit) {
    Support.emit("inputDialogue", numeric, prompt, Support.track(handler))
}
/* ORIGINAL_RESTRICT_FOR_IRONMAN */
fun anyInEquipment(player: Player, vararg ids: Int): Boolean {
    Support.emit("anyInEquipment", Support.values(ids))
    return Support.answerBool("anyInEquipment", false)
}
fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    val i = item as Item
    Support.emit("removeItem", i.id, i.amount, container.name)
    return Support.answerBool("removeItem", true)
}
fun addItem(player: Player, id: Int, amount: Int = 1, container: Container = Container.INVENTORY): Boolean {
    Support.emit("addItem", id, amount, container.name)
    return Support.answerBool("addItem:" + container.name, true)
}
fun sendDoubleItemDialogue(player: Player, item1: Any, item2: Any, message: String) { Support.emit("doubleItemDialogue", Support.value(item1), Support.value(item2), message) }
fun sendItemDialogue(player: Player, item: Any, message: String) { Support.emit("itemDialogue", Support.value(item), message) }
fun getItemName(id: Int): String { Support.emit("itemName", id); return Support.text("itemName:$id", "item $id") }
@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: Location? = null, radius: Int = 8) { Support.emit("playAudio", id, delay, loops) }
fun log(origin: Class<*>, type: Log, message: String) { Support.emit("log", type.name, message) }
fun amountInInventory(player: Player, id: Int): Int { Support.emit("amountInInventory", id); return Support.answerInt("amountInInventory", 0) }
fun sendNPCDialogue(player: Player, npc: Int, msg: String, expr: core.game.dialogue.FacialExpression = core.game.dialogue.FacialExpression.NEUTRAL, hide: Boolean = false) {
    Support.emit("npcDialogue", npc, msg, expr.name, hide)
}
fun openBankAccount(player: Player) { Support.emit("openBank") }
fun openGrandExchangeCollectionBox(player: Player) { Support.emit("openCollectionBox") }
fun openDepositBox(player: Player) { Support.emit("openDepositBox") }
fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) { Support.emit("animate", Support.key(entity), Support.value(anim), forced) }
fun dumpBeastOfBurden(player: Player) { Support.emit("dumpBob") }
fun openDialogue(player: Player, dialogue: Any, vararg args: Any) { Support.emit("openDialogue", Support.value(dialogue), Support.values(args)) }
fun freeSlots(player: Player): Int { Support.emit("freeSlots"); return Support.answerInt("freeSlots", 0) }
fun hasOption(node: Node, option: String): Boolean { Support.emit("hasOption", Support.key(node), option); return Support.answerBool("hasOption", true) }
''',
}
