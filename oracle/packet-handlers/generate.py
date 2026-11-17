"""Run the original packet handler bodies on JDK 11 and record what they do.

Compiled unchanged: UseWithHandler, NodeUsageEvent, EquipHandler, DropListener and DialogueAction. The members named in
`markers` (PacketProcessor's item, NPC, player, use-with, component, slot switch and examine/track branches, InteractPlugin's
handleItemOption, the examine getters, isPlusN, getArrayIndex, SpellUtils' two members, SpellBook.forInterface and the
constants) are original text inserted into the adapters of adapters.py at generation time. Everything else is a recording
adapter answering from the case. Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/packet-handlers-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/packet-handlers'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('packet_handlers_adapters', HERE / 'adapters.py')
corpus = load('packet_handlers_cases', HERE / 'cases.py')

UNCHANGED = [
    'core/game/interaction/UseWithHandler.java',
    'core/game/interaction/NodeUsageEvent.java',
    'core/game/global/action/EquipHandler.kt',
    'core/game/global/action/DropListener.kt',
    'core/game/dialogue/DialogueAction.java',
]
PROCESSOR_METHODS = [
    'private fun processSlotSwitch(pkt: Packet) {',
    'private fun processComponentUseWith(pkt: Packet) {',
    'private fun processUseWith(pkt: Packet) {',
    'private fun processPlayerAction(pkt: Packet.PlayerAction) {',
    'private fun processNpcAction(pkt: Packet.NpcAction) {',
    'private fun processItemAction(pkt: Packet.ItemAction) {',
    'private fun getLikelyContainerForIface(player: Player, iface: Int) : Container? {',
    'private fun sendClearMinimap(player: Player) {',
    'fun switchItem(slot: Int, secondSlot: Int, container: Container?, insert: Boolean, player: Player) {',
]
PROCESS_ARMS = [
    'is Packet.TrackFinished -> {',
    'is Packet.ItemExamine -> {',
    'is Packet.SceneryExamine -> {',
    'is Packet.NpcExamine -> {',
]
SCOPE = (
    'Unchanged UseWithHandler, NodeUsageEvent, EquipHandler, DropListener and DialogueAction on JDK 11 with Kotlin 1.8.20 and '
    'the published ConstLib 1.4.28 (Components, Items and Sounds values checked against the suffixes of the names used). '
    'Original text inserted into adapters at generation time: PacketProcessor.processItemAction, processNpcAction, '
    'processPlayerAction, processUseWith, processComponentUseWith, processSlotSwitch, getLikelyContainerForIface, '
    'sendClearMinimap, switchItem and the TrackFinished, ItemExamine, SceneryExamine and NpcExamine branches of process; '
    'InteractPlugin.handleItemOption (its pulse is InteractPlugin$1); Definition.getConfiguration(key, fail) and getExamine, '
    'ItemDefinition.getExamine, SceneryDefinition.getConfigFile, NPCDefinition.getExamine, StringUtils.isPlusN, '
    'BankContainer.getArrayIndex, Item.toString, SpellUtils.validateSpellbookInterface/getBookFromInterface, '
    'SpellBook.forInterface and its constants, the EquipmentContainer slot constants and the ItemConfigParser and NPCConfigParser '
    'keys. Everything else is a recording adapter answering from the case: containers (get, replace, insert, remove, add, '
    'freeSlot, refresh, getMaximumAdd; replace stores the given object, not a copy), the bank tab members, '
    'EquipmentContainer.add, the repository node lists (NodeList.get bounds), node lookups (getShownNPC, getChild, '
    'RegionManager.getObject, GroundItemManager.get/create), getInteraction and the options array, InteractPlugin.handle and '
    'handleInvalidInteraction, InteractionListeners.run (three forms, with the Kotlin parameter checks), '
    'PluginInteractionManager.handle, the zone monitor, locks, scripts, pulses (MovementPulse construction, PulseManager, '
    'GameWorld.Pulser), option handlers, use-with handler subclasses, interfaces, dialogue, familiar, spell book, spell '
    'listeners, castSpell, music player, attributes, messages, audio, events, pets, graves, PlayerParser.save, PlayerMonitor, '
    'definitions lookup, varbits and node toString text. Not driven: UseWithHandler.getValidChildren and getDestination, '
    'the deprecated InteractionListener members and every content listener or plugin behind the recorded services.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


def kotlin_const(text, name):
    found = re.search(r'const val ' + name + r' = ("[^"]+")', text)
    if found is None:
        raise RuntimeError(f'no const val {name}')
    return f'public static final String {name} = {found.group(1)};'


def markers():
    read = lambda name: c.read(SERVER / name)
    processor = read('core/net/packet/PacketProcessor.kt')
    definition = read('core/cache/def/Definition.java')
    books = read('core/game/node/entity/player/link/SpellBookManager.java')
    constants = re.findall(r'^\t\t(MODERN|ANCIENT|LUNAR)\((\d+)\)', books, re.M)
    if [name for name, _ in constants] != ['MODERN', 'ANCIENT', 'LUNAR']:
        raise RuntimeError(f'SpellBook constants changed: {constants}')
    spells = read('content/global/skill/magic/SpellUtils.kt')
    items = read('core/game/system/config/ItemConfigParser.kt')
    npcs = read('core/game/system/config/NPCConfigParser.kt')
    return {
        'ORIGINAL_PROCESSOR_METHODS': '\n\n    '.join(c.method(processor, s) for s in PROCESSOR_METHODS),
        'ORIGINAL_PROCESS_ARMS': '\n            '.join(c.method(processor, s) for s in PROCESS_ARMS),
        'ORIGINAL_HANDLE_ITEM_OPTION': c.method(read('core/game/interaction/InteractPlugin.java'),
                                                'public void handleItemOption(final Player player, final Option option, final Container container) {'),
        'ORIGINAL_GET_ARRAY_INDEX': c.method(read('core/game/container/impl/BankContainer.java'), 'public static int getArrayIndex(int tabId) {'),
        'ORIGINAL_SLOT_WEAPON': lines(read('core/game/container/impl/EquipmentContainer.java'), 'public static final int SLOT_HAT = 0,'),
        'ORIGINAL_GET_CONFIGURATION': c.method(definition, 'public <V> V getConfiguration(String key, V fail) {'),
        'ORIGINAL_DEFINITION_EXAMINE': c.method(definition, 'public String getExamine() {'),
        'ORIGINAL_ITEM_EXAMINE': c.method(read('core/cache/def/impl/ItemDefinition.java'), 'public String getExamine() {'),
        'ORIGINAL_GET_CONFIG_FILE': c.method(read('core/cache/def/impl/SceneryDefinition.java'), 'public VarbitDefinition getConfigFile() {'),
        'ORIGINAL_NPC_EXAMINE': c.method(read('core/cache/def/impl/NPCDefinition.java'), 'public final String getExamine() {'),
        'ORIGINAL_IS_PLUS_N': c.method(read('core/tools/StringUtils.java'), 'public static boolean isPlusN(String word) {'),
        'ORIGINAL_ITEM_TO_STRING': c.method(read('core/game/node/item/Item.java'), 'public String toString() {'),
        'ORIGINAL_SPELL_UTILS': '\n    '.join(c.method(spells, s) for s in [
            'fun validateSpellbookInterface(player : Player, packetInterfaceId : Int) : Boolean {',
            'fun getBookFromInterface(id: Int): String{']),
        'ORIGINAL_SPELL_BOOKS': ', '.join(f'{name}({value})' for name, value in constants),
        'ORIGINAL_FOR_INTERFACE': c.method(books, 'public static SpellBook forInterface(final int id) {'),
        'ORIGINAL_ITEM_CONFIG_KEYS': '\n    '.join(kotlin_const(items, k) for k in ['EQUIP_SLOT', 'EQUIP_AUDIO', 'DESTROY']),
        'ORIGINAL_NPC_CONFIG_KEYS': kotlin_const(npcs, 'EXAMINE'),
    }


def libraries():
    """Pinned jars: this run directory first, then any other oracle's run directory, each checked against its pin."""
    found = []
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
        jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
        if jar is None:
            # Pinned by hash; this generator never downloads.
            raise RuntimeError(f'{name} matching its pin was not found in runs/packet-handlers-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def constants_match_names(classpath, texts):
    """Every Items/Components/Sounds constant the compiled sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(texts)
    checked = {}
    for kind in ['Items', 'Components', 'Sounds']:
        text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.' + kind], capture_output=True, check=True).stdout.decode()
        names = sorted(set(re.findall(kind + r'\.([A-Z0-9_]+)', sources)))
        for name in names:
            found = re.search(r'int ' + name + r' = (-?\d+);', text)
            if found is None or found.group(1) != name.rsplit('_', 1)[1]:
                raise RuntimeError(f'{kind}.{name} differs from its suffix')
        checked[kind] = names
    return checked


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    reference = ROOT / 'reference' / 'server'
    if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libs = [stdlib, *libraries()]
    kotlin, java, texts = [], [], []
    for name in UNCHANGED:
        path = SERVER / name
        texts.append(c.read(path))
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(adapters.ADAPTERS.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        texts.append(text)
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'cases.py')
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx768m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-XDignore.symbol.file', '-cp', classpath, '-d', classes, *java])
    checked = constants_match_names(os.pathsep.join(map(str, libs)), texts)
    return lock, classpath, checked


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support'], input=payload, cwd=ROOT, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Index', 'Cast', 'Binding')


def generate(check):
    lock, classpath, checked = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, len(row['expected'][sweep]['events']) + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(classpath, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['ops'], row['expected']):
            error = step['error']
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, (number, op))
            elif error == 'Binding' and 'failAt' not in op and not corpus.injects(row):
                mistakes.setdefault(error, (number, op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if step['error']:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=corpus.groups(cases),
                    failureCases=len(failing))
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), scope=SCOPE)
    summary = (f'{len(rows)} original packet handler cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('packet handler fixtures/provenance differ; inspect before regeneration')
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
