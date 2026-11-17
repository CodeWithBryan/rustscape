"""The persisted state of the systems behind save::Host, driven through their whole original classes.

A second compilation of the unmodified PlayerSaver.kt and PlayerSaveParser.kt. Where the first
one (generate.py) holds quests, music, emotes, diaries, pouches and POH storage behind recording
adapters, this one compiles their complete original classes and observes them through their own
accessors. Its cases are appended after the first corpus and never change it.
"""
import json, os, pathlib, random, re, shutil, subprocess, tempfile

HERE = pathlib.Path(__file__).resolve().parent

# Whole original classes this build adds to the ones generate.py already compiles complete.
WHOLE_JAVA = [
    'core/game/node/entity/player/link/quest/QuestRepository.java',
    'core/game/node/entity/player/link/quest/Quest.java',
    'core/game/node/entity/player/link/music/MusicPlayer.java',
    'core/game/node/entity/player/link/emote/EmoteManager.java',
    'core/game/node/entity/player/link/diary/AchievementDiaryManager.java',
    'core/game/node/entity/player/link/diary/AchievementDiary.java',
    'core/game/node/entity/player/link/diary/DiaryType.java',
    'core/plugin/Plugin.java', 'core/plugin/PluginManifest.java', 'core/plugin/PluginType.java',
    'core/game/component/CloseEvent.java',
    'content/global/skill/smithing/smelting/Bar.java',
]
WHOLE_KOTLIN = [
    'content/global/skill/runecrafting/PouchManager.kt',
    'content/global/skill/construction/decoration/pohstorage/StorageState.kt',
    'content/global/skill/construction/decoration/pohstorage/StorageContainer.kt',
    'content/global/skill/construction/decoration/pohstorage/StorableFamily.kt',
    'content/data/Quests.kt',
    'core/game/diary/DiaryLevel.kt',
]
# Adapters of the first build that a whole original class replaces here.
REPLACED = [
    'core/game/node/entity/player/link/quest/QuestRepository.java',
    'core/game/node/entity/player/link/music/MusicPlayer.java',
    'core/game/node/entity/player/link/emote/EmoteManager.java',
    'core/game/node/entity/player/link/diary/AchievementDiary.java',
    'core/game/node/entity/player/link/diary/AchievementDiaryManager.java',
    'core/game/node/entity/player/link/diary/DiaryType.java',
    'content/global/skill/runecrafting/PouchManager.java',
    'content/global/skill/construction/decoration/pohstorage/StorageState.java',
    'content/data/Quests.java',
    'core/api/ContentAPIKt.java',
    'core/api/ProfileLog.kt',
]
SCOPE = (' Appended state cases: a second compilation of the unmodified PlayerSaver.kt and PlayerSaveParser.kt over the complete unmodified QuestRepository, Quest, Quests, MusicPlayer, MusicEntry, EmoteManager, '
    'AchievementDiaryManager, AchievementDiary, DiaryType, DiaryLevel, Bar, PouchManager, StorageState, StorageContainer, StorableFamily, SpellBookManager, Plugin, PluginManifest, PluginType and CloseEvent. '
    'Verbatim selected members: GraveController (ATTR_GTYPE, getGraveType, updateGraveType), ContentAPI (setAttribute, getAttribute, setVarp), Entity (setAttribute, getAttribute, removeAttribute), '
    'HouseManager (location, style, crest, rooms and servant fields with their getters and setters) and EquipmentContainer slot constants. Emotes, HouseLocation, HousingStyle and CrestType are generated '
    'with the original constant names and order only; org.rs09.consts.Items constants take the number that ends their name. Familiars, pets, house loading and content hooks stay recording adapters. '
    'Packet dispatch, interface manager, packets, GameWorld, NPCDefinition, events, the remaining ContentAPI functions and Skills.getLevel are stand-ins outside the driven paths; container listeners record. '
    'Quest registration, songs, spells and item definitions are scripted oracle inputs, not the pinned content.')


