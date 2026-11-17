"""Run the original everyday item handlers on JDK 11 and record what they do.

Compiled unchanged: the everyday item options and item-on-object basics of content/global/handlers/item listed in
UNCHANGED (with the tables they read: EmptyOptionListener's herblore enums, LightSource, Lamps, BirdNest,
WeightBasedTable, RandomFunction, ChanceItem, DigSpadeHandler, the gloves), with the unchanged OptionHandler, Plugin,
Pulse, IntType, core.api.Container, Script, DialogueConst, EquipmentSlot, ApiExtensions, Globals, LoginListener and
PersistPlayer. The members named in `markers` are original text inserted into the adapters of adapters.py at
generation time. Everything else is a recording adapter answering from the case. Every case runs in a class loader of
its own (Boot.java), so the originals' static state starts fresh. Java/Kotlin are test oracles only; --check never
rewrites a fixture.
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
RUN = ROOT / 'runs/global-items2-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/global-items2'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('global_items2_adapters', HERE / 'adapters.py')
corpus = load('global_items2_cases', HERE / 'cases.py')

H = 'content/global/handlers/item/'
UNCHANGED = [
    H + 'EmptyOptionListener.kt',
    'content/global/skill/herblore/FinishedPotion.kt',
    'content/global/skill/herblore/UnfinishedPotion.kt',
    'content/global/skill/herblore/Herbs.java',
    'content/global/skill/herblore/GrindingItem.java',
    'content/data/Quests.kt',
    H + 'LightSourceListeners.kt',
    'content/data/LightSource.java',
    H + 'SpadeDigOptionPlugin.java',
    H + 'SpadeDigListener.kt',
    'core/game/global/action/DigSpadeHandler.java',
    'core/game/global/action/DigAction.java',
    H + 'CasketListener.kt',
    'core/api/utils/WeightBasedTable.kt',
    'core/api/utils/WeightedItem.kt',
    H + 'BirdNestScript.kt',
    H + 'BirdNestPlugin.java',
    'content/data/tables/BirdNest.java',
    H + 'LampPlugin.java',
    'content/data/Lamps.java',
    H + 'StarterPackPlugin.java',
    H + 'ItemQuestRequirementListener.kt',
    H + 'MorphItemListener.kt',
    H + 'RingUnchargePlugin.java',
    H + 'NewspaperListener.kt',
    H + 'RunePackPlugin.java',
    H + 'HerbBoxPlugin.java',
    H + 'CadavaPotionListener.kt',
    H + 'DrinkBlamishOilListener.kt',
    H + 'ToyListeners.kt',
    H + 'ToyHorseyListener.kt',
    H + 'withobject/WaterSourceListener.kt',
    H + 'withobject/SandSourceListener.kt',
    H + 'withobject/ChestKeyListener.kt',
    H + 'withobject/HatStand.kt',
    H + 'withobject/SackOnHay.kt',
    H + 'equipment/brawling_gloves/BrawlingGloves.java',
    H + 'equipment/brawling_gloves/BrawlingGlovesManager.java',
    H + 'equipment/fistofguthixgloves/FOGGlovesListener.kt',
    H + 'equipment/fistofguthixgloves/FOGGlovesManager.kt',
    H + 'EnchantedGemListener.kt',
    H + 'PetRock.kt',
    H + 'toys/DiangoOptionHandler.java',
    H + 'toys/DiangoReclaimInterface.java',
    H + 'toys/SnowGlobePlugin.java',
    'core/api/DialUtils.kt',
    'core/game/dialogue/DialogueTopic.kt',
    'core/game/component/ComponentPlugin.java',
    'content/global/skill/construction/decoration/pohstorage/StorableFamily.kt',
    'core/game/interaction/OptionHandler.java',
    'core/plugin/Plugin.java',
    'core/game/system/task/Pulse.java',
    'core/game/interaction/IntType.kt',
    'core/api/Container.kt',
    'core/game/interaction/Script.kt',
    'core/tools/DialogueConst.kt',
    'core/api/EquipmentSlot.kt',
    'core/api/ApiExtensions.kt',
    'core/tools/Globals.kt',
    'core/api/LoginListener.kt',
    'core/api/PersistPlayer.kt',
    'core/tools/RandomFunction.java',
    'core/game/node/item/ChanceItem.java',
    'core/game/node/item/WeightedChanceItem.java',
    'core/game/node/entity/npc/drop/DropFrequency.java',
]
SCOPE = (
    'The everyday item handlers listed in `unchanged` compiled unchanged on JDK 11 with Kotlin 1.8.20 and the published '
    'ConstLib 1.4.28 (Items, Sounds, Scenery, Animations, NPCs and Components values checked against the suffixes of the '
    'names used), together with the tables they read and the unchanged OptionHandler, Plugin, Pulse, IntType, '
    'core.api.Container, Script, DialogueConst, EquipmentSlot, ApiExtensions, Globals, LoginListener, PersistPlayer, '
    'RandomFunction, ChanceItem, WeightedChanceItem and DropFrequency. Original text inserted into adapters at generation '
    'time: the Skills ids and SKILL_NAME, StringUtils.isPlusN, NPCDropTables.DROP_RATES and the ContentAPI functions '
    'visualize, animate, runTask, getUsedOption, sendChat and getItemFromEquipment. Everything else is a recording adapter '
    'answering from the case: the InteractionListeners and InterfaceListeners registries and item definition handler maps '
    '(registration is recorded in order), every other ContentAPI function, the player members (attributes with the '
    'GameAttributes "/save:" rule, locks, animator, pulse manager, packet dispatch, interface manager, dialogue '
    'interpreter, inventory, equipment and bank, appearance, diaries, quest repository, skull manager, skills), the world '
    'pulser, GroundItemManager, the item definitions table (a HashMap filled in the case order) and the node fields. '
    'Each case runs in a class loader of its own and seeds RandomFunction.RANDOM, the generator of Collections.shuffle '
    'and the thread\'s ThreadLocalRandom (Kotlin\'s Random.Default); the raw state of each is recorded after every '
    'operation. Calls made while a class initialises are not recorded.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


CONTENT_API = [
    'fun <A, G> visualize(entity: Entity, anim: A, gfx: G) {',
    'fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) {',
    'fun runTask(entity: Entity, delay: Int = 0, repeatTimes: Int = 1, task: () -> Unit) {',
    'fun getUsedOption(player: Player): String {',
    'fun sendChat(entity: Entity, message: String, delay: Int = -1) {',
    'fun getItemFromEquipment(player: Player, slot: EquipmentSlot): Item? {',
    'fun face(entity: Entity, toFace: Node, duration: Int = -1) {',
    'fun getSlayerMasterLocation(player: Player): String {',
]


def markers():
    read = lambda name: c.read(SERVER / name)
    api = read('core/api/ContentAPI.kt')
    skills = read('core/game/node/entity/skill/Skills.java')
    dialogue = read('core/game/dialogue/DialogueFile.kt')
    return {
        'ORIGINAL_SKILL_IDS': lines(skills, 'public static final int ATTACK = 0,'),
        'ORIGINAL_SKILL_NAME': lines(skills, 'public static final String[] SKILL_NAME = {'),
        'ORIGINAL_IS_PLUS_N': c.method(read('core/tools/StringUtils.java'), 'public static boolean isPlusN(String word) {'),
        'ORIGINAL_DROP_RATES': lines(read('core/game/node/entity/npc/drop/NPCDropTables.java'), 'public static final int[] DROP_RATES'),
        'ORIGINAL_CONTENT_API': '\n\n'.join(c.method(api, signature) for signature in CONTENT_API),
        'ORIGINAL_PLURALISE': c.method(read('content/global/skill/slayer/SlayerUtils.kt'), 'fun pluralise(str: String): String {'),
        'ORIGINAL_DIALOGUE_FILE_NPC': c.method(dialogue, 'open fun npc(expression: FacialExpression?, vararg messages: String?): Component? {'),
        'ORIGINAL_DIALOGUE_FILE_PLAYER_PLAIN': c.method(dialogue, 'open fun player(vararg messages: String?): Component? {'),
        'ORIGINAL_DIALOGUE_FILE_PLAYER': c.method(dialogue, 'open fun player(expression: FacialExpression?, vararg messages: String?): Component? {'),
        'ORIGINAL_DIALOGUE_FILE_NPCL': c.method(dialogue, 'open fun npcl(expr: FacialExpression?, msg: String?): Component? {'),
        'ORIGINAL_DIALOGUE_FILE_END': c.method(dialogue, 'open fun end(){'),
        'ORIGINAL_DIALOGUE_FILE_OPTIONS': c.method(dialogue, 'open fun options(vararg options: String?, title: String = "Select an Option") {'),
        'ORIGINAL_DIALOGUE_FILE_SHOW_TOPICS': c.method(dialogue, 'fun showTopics(vararg topics: Topic<*>, title: String = "Select an Option"): Boolean {'),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/global-items2-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


CONSTANT_KINDS = ['Items', 'Sounds', 'Scenery', 'Animations', 'NPCs', 'Components']


def constants_match_names(classpath, texts):
    """Every ConstLib constant the compiled sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(texts)
    checked = {}
    for kind in CONSTANT_KINDS:
        text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.' + kind], capture_output=True, check=True).stdout.decode()
        used = sorted(set(re.findall(r'(?<![\w.])' + kind + r'\.([A-Z][A-Z0-9_]*_\d+)\b', sources)))
        for name in used:
            found = re.search(r'int ' + name + r' = (-?\d+);', text)
            if found is None or found.group(1) != name.rsplit('_', 1)[1]:
                raise RuntimeError(f'{kind}.{name} differs from its suffix')
        checked[kind] = used
    return checked


