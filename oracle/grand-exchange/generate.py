"""Run the unchanged Grand Exchange classes on JDK 11 over a real SQLite file and record what they do.

Compiled unchanged: core/game/ge (GrandExchange.kt, GrandExchangeOffer.kt,
GrandExchangeRecords.kt, GrandExchangeTimer.kt, GEDB.kt, PriceIndex.kt,
GEGuidePrice.java, GEItemSet.java, OfferState.java, BotPrices.kt,
GEAutoStock.kt), content/global/handlers/iface/ge (StockMarket.kt,
GrandExchangeInterface.java, ExchangeItemSets.kt), InterfaceContainer.java,
GrandExchangePacket.java with its context, LoginListener, PersistPlayer,
StartupListener, InterfaceListener/InterfaceListeners, IntType,
ContentInterface, IfaceSettingsBuilder, InputType, RunScript, IronmanManager,
Definition, Item, Container with its event/listener/type/sort classes,
Component, ComponentDefinition, ComponentPlugin, CloseEvent, InterfaceType,
Plugin, Initializable, the complete InterfaceManager, ClientInfo, IoBuffer and
the packet writers with their contexts. SQLiteProvider is its original text
with the coroutine members removed and the connection wrapped by a recording
proxy. Everything else is an adapter under oracle/grand-exchange/adapters, with
the original text of the members named in SCOPE inserted at generation time.
The pinned sqlite-jdbc 3.36.0.3 jar (oracle/world-trace/artifacts.lock.json)
runs the database. Launcher.java runs each case in its own class loader.
--check never updates a fixture, and nothing here downloads: the pinned jars
must already be under runs/.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/grand-exchange-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/grand-exchange'
CONFIG = ROOT / 'fixtures/interfaces/interface_configs.json'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
cases = load('ge_cases', HERE / 'cases.py')

GE = [
    'core/game/ge/GrandExchange.kt',
    'core/game/ge/GrandExchangeOffer.kt',
    'core/game/ge/GrandExchangeRecords.kt',
    'core/game/ge/GrandExchangeTimer.kt',
    'core/game/ge/GEDB.kt',
    'core/game/ge/PriceIndex.kt',
    'core/game/ge/GEGuidePrice.java',
    'core/game/ge/GEItemSet.java',
    'core/game/ge/OfferState.java',
    'core/game/ge/BotPrices.kt',
    'core/game/ge/GEAutoStock.kt',
    'content/global/handlers/iface/ge/StockMarket.kt',
    'content/global/handlers/iface/ge/GrandExchangeInterface.java',
    'content/global/handlers/iface/ge/ExchangeItemSets.kt',
]
ORIGINAL = GE + [
    'core/game/container/access/InterfaceContainer.java',
    'core/net/packet/out/GrandExchangePacket.java',
    'core/net/packet/context/GrandExchangeContext.java',
    'core/api/LoginListener.kt',
    'core/api/PersistPlayer.kt',
    'core/api/StartupListener.kt',
    'core/game/interaction/InterfaceListener.kt',
    'core/game/interaction/InterfaceListeners.kt',
    'core/game/interaction/IntType.kt',
    'core/api/ContentInterface.kt',
    'core/api/IfaceSettingsBuilder.kt',
    'core/api/InputType.kt',
    'core/net/packet/in/RunScript.kt',
    'core/tools/DialogueConst.kt',
    'core/game/node/entity/player/link/IronmanManager.java',
    'core/game/node/entity/player/link/IronmanMode.java',
    'core/cache/def/Definition.java',
    'core/game/node/item/Item.java',
    'core/game/container/Container.java',
    'core/game/container/ContainerEvent.java',
    'core/game/container/ContainerListener.java',
    'core/game/container/ContainerType.java',
    'core/game/container/SortType.java',
    'core/game/component/Component.java',
    'core/game/component/ComponentDefinition.java',
    'core/game/component/ComponentPlugin.java',
    'core/game/component/CloseEvent.java',
    'core/game/component/InterfaceType.java',
    'core/plugin/Plugin.java',
    'core/plugin/Initializable.java',
    'core/game/node/entity/player/link/InterfaceManager.java',
    'core/game/node/entity/player/info/ClientInfo.java',
    'core/net/packet/IoBuffer.java',
    'core/net/packet/PacketHeader.java',
    'core/net/packet/Context.java',
    'core/net/packet/OutgoingPacket.java',
    'core/net/packet/out/WindowsPane.java',
    'core/net/packet/out/Interface.java',
    'core/net/packet/out/CloseInterface.java',
    'core/net/packet/out/InterfaceConfig.java',
    'core/net/packet/out/Config.java',
    'core/net/packet/out/RunScriptPacket.java',
    'core/net/packet/out/StringPacket.java',
    'core/net/packet/out/AccessMask.java',
    'core/net/packet/out/ContainerPacket.java',
    'core/net/packet/context/WindowsPaneContext.java',
    'core/net/packet/context/InterfaceContext.java',
    'core/net/packet/context/InterfaceConfigContext.java',
    'core/net/packet/context/ConfigContext.java',
    'core/net/packet/context/RunScriptContext.java',
    'core/net/packet/context/StringContext.java',
    'core/net/packet/context/AccessMaskContext.java',
    'core/net/packet/context/ContainerContext.java',
]
SCOPE = (
    'Unchanged core/game/ge (GrandExchange with its start-up listener, worker loop body, matching, dispatch, bot offers, queries and command '
    'definitions; GrandExchangeOffer; GrandExchangeRecords with the login/save/parse hooks, collection box, history log and init; '
    'GrandExchangeTimer.run; GEDB; PriceIndex/PriceInfo; GEGuidePrice; GEItemSet; OfferState; BotPrices; GEAutoStock), '
    'content/global/handlers/iface/ge (StockMarket, GrandExchangeInterface, ExchangeItemSets), InterfaceContainer, GrandExchangePacket, '
    'LoginListener, PersistPlayer, StartupListener, InterfaceListener/InterfaceListeners, IntType, ContentInterface, IfaceSettingsBuilder, '
    'InputType, RunScript.processInput, DialogueConst, IronmanManager/IronmanMode, Definition, Item, Container with ContainerEvent/'
    'ContainerListener/ContainerType/SortType, Component, ComponentDefinition, ComponentPlugin, CloseEvent, InterfaceType, Plugin, '
    'Initializable, the complete InterfaceManager, ClientInfo, IoBuffer and the WindowsPane, Interface, CloseInterface, InterfaceConfig, '
    'Config, RunScriptPacket, StringPacket, AccessMask and ContainerPacket writers with their contexts, on JDK 11 with sqlite-jdbc 3.36.0.3, '
    'each case in a fresh class loader over its own database file, with the pinned interface_configs.json. Original text inserted at '
    'generation time: SQLiteProvider (initTables, run, connect, TABLE_CHECK; the coroutine members runAsync/pruneOldData are left out and '
    'the connection is wrapped by a proxy that records each executeQuery/executeUpdate/execute with its bound parameters); '
    'PacketProcessor.process\'s GESetOfferItem branch; ContentAPI getAttribute, setAttribute, removeAttribute, openInterface, '
    'closeInterface, inInventory, amountInInventory, hasSpaceFor, itemDefinition, getItemName, both sendInputDialogue forms and '
    'hasAwaitingGrandExchangeCollections; PacketDispatch sendString, sendIfaceSettings, sendRunScript and sendInterfaceConfig; '
    'StringUtils.formatDisplayName; Location.transform; Node and the ItemDefinition members the classes reach; InterfaceConfigParser '
    'with a string reader. ContentAPI removeItem/addItem keep their INVENTORY branch. Recording adapters: Player (attributes as a map, '
    'uid, artificial flag and bank PIN state from the case), PlayerDetails (getDetails), the session (fixed mode, zero ISAAC), '
    'Player.sendMessage, PacketDispatch.sendMessage after its null return, ContentAPI sendMessage/notify/setVarp/playAudio/playJingle/log, '
    'SystemLogger.logGE, Repository (players, uid_map, sendGrandExchangeNews), Discord.postNewOffer, CS2Mapping.forId, '
    'ItemDefinition.getExamine, GameWorld.Pulser.submit, the Commands definitions (bodies run by the case), RSTimer\'s constructor, '
    'ServerConstants from the case, System.currentTimeMillis of core.game.ge (a case clock with a step per read) and an inventory listener '
    'that records update slots and refreshes. Modelled in Support.java: the tail of processIfAction (component lookup, hidden check, '
    'ButtonClickEvent, InterfaceListeners.run, plugin), the InputPromptResponse branch around RunScript.processInput, the worker thread '
    '(the body boot() starts runs on the case thread at each "worker" action until pendingOffers is empty: pendingOffers is replaced at '
    'class load by a deque whose takeFirst ends the step instead of blocking; an exception ends the thread for good), pulses run at '
    'each "pulse" action, and GEAutoStock\'s relative file under the JVM\'s working directory. Not exercised: concurrent execution of the '
    'worker and the world thread, the SQLiteProvider lock timeouts, Discord webhooks, the real InventoryListener, GameMessage bytes and the '
    'real item cache (definitions are authored inputs).'
)


def markers():
    def source(name):
        return c.read(SERVER / name)

    location = source('core/game/world/map/Location.kt')
    dispatch = source('core/game/node/entity/player/link/PacketDispatch.java')
    content = source('core/api/ContentAPI.kt')
    strings = source('core/tools/StringUtils.java')
    processor = source('core/net/packet/PacketProcessor.kt')
    branch = c.method(processor, 'is Packet.GESetOfferItem ->')
    return {
        'ORIGINAL_TRANSFORM': re.search(r'^\s*fun transform\(diffX: Int, diffY: Int, z: Int\): Location = .*$', location, re.M).group().strip(),
        'ORIGINAL_SEND_STRING': c.method(dispatch, 'public void sendString(String string, int interfaceId, int lineId)'),
        'ORIGINAL_SEND_IFACE_SETTINGS': c.method(dispatch, 'public void sendIfaceSettings('),
        'ORIGINAL_SEND_RUN_SCRIPT': c.method(dispatch, 'public void sendRunScript(int id, String string, Object... objects)'),
        'ORIGINAL_SEND_INTERFACE_CONFIG': c.method(dispatch, 'public void sendInterfaceConfig(int interfaceId, int childId, boolean hide)'),
        'ORIGINAL_GET_ATTRIBUTE': c.method(content, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T'),
        'ORIGINAL_SET_ATTRIBUTE': c.method(content, 'fun <T> setAttribute(entity: Entity, attribute: String, value: T)'),
        'ORIGINAL_REMOVE_ATTRIBUTE': c.method(content, 'fun removeAttribute(entity: Entity, attribute: String)'),
        'ORIGINAL_OPEN_INTERFACE': c.method(content, 'fun openInterface(player: Player, id: Int)'),
        'ORIGINAL_CLOSE_INTERFACE': c.method(content, 'fun closeInterface(player: Player)'),
        'ORIGINAL_IN_INVENTORY': c.method(content, 'fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean'),
        'ORIGINAL_AMOUNT_IN_INVENTORY': c.method(content, 'fun amountInInventory(player: Player, id: Int): Int'),
        'ORIGINAL_HAS_SPACE_FOR': c.method(content, 'fun hasSpaceFor(player: Player, item: Item): Boolean'),
        'ORIGINAL_ITEM_DEFINITION': c.method(content, 'fun itemDefinition(id: Int): ItemDefinition'),
        'ORIGINAL_GET_ITEM_NAME': c.method(content, 'fun getItemName(id: Int): String'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC': c.method(content, 'fun sendInputDialogue(player: Player, numeric: Boolean'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_TYPE': c.method(content, 'fun sendInputDialogue(player: Player, type: InputType'),
        'ORIGINAL_HAS_AWAITING_COLLECTIONS': c.method(content, 'fun hasAwaitingGrandExchangeCollections(player: Player): Boolean'),
        'ORIGINAL_FORMAT_DISPLAY_NAME': c.method(strings, 'public static String formatDisplayName(String name)'),
        'ORIGINAL_GE_SET_OFFER_ITEM': branch,
    }


def constants(texts, kind):
    names = set(re.findall(kind + r'\.(\w+_\d+)\b', ''.join(texts)))
    return ('package org.rs09.consts;public class ' + kind + '{'
            + ''.join(f'public static final int {name}={int(name.rsplit("_", 1)[1])};' for name in sorted(names)) + '}')


def generated_adapters():
    """Adapters built from original text (the inventory, interfaces, shops and trade oracles' method)."""
    out = {}
    definition = c.read(SERVER / 'core/cache/def/impl/ItemDefinition.java')
    body = definition[definition.index('private static final Map<Integer, ItemDefinition> DEFINITIONS'):definition.index('public ItemDefinition()')]
    methods = ['public ItemDefinition()', 'public static ItemDefinition forId(', 'public static Map<Integer, ItemDefinition> getDefinitions()',
               'public boolean isStackable()', 'public int getValue()', 'public int getAlchemyValue(', 'public int getNoteId()',
               'public boolean isUnnoted()', 'public ItemPlugin getItemPlugin()', 'public static OptionHandler getOptionHandler(',
               'public boolean isTradeable()', 'public boolean hasAction(String optionName)', 'public boolean hasDestroyAction()']
    body += '\n'.join(c.method(definition, m) for m in methods)
    body += '\npublic String getExamine(){return oracle.Support.examine(this);}'
    out['core/cache/def/impl/ItemDefinition.java'] = (
        'package core.cache.def.impl;import java.util.*;import core.cache.def.Definition;import core.game.node.item.*;'
        'import core.game.interaction.OptionHandler;import core.game.system.config.ItemConfigParser;import core.tools.Log;'
        'import org.rs09.consts.Items;import static core.api.ContentAPIKt.log;public class ItemDefinition extends Definition<Item>{' + body + '}')
    node = c.read(SERVER / 'core/game/node/Node.java')
    node_methods = ['public Node(', 'public String getName()', 'public int getIndex()', 'public void setIndex(', 'public DestinationFlag getDestinationFlag()',
                    'public Location getLocation()', 'public Direction getDirection()']
    out['core/game/node/Node.java'] = (
        'package core.game.node;import core.game.interaction.*;import core.game.world.map.Location;import core.game.world.map.Direction;'
        'public class Node {protected String name;protected int index;protected Location location;protected InteractPlugin interactPlugin;'
        'protected DestinationFlag destinationFlag;protected Direction direction;'
        + ''.join(c.method(node, m) for m in node_methods)
        + 'public int getId(){return this instanceof core.game.node.entity.npc.NPC ? ((core.game.node.entity.npc.NPC) this).getId() : '
          'this instanceof core.game.node.item.Item ? ((core.game.node.item.Item) this).getId() : -1;}}')
    config = c.read(SERVER / 'core/game/system/config/ItemConfigParser.kt')
    names = ['HIGH_ALCHEMY', 'LOW_ALCHEMY', 'TRADEABLE']
    constants_text = ''.join('public static String ' + name + '=' + re.search(r'const val ' + name + r' = ("[^"]+")', config).group(1) + ';' for name in names)
    out['core/game/system/config/ItemConfigParser.java'] = 'package core.game.system.config;public class ItemConfigParser {' + constants_text + '}'
    parser = c.read(SERVER / 'core/game/system/config/InterfaceConfigParser.kt')
    replaced = parser.replace('var reader: FileReader? = null', 'var reader: java.io.Reader? = null').replace(
        'FileReader(ServerConstants.CONFIG_PATH + "interface_configs.json")', 'java.io.StringReader(oracle.Support.config)')
    if replaced.count('oracle.Support.config') != 1 or 'java.io.Reader?' not in replaced:
        raise RuntimeError('InterfaceConfigParser text differs from the expected reader lines')
    out['core/game/system/config/InterfaceConfigParser.kt'] = replaced
    provider = c.read(SERVER / 'core/integrations/sqlite/SQLiteProvider.kt')
    for member in ['fun pruneOldData (', 'fun runAsync (']:
        provider = provider.replace(c.method(provider, member), '')
    for line in ['import kotlinx.coroutines.CoroutineScope\n', 'import kotlinx.coroutines.Dispatchers\n', 'import kotlinx.coroutines.Job\n',
                 'import kotlinx.coroutines.launch\n', '    private val coroutineScope = CoroutineScope(Dispatchers.IO)\n']:
        if provider.count(line) != 1:
            raise RuntimeError(f'SQLiteProvider text differs: {line!r}')
        provider = provider.replace(line, '')
    if provider.count('connection = ds.connection') != 1 or 'kotlinx' in provider:
        raise RuntimeError('SQLiteProvider text differs from the expected connection line')
    out['core/integrations/sqlite/SQLiteProvider.kt'] = provider.replace('connection = ds.connection', 'connection = oracle.Support.wrap(ds.connection)')
    texts = [c.read(SERVER / name) for name in GE + ['core/game/node/entity/player/link/InterfaceManager.java', 'core/game/component/InterfaceType.java',
                                                     'core/game/container/Container.java']]
    out['org/rs09/consts/Components.java'] = constants(texts, 'Components')
    out['org/rs09/consts/Items.java'] = constants(texts + [definition], 'Items')
    out['org/rs09/consts/NPCs.java'] = constants(texts, 'NPCs')
    out['org/rs09/consts/Sounds.java'] = constants(texts, 'Sounds')
    return out


def pinned_jar(name, sha256):
    candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}')) + sorted((ROOT / 'runs').glob(f'*-oracle/jars/{name}'))
    for jar in candidates:
        if jar.is_file() and hashlib.sha256(jar.read_bytes()).hexdigest() == sha256:
            return jar
    raise RuntimeError(f'No {name} matching its pin under runs/; this generator never downloads')


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)

    lock_names = json.loads(c.read(ROOT / 'oracle/login/kotlin.lock.json'))
    for name, entry in lock_names.items():
        jar = ROOT / 'runs/kotlin-1.8.20' / name
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != entry['sha256']:
            raise RuntimeError(f'{jar} is missing or differs from its pin; this generator never downloads')
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    artifacts = json.loads(c.read(ROOT / 'oracle/world-trace/artifacts.lock.json'))
    json_pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))
    libraries = [stdlib, pinned_jar('json-simple-1.1.1.jar', json_pin['sha256']),
                 pinned_jar('sqlite-jdbc-3.36.0.3.jar', artifacts['sqlite-jdbc-3.36.0.3.jar']['sha256'])]

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    texts = generated_adapters()
    for path in sorted((HERE / 'adapters').rglob('*')):
        if path.is_file():
            texts[path.relative_to(HERE / 'adapters').as_posix()] = c.read(path)
    for name, text in sorted(texts.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    for name in ['Support.java', 'Launcher.java']:
        c.read(HERE / name)
    java.append(HERE / 'Support.java')
    config = CONFIG.read_bytes()
    c.read(CONFIG)

    classes, launcher = RUN / 'classes', RUN / 'launcher'
    for path in [classes, launcher]:
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-d', launcher, HERE / 'Launcher.java'])

    corpus = cases.corpus()
    work, db, tmp = RUN / 'work', RUN / 'db', RUN / 'tmp'
    for path in [work, db, tmp]:
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(dict(case, items=corpus.items[case['itemsTable']])) + '\n' for case in corpus.cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx2g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', f'-Dge.config={CONFIG}',
               f'-Dge.db={db}', f'-Djava.io.tmpdir={tmp}', f'-Dorg.sqlite.tmpdir={tmp}', '-cp', launcher.resolve(), 'oracle.Launcher',
               classes.resolve(), *[path.resolve() for path in libraries]]
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=work, stdin=stdin, capture_output=True, timeout=7200)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-500:])
    outputs = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(outputs) != len(corpus.cases):
        raise RuntimeError(f'{len(outputs)} results for {len(corpus.cases)} cases')
    rows = [dict(case, expected=output['expected']) for case, output in zip(corpus.cases, outputs)]
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    inputs = (json.dumps(dict(items=corpus.items), sort_keys=True, indent=1) + '\n').encode()
    c.read(pathlib.Path(__file__))
    c.read(HERE / 'cases.py')
    c.read(ROOT / 'oracle/login/generate.py')

    events, errors, operations = {}, {}, {}
    for row in rows:
        for action, step in zip([dict(op='setup')] + row['actions'], row['expected']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if step['error'] is not None:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    statements = {}
    for row in rows:
        for step in row['expected']:
            for event in step['events']:
                if event[0] == 'sql':
                    key = re.sub(r'\d+', 'N', event[2])
                    statements[key] = statements.get(key, 0) + 1
    coverage = dict(cases=len(rows), actions=sum(operations.values()), operations=dict(sorted(operations.items())),
                    recordedCalls=sum(events.values()), events=dict(sorted(events.items())), errors=dict(sorted(errors.items())),
                    statements=len(statements), injectedFailures=sum(len(row.get('failures', {})) for row in rows),
                    workerRuns=operations.get('worker', 0))
    for needed in ['sql', 'send', 'message', 'contentMessage', 'setVarp', 'audio', 'jingle', 'log', 'logGE', 'news', 'discord', 'clock',
                   'uidMap', 'players', 'details', 'pinUnlocked', 'pinOpen', 'examine', 'cs2', 'submit', 'workerStart', 'command',
                   'notify', 'invUpdate', 'invRefresh', 'sendInput', 'buttonClick', 'dispatchOpen', 'dispatchClose']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        interfaceConfigSha256=hashlib.sha256(config.replace(b'\r', b'')).hexdigest(), inputsSha256=hashlib.sha256(inputs).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', sqliteJdbc='3.36.0.3', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original Grand Exchange cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{events.get("sql", 0)} SQL statements of {len(statements)} kinds, {coverage["injectedFailures"]} injected failures)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or (FIXTURES / 'inputs.json').read_bytes() != inputs or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Grand Exchange fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'inputs.json').write_bytes(inputs)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary} under {FIXTURES}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
