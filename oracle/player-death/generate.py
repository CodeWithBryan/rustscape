"""Run the unchanged player death and grave classes on JDK 11 and record what they do.

Compiled unchanged: DeathTask.java (getContainers, start, exec, stop), Item,
Container, ContainerEvent, ContainerListener, ContainerType, SortType,
Definition, NodeTask, Pulse, PulseType, GroundItem, GroundItemManager,
Grave.kt, GraveController.kt, GraveType.kt, BarrowsEquipment.kt,
DeathInterface.kt, TickUtils.kt, Globals.kt, Quests.kt, IronmanMode, Rights,
ZoneType, ZoneRestriction, MapDistance and StatAttributeKeys.kt. Everything
else is an adapter from adapters.py, with the original text of the methods
named there inserted at generation time. --check never updates a fixture.
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
RUN = ROOT / 'runs/player-death-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/player-death'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('player_death_adapters', HERE / 'adapters.py')
corpus = load('player_death_corpus', HERE / 'corpus.py')

ORIGINAL = [
    'core/game/node/item/Item.java',
    'core/game/container/Container.java',
    'core/game/container/ContainerEvent.java',
    'core/game/container/ContainerListener.java',
    'core/game/container/ContainerType.java',
    'core/game/container/SortType.java',
    'core/cache/def/Definition.java',
    'core/game/node/entity/combat/DeathTask.java',
    'core/game/system/task/NodeTask.java',
    'core/game/system/task/Pulse.java',
    'core/game/node/entity/impl/PulseType.java',
    'core/game/node/item/GroundItem.java',
    'core/game/node/item/GroundItemManager.java',
    'core/game/node/entity/combat/graves/Grave.kt',
    'core/game/node/entity/combat/graves/GraveController.kt',
    'core/game/node/entity/combat/graves/GraveType.kt',
    'content/global/handlers/item/equipment/BarrowsEquipment.kt',
    'content/global/handlers/iface/DeathInterface.kt',
    'core/tools/TickUtils.kt',
    'core/tools/Globals.kt',
    'content/data/Quests.kt',
    'core/game/node/entity/player/link/IronmanMode.java',
    'core/game/node/entity/player/info/Rights.java',
    'core/game/world/map/zone/ZoneType.java',
    'core/game/world/map/zone/ZoneRestriction.java',
    'core/game/world/map/MapDistance.java',
    'core/game/system/command/sets/StatAttributeKeys.kt',
]
SCOPE = (
    'Unchanged DeathTask (getContainers; start, exec and stop over a player), Item, Container, ContainerEvent, '
    'ContainerListener, ContainerType, SortType, Definition, NodeTask, Pulse, PulseType, GroundItem, GroundItemManager, '
    'Grave, GraveController, GraveType, BarrowsEquipment, DeathInterface, TickUtils, Globals, Quests, IronmanMode, Rights, '
    'ZoneType, ZoneRestriction, MapDistance and StatAttributeKeys on JDK 11. Original text inserted at generation time: '
    'Player commenceDeath/finalizeDeath; Entity commenceDeath/finalizeDeath; Node constructor, getId, getName, '
    'index and destination accessors; ImpactHandler log and disabled-tick accessors; Animator RESET_A/RESET_G; '
    'Location toString/fromString/getDistance/withinDistance; RegionChunk add/remove(GroundItem); ItemUpdateFlag '
    'types; ItemPlugin.DROP; the ItemDefinition fields and forId, getDefinitions, getName, isStackable, getValue, '
    'getAlchemyValue, getNoteId, isUnnoted, getItemPlugin, getOptionHandler, hasAction, hasDestroyAction, isTradeable; '
    'the Log enum. Recording adapters: every other Player, Entity and NPC member (prayer, skull, ironman, settings, '
    'packets, zone monitor, properties, skills, timers, familiars, interfaces, appearance, saved data), the grave NPC '
    'base (AbstractNPC/NPC: transform, init, clear, location, walking flags), ContentAPI calls, InteractionListeners, '
    'item and equipment plugins, permadeath, Repository.uid_map/sendNews, WildernessZone.isInZone, ServerStore, '
    'PlayerMonitor, GlobalStats, SlayerEquipmentFlags, packets and the chunk lookup. Item definitions, player answers, '
    'the tick counter and the per-tick driver order (GraveController.tick, GroundItemManager.pulse, Grave NPC ticks) '
    'are case inputs. Inventory and equipment are base Containers with no listeners. Not exercised: Permadeath.kt '
    'itself, GravePurchaseInterface, the forcegravedeath command, PulseManager/PulseRunner scheduling of the death '
    'task, packet bytes.'
)


def line(text, pattern):
    """One declaration line of an original source, as written."""
    match = re.search(pattern, text, re.M)
    if not match:
        raise RuntimeError(f'declaration not found: {pattern}')
    return match.group(0).strip()


def markers():
    def source(name):
        return c.read(SERVER / name)

    node = source('core/game/node/Node.java')
    entity = source('core/game/node/entity/Entity.java')
    player = source('core/game/node/entity/player/Player.java')
    impact = source('core/game/node/entity/combat/ImpactHandler.java')
    location = source('core/game/world/map/Location.kt')
    animator = source('core/game/node/entity/impl/Animator.java')
    chunk = source('core/game/world/map/RegionChunk.java')
    flag = source('core/game/world/update/flag/chunk/ItemUpdateFlag.java')
    definition = source('core/cache/def/impl/ItemDefinition.java')
    skills = source('core/game/node/entity/skill/Skills.java')
    for name, value in [('PRAYER', 5), ('MAGIC', 6)]:
        if not re.search(rf'\b{name} = {value}\b', skills):
            raise RuntimeError(f'Skills.{name} differs')
    body = definition[definition.index('private static final Map<Integer, ItemDefinition> DEFINITIONS'):definition.index('public ItemDefinition()')]
    for signature in ['public ItemDefinition()', 'public static ItemDefinition forId(', 'public static Map<Integer, ItemDefinition> getDefinitions()',
                      'public String getName()', 'public boolean isStackable()', 'public int getValue()', 'public int getAlchemyValue(',
                      'public int getNoteId()', 'public boolean isUnnoted()', 'public ItemPlugin getItemPlugin()', 'public static OptionHandler getOptionHandler(',
                      'public boolean hasAction(String optionName)', 'public boolean hasDestroyAction()', 'public boolean isTradeable()']:
        body += '\n\t' + c.method(definition, signature) + '\n'
    return {
        'ORIGINAL_NODE_CONSTRUCTOR': c.method(node, 'public Node(String name, Location location)'),
        'ORIGINAL_NODE_GET_ID': c.method(node, 'public int getId()'),
        'ORIGINAL_NODE_GET_NAME': c.method(node, 'public String getName()'),
        'ORIGINAL_NODE_GET_INDEX': c.method(node, 'public int getIndex()'),
        'ORIGINAL_NODE_SET_INDEX': c.method(node, 'public void setIndex(int index)'),
        'ORIGINAL_NODE_GET_DESTINATION_FLAG': c.method(node, 'public DestinationFlag getDestinationFlag()'),
        'ORIGINAL_ENTITY_COMMENCE_DEATH': c.method(entity, 'public void commenceDeath(Entity killer)'),
        'ORIGINAL_ENTITY_FINALIZE_DEATH': c.method(entity, 'public void finalizeDeath(Entity killer)'),
        'ORIGINAL_PLAYER_COMMENCE_DEATH': c.method(player, 'public void commenceDeath(Entity killer)'),
        'ORIGINAL_PLAYER_FINALIZE_DEATH': c.method(player, 'public void finalizeDeath(Entity killer)'),
        'ORIGINAL_GET_NPC_LOG': c.method(impact, 'public Map<Entity, Integer> getNpcImpactLog()'),
        'ORIGINAL_GET_PLAYER_LOG': c.method(impact, 'public Map<Integer, Integer> getPlayerImpactLog()'),
        'ORIGINAL_GET_DISABLED': c.method(impact, 'public int getDisabledTicks()'),
        'ORIGINAL_SET_DISABLED': c.method(impact, 'public void setDisabledTicks(int ticks)'),
        'ORIGINAL_RESET_A': line(animator, r'^\s*public static final Animation RESET_A = [^;]+;'),
        'ORIGINAL_RESET_G': line(animator, r'^\s*public static final Graphics RESET_G = [^;]+;'),
        'ORIGINAL_LOCATION_TO_STRING': line(location, r'^\s*override fun toString\(\): String = .+$'),
        'ORIGINAL_LOCATION_FROM_STRING': c.method(location, 'fun fromString(locString: String): Location'),
        'ORIGINAL_LOCATION_DISTANCE': c.method(location, 'fun getDistance(first: Location, second: Location)'),
        'ORIGINAL_LOCATION_WITHIN': c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)'),
        'ORIGINAL_CHUNK_ADD': c.method(chunk, 'public void add(GroundItem item)'),
        'ORIGINAL_CHUNK_REMOVE': c.method(chunk, 'public void remove(GroundItem item)'),
        'ORIGINAL_CONSTRUCT_TYPE': line(flag, r'^\s*public static final int CONSTRUCT_TYPE = [^;]+;'),
        'ORIGINAL_REMOVE_TYPE': line(flag, r'^\s*public static final int REMOVE_TYPE = [^;]+;'),
        'ORIGINAL_PLUGIN_DROP': line(source('core/game/node/item/ItemPlugin.java'), r'^\s*protected static final int DROP = [^;]+;'),
        'ORIGINAL_ITEMDEF_BODY': body,
        'ORIGINAL_LOG_ENUM': c.method(source('core/tools/SystemLogger.kt'), 'enum class Log {'),
    }


def constants(name, text, keys):
    """A Java holder for the named `const val` strings of an original Kotlin companion."""
    fields = ''
    for key in keys:
        match = re.search(rf'const val {key}\s*=\s*("[^"]*")', text)
        if not match:
            raise RuntimeError(f'{name}.{key} not found')
        fields += f'\tpublic static final String {key} = {match.group(1)};\n'
    return f'package core.game.system.config;\n\n/** Constant values taken from the original companion at generation time. */\npublic class {name} {{\n{fields}}}\n'


def library(name, lock_path):
    """A pinned jar in this oracle's run directory, copied from another oracle's run directory if needed."""
    pin = json.loads(c.read(ROOT / lock_path))
    jar = RUN / name

    def valid(path):
        return path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == pin['sha256']

    if not valid(jar):
        for candidate in sorted((ROOT / 'runs').glob(f'*/{name}')):
            if valid(candidate):
                shutil.copyfile(candidate, jar)
                break
    # Pinned by hash; this generator never downloads.
    if not valid(jar):
        raise RuntimeError(f'{jar} is missing or differs from its pin; place it from another oracle run directory')
    return jar


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)

    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libraries = [stdlib, library('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), library('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    texts = dict(adapters.ADAPTERS)
    texts['core/game/system/config/ItemConfigParser.java'] = constants(
        'ItemConfigParser', c.read(SERVER / 'core/game/system/config/ItemConfigParser.kt'), ['HIGH_ALCHEMY', 'LOW_ALCHEMY', 'TRADEABLE'])
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(texts.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'corpus.py')
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')

    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx768m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])

    cases = corpus.requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '--add-opens', 'java.base/java.lang=ALL-UNNAMED',
               '--add-opens', 'java.base/java.util=ALL-UNNAMED', '-cp', classpath, 'oracle.Support']
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    (RUN / 'output.jsonl').write_bytes(result.stdout)
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    steps = [(action, step) for row in rows for action, step in zip(row['actions'], row['expected'])]
    events, errors, operations = {}, {}, {}
    for action, step in steps:
        operations[action['op']] = operations.get(action['op'], 0) + 1
        if step['error'] is not None:
            errors[step['error']] = errors.get(step['error'], 0) + 1
        for event in step['events']:
            events[event[0]] = events.get(event[0], 0) + 1
    unexpected = [error for error in errors if '.' in error]
    if unexpected:
        raise RuntimeError(f'unexpected failures in the corpus: {unexpected}')
    coverage = dict(
        actions=len(steps),
        operations=dict(sorted(operations.items())),
        serviceCalls=sum(events.values()),
        events=dict(sorted(events.items())),
        errors=dict(sorted(errors.items())),
        groundItems=sum(len(row['expected'][-1]['state']['grounds']) for row in rows),
        graves=sum(len(row['expected'][-1]['state']['graves']) for row in rows),
        ticks=operations.get('tick', 0) + events.get('tick', 0),
    )
    for needed in corpus.NEEDED:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original player death cases ({coverage["actions"]} actions, {coverage["serviceCalls"]} service calls, '
               f'{coverage["groundItems"]} ground items, {coverage["graves"]} graves, {coverage["ticks"]} ticks)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Player death fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