def static_items(classpath, texts):
    """The static `Items.*` imports (LightSource) name constants without the class prefix; check those too."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.Items'], capture_output=True, check=True).stdout.decode()
    source = c.read(SERVER / 'content/data/LightSource.java')
    used = sorted(set(re.findall(r'new Item\(([A-Z][A-Z0-9_]*_\d+)\b', source)))
    for name in used:
        found = re.search(r'int ' + name + r' = (-?\d+);', text)
        if found is None or found.group(1) != name.rsplit('_', 1)[1]:
            raise RuntimeError(f'Items.{name} differs from its suffix')
    return used


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    reference = ROOT / 'reference' / 'server'
    if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    # BrawlingGlovesManager's @NotNull comes from the JetBrains annotations the Kotlin compiler pin includes.
    annotations = next(path for path in jars if path.name.startswith('annotations-'))
    libs = [stdlib, annotations, *libraries()]
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
    for name in ('Support.java', 'Calls.java'):
        c.read(HERE / name)
        java.append(HERE / name)
    c.read(HERE / 'Boot.java')
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-XDignore.symbol.file', '-cp', classpath, '-d', classes, *java])
    boot = RUN / 'boot'
    if boot.exists():
        shutil.rmtree(boot)
    boot.mkdir()
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-d', boot, HERE / 'Boot.java'])
    checked = constants_match_names(os.pathsep.join(map(str, libs)), texts)
    checked['Items (static import)'] = static_items(os.pathsep.join(map(str, libs)), texts)
    return lock, os.pathsep.join(map(str, [boot, *libs])), classes, checked


def execute(classpath, classes, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.boot.Boot', str(classes)], input=payload, cwd=ROOT, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Index', 'Cast', 'Binding', 'Number', 'NoSuchElement', 'Argument', 'State')


def generate(check):
    lock, classpath, classes, checked = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, classes, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, len(row['expected'][sweep]['events']) + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(classpath, classes, failing)
    cases += failing
    # The run's raw result, for inspection; the fixture is written only below.
    (RUN / 'last.jsonl').write_text(''.join(json.dumps(row, sort_keys=True) + '\n' for row in rows), encoding='utf-8')
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['ops'], row['expected']):
            error = step['error']
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, (number, row.get('name'), op))
            elif error == 'Binding' and 'failAt' not in op and not corpus.injects(row):
                mistakes.setdefault(error, (number, row.get('name'), op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
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
    summary = (f'{len(rows)} original everyday item handler cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('everyday item handler fixtures/provenance differ; inspect before regeneration')
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