def adapters(gen):
    out = gen.adapters()
    for name in REPLACED:
        del out[name]
    src = lambda path: gen.read(gen.SERVER / path)
    method, members, field, enum_names = gen.method, gen.members, gen.field, gen.enum_names
    api = src('core/api/ContentAPI.kt')
    out['core/api/ContentAPI.kt'] = ('package core.api\nimport core.game.node.entity.Entity\nimport core.game.node.entity.player.Player\n'
        '/** PlayerSaver, PlayerSaveParser and QuestRepository log calls. */\n'
        'fun log(origin: Class<*>, type: Int, message: String) { oracle.Rec.call("log", if (type == core.tools.Log.ERR) "ERR" else "WARN", message) }\n'
        '/** The item classes report a failed definition lookup here; no case may reach it. */\n'
        'fun log(origin: Any?, type: Int, message: String) { throw AssertionError(message) }\n'
        + method(api, 'fun <T> setAttribute(entity: Entity, attribute: String, value: T)') + '\n'
        + method(api, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T') + '\n'
        + '@JvmOverloads\n' + method(api, 'fun setVarp (player: Player, varpIndex: Int, value: Int, save: Boolean = false)') + '\n'
        '// Gameplay functions the whole classes name outside their save and parse methods.\n'
        'private fun outside(name: String): Nothing = throw UnsupportedOperationException("$name is outside the player state oracle")\n'
        '@JvmOverloads fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) { outside("setVarbit") }\n'
        'fun playJingle(player: Player, jingleId: Int) { outside("playJingle") }\n'
        'fun sendMessage(player: Player, message: String) { outside("sendMessage") }\n'
        'fun <T> removeItem(player: Player, item: T): Boolean = outside("removeItem")\n'
        'fun replaceAllItems(player: Player, itemId: Int, replaceId: Int) { outside("replaceAllItems") }\n'
        'fun freeSlots(player: Player): Int = outside("freeSlots")\n'
        'fun addItem(player: Player, id: Int, amount: Int = 1): Boolean = outside("addItem")\n')
    out['core/game/event/Events.kt'] = ('package core.game.event\nimport core.game.node.entity.Entity\ninterface Event\n'
        + ''.join(re.search(r'^data class ' + name + r'\([^\n]*', src('core/game/event/Events.kt'), re.M).group(0) + '\n' for name in ['AttributeSetEvent', 'AttributeRemoveEvent']))
    entity = src('core/game/node/entity/Entity.java')
    text = out['core/game/node/entity/Entity.java'].replace('package core.game.node.entity;\n', 'package core.game.node.entity;\nimport core.game.event.*;\n')
    out['core/game/node/entity/Entity.java'] = (text[:text.rindex('}')]
        + method(entity, 'public void setAttribute(String key, Object value)') + '\n'
        + method(entity, 'public <T> T getAttribute(String string, T fail)') + '\n'
        + method(entity, 'public void removeAttribute(String string)') + '\n'
        + '    public void dispatch(Event event) { oracle.Rec.event(event); }\n}\n')
    grave = src('core/game/node/entity/combat/graves/GraveController.kt')
    out['core/game/node/entity/combat/graves/GraveController.kt'] = ('package core.game.node.entity.combat.graves\nimport core.api.*\nimport core.game.node.entity.player.Player\n'
        '/** The three members the save parser reaches; the original holds them in its companion object. */\nobject GraveController {\n'
        + re.search(r'^\s*val ATTR_GTYPE = [^\n]*', grave, re.M).group(0) + '\n'
        + '    @JvmStatic ' + method(grave, 'fun getGraveType(player: Player) : GraveType') + '\n'
        + '    @JvmStatic ' + method(grave, 'fun updateGraveType(player: Player, type: GraveType)') + '\n}\n')
    # Constant names end in the item id, as generate.py already assumes for NPCs.
    items = set()
    for name in WHOLE_JAVA + WHOLE_KOTLIN:
        items.update(re.findall(r'\bItems\.([A-Z_0-9]+)', src(name)))
    for name in sorted(items):
        if not re.search(r'_(\d+)$', name):
            raise RuntimeError('Item constant without an id suffix: ' + name)
    out['org/rs09/consts/Items.java'] = ('package org.rs09.consts;\npublic class Items {\n    public static int MAGIC_CARPET_5614 = 5614;\n'
        + ''.join('    public static final int ' + name + ' = ' + re.search(r'_(\d+)$', name).group(1) + ';\n' for name in sorted(items)) + '}\n')
    emotes = src('core/game/node/entity/player/link/emote/Emotes.java')
    out['core/game/node/entity/player/link/emote/Emotes.java'] = 'package core.game.node.entity.player.link.emote;\npublic enum Emotes { ' + ', '.join(enum_names(emotes, r'^    ([A-Z][A-Z_0-9]*)\(', 45)) + ' }\n'
    house = src('content/global/skill/construction/HouseManager.java')
    out['content/global/skill/construction/HouseManager.java'] = ('package content.global.skill.construction;\nimport org.json.simple.JSONObject;\npublic final class HouseManager {\n'
        + members(house, ['location', 'style', 'rooms', 'servant', 'crest'], ['public boolean hasServant()', 'public Room[][][] getRooms()', 'public HouseLocation getLocation()', 'public void setLocation(',
            'public HousingStyle getStyle()', 'public void setStyle(', 'public Servant getServant()', 'public void setServant(', 'public CrestType getCrest()', 'public void setCrest(']) +
        '    public HouseManager() {}\n    public void parse(JSONObject data) { oracle.Rec.call("house.parse", data.toJSONString()); }\n}\n')
    for name, path, pattern, count in [('HouseLocation', 'HouseLocation.java', r'^\t([A-Z_]+)\(', 7), ('HousingStyle', 'HousingStyle.java', r'^\t([A-Z_]+)\s*\(\d', 6), ('CrestType', 'CrestType.kt', r'^    ([A-Z_]+)\(', 17)]:
        out['content/global/skill/construction/' + name + '.java'] = ('package content.global.skill.construction;\n/** Constant names and order only. */\npublic enum ' + name + ' { '
            + ', '.join(enum_names(src('content/global/skill/construction/' + path), pattern, count)) + ' }\n')
    equipment = src('core/game/container/impl/EquipmentContainer.java')
    text = out['core/game/container/impl/EquipmentContainer.java']
    out['core/game/container/impl/EquipmentContainer.java'] = text[:text.rindex('}')] + re.search(r'public static final int SLOT_HAT[^;]*;', equipment).group(0) + '}'
    text = out['core/game/node/entity/skill/Skills.java']
    out['core/game/node/entity/skill/Skills.java'] = (text[:text.rindex('}')]
        + '    /** Stand-in for the assist-aware original; only PouchManager.checkRequirement names it and no case calls that. */\n    public int getLevel(int slot) { return dynamicLevels[slot]; }\n}\n')
    text = out['core/game/node/entity/player/Player.java'].replace(
        '    public InterfaceManager getInterfaceManager() { throw new UnsupportedOperationException(); }\n',
        '    private final InterfaceManager interfaceManager = new InterfaceManager();\n    public InterfaceManager getInterfaceManager() { return interfaceManager; }\n')
    if 'interfaceManager = new' not in text:
        raise RuntimeError('Player adapter changed')
    out['core/game/node/entity/player/Player.java'] = text[:text.rindex('}')] + '''    private final core.game.node.entity.player.link.PacketDispatch packetDispatch = new core.game.node.entity.player.link.PacketDispatch();
    public core.game.node.entity.player.link.PacketDispatch getPacketDispatch() { return packetDispatch; }
    public boolean isDebug() { return false; }
    public boolean isArtificial() { return false; }
    public boolean hasItem(core.game.node.item.Item item) { throw new UnsupportedOperationException("Player.hasItem is outside the player state oracle"); }
    public void sendMessages(String... messages) { oracle.Rec.call("messages", java.util.Arrays.asList(messages)); }
    public void debug(String text) { oracle.Rec.call("debug", text); }
}
'''
    out.update({
'core/game/node/entity/player/link/PacketDispatch.java': '''package core.game.node.entity.player.link;
/** Records what the whole classes send; nothing is encoded. */
public class PacketDispatch {
    public void sendVarp(int index, int value) { oracle.Rec.call("varp", index, value); }
    public void sendString(String text, int component, int child) { oracle.Rec.call("string", text, component, child); }
    public void sendMessage(String text) { oracle.Rec.call("message", text); }
    public void sendRunScript(int id, String types, Object... values) { oracle.Rec.call("script", id, types); }
    public void sendIfaceSettings(int settings, int child, int component, int start, int end) { oracle.Rec.call("ifaceSettings", settings, child, component, start, end); }
}''',
'core/game/node/entity/player/InterfaceManager.java': '''package core.game.node.entity.player;
import core.game.component.Component;
public class InterfaceManager {
    public Component open(Component component) { oracle.Rec.call("iface.open"); return component; }
    public void openTab(Component component) { oracle.Rec.call("iface.tab"); }
    public void openTab(int slot, Component component) { oracle.Rec.call("iface.tab", slot); }
    public boolean isOpened() { oracle.Rec.call("iface.isOpened"); return false; }
}''',
'core/game/component/Component.java': '''package core.game.component;
public class Component {
    public Component(int id) {}
    public Component setCloseEvent(CloseEvent event) { return this; }
}''',
'core/net/packet/PacketRepository.java': '''package core.net.packet;
import core.net.packet.context.*;
public class PacketRepository {
    public static void send(Class<?> type, SkillContext context) { oracle.Rec.call("packet", context.skill); }
    public static void send(Class<?> type, MusicContext context) { oracle.Rec.call("music.packet", context.id); }
    public static void send(Class<?> type, StringContext context) { oracle.Rec.call("string.packet", context.text); }
}''',
'core/net/packet/context/MusicContext.java': '''package core.net.packet.context;
import core.game.node.entity.player.Player;
public class MusicContext {
    public final int id;
    public MusicContext(Player player, int id) { this.id = id; }
    public MusicContext(Player player, int id, boolean secondary) { this.id = id; }
}''',
'core/net/packet/context/StringContext.java': '''package core.net.packet.context;
import core.game.node.entity.player.Player;
public class StringContext {
    public final String text;
    public StringContext(Player player, String text, int component, int child) { this.text = text; }
}''',
'core/net/packet/out/MusicPacket.java': 'package core.net.packet.out;\npublic class MusicPacket {}',
'core/net/packet/out/StringPacket.java': 'package core.net.packet.out;\npublic class StringPacket {}',
'core/game/world/GameWorld.java': '''package core.game.world;
public class GameWorld {
    public static int getTicks() { throw new UnsupportedOperationException("GameWorld.getTicks is outside the player state oracle"); }
}''',
'core/cache/def/impl/NPCDefinition.java': '''package core.cache.def.impl;
public class NPCDefinition {
    public static NPCDefinition forId(int id) { throw new UnsupportedOperationException("NPC definitions are outside the player state oracle"); }
    public String getName() { throw new UnsupportedOperationException(); }
}''',
'core/game/container/impl/RecordingListener.java': '''package core.game.container.impl;
import core.game.container.*;
/** Stands in for the three packet-sending listeners: records which one a container notified. */
public class RecordingListener implements ContainerListener {
    public void update(Container c, ContainerEvent e) { oracle.Rec.call("listener", getClass().getSimpleName(), "update"); }
    public void refresh(Container c) { oracle.Rec.call("listener", getClass().getSimpleName(), "refresh"); }
}''',
    })
    missing = [name for name, text in out.items() if text is None]
    if missing:
        raise RuntimeError('Unfilled adapters: ' + ', '.join(missing))
    return out


def build(gen):
    """Compile the second oracle next to the first; returns its runtime classpath."""
    compiler, read, run = gen.compiler, gen.read, gen.run
    classes = gen.RUN / 'state-classes'
    folder = gen.RUN / 'state-adapters'
    shutil.rmtree(classes, ignore_errors=True)
    shutil.rmtree(folder, ignore_errors=True)
    classes.mkdir()
    jars = compiler.artifacts()
    jar = gen.timers.RUN / 'json-simple-1.1.1.jar'
    runtime = [p for p in jars if p.name.startswith(('kotlin-stdlib-', 'kotlin-reflect-'))]
    cp = os.pathsep.join(map(str, [classes, gen.timers.RUN / 'classes', gen.inventory.RUN / 'classes', jar, *runtime]))
    kotlin, java = [], []
    for name, text in adapters(gen).items():
        path = folder / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if name.endswith('.kt') else java).append(path)
    for name in gen.ORIGINAL_KOTLIN + WHOLE_KOTLIN:
        read(gen.SERVER / name)
        kotlin.append(gen.SERVER / name)
    for name in gen.ORIGINAL_JAVA + WHOLE_JAVA:
        read(gen.SERVER / name)
        java.append(gen.SERVER / name)
    for name in ['Coroutines.kt', 'State.kt']:
        read(HERE / name)
        kotlin.append(HERE / name)
    run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-nowarn', '-jvm-target', '11', '-classpath', cp, '-d', classes, *kotlin, *java])
    run(['javac', '--release', '11', '-nowarn', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    return cp


def evaluate(gen, cp, cases):
    file = gen.RUN / 'state-input.jsonl'
    file.write_bytes(''.join(json.dumps(c, sort_keys=True) + '\n' for c in cases).encode())
    home = gen.compiler.JAVA_HOME
    executable = home / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if home else 'java'
    with tempfile.TemporaryDirectory(prefix='state-files-', dir=gen.RUN) as temp:
        assert pathlib.Path(temp).resolve().parent == gen.RUN.resolve()
        with file.open('rb') as stdin:
            result = subprocess.run(list(map(str, [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-cp', cp, 'oracle.StateKt', temp])), cwd=gen.ROOT, stdin=stdin, capture_output=True, timeout=600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:])
    output = [json.loads(line) for line in result.stdout.splitlines() if line]
    if len(output) != len(cases):
        raise RuntimeError('Missing player state cases')
    return output


def generate(gen):
    """The appended cases, each marked with "state": true."""
    import state_corpus
    read = gen.read
    # Quests.toString() is the quest name; the repository's log lines print it.
    rows = re.findall(r'^    ([A-Z][A-Z_0-9]*)\("([^"\\$]*)"\)[,;]$', read(gen.SERVER / 'content/data/Quests.kt'), re.M)
    constants = re.findall(r'^    ([A-Z][A-Z_0-9]*)\(', read(gen.SERVER / 'content/data/Quests.kt'), re.M)
    if [c for c, _ in rows] != constants or len(constants) < 100:
        raise RuntimeError('Quests constants changed; review the state corpus')
    state_corpus.QUEST_NAMES[:] = [name for _, name in rows]
    cp = build(gen)
    seed = evaluate(gen, cp, state_corpus.seed_requests())
    output = evaluate(gen, cp, state_corpus.requests(seed))
    read(pathlib.Path(__file__))
    read(HERE / 'state_corpus.py')
    return output
