"""Run the original dialogue DSLs on JDK 11 and record what they do.

Compiled unchanged: DialogueLabeller.kt, DialogueBuilder.kt, SkillDialogueHandler.kt, SkillDialoguePlugin.java,
ActivateEnchantedGem.java, the dialogue core they run on (DialogueInterpreter, DialoguePlugin, DialogueFile,
EmptyPlugin, DialogueTopic, FacialExpression, DialogueAction, SimpleDialoguePlugin), DialUtils.kt, DialogueConst.kt,
core.api.InputType, Event.kt, EventHook.kt, CloseEvent.java, IronmanMode.java and the plugin annotations/interface,
plus the three dialogue event classes copied verbatim from Events.kt. Original text inserted into adapters at
generation time (the `markers`): ContentAPI's face, openDialogue, both sendInputDialogue overloads,
SecondaryBankAccountActivationResult and its ContentAPI class line; Event.DialogueClosed; Entity's hook field,
dispatch, unhook and both hook overloads; Component.setUnclosable; StringUtils.formatDisplayName; BankerNPC's
ODOVACAR_FEE, canAccessBank and the whole BankerDialogueLabellerFile. Everything else is a recording stand-in
answering from the case. Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/dialogue-dsl-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/dialogue-dsl'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
corpus = load('dialogue_dsl_cases', HERE / 'cases.py')

D = 'core/game/dialogue/'
UNCHANGED = [
    D + 'DialogueLabeller.kt',
    D + 'DialogueBuilder.kt',
    D + 'SkillDialogueHandler.kt',
    D + 'SkillDialoguePlugin.java',
    D + 'ActivateEnchantedGem.java',
    D + 'DialogueInterpreter.java',
    D + 'DialoguePlugin.java',
    D + 'DialogueFile.kt',
    D + 'EmptyPlugin.kt',
    D + 'DialogueTopic.kt',
    D + 'FacialExpression.java',
    D + 'DialogueAction.java',
    D + 'SimpleDialoguePlugin.java',
    'core/game/event/Event.kt',
    'core/game/event/EventHook.kt',
    'core/game/component/CloseEvent.java',
    'core/tools/DialogueConst.kt',
    'core/api/DialUtils.kt',
    'core/api/InputType.kt',
    'core/game/node/entity/player/link/IronmanMode.java',
    'core/plugin/Plugin.java',
    'core/plugin/PluginManifest.java',
    'core/plugin/PluginType.java',
    'core/plugin/Initializable.java',
]
SCOPE = (
    'The classes listed in `unchanged` compiled unchanged on JDK 11 with Kotlin 1.8.20 and the published ConstLib '
    '1.4.28, plus the three dialogue event classes copied verbatim from Events.kt. Original text inserted into adapters '
    'at generation time: ContentAPI face, openDialogue, both sendInputDialogue overloads, '
    'SecondaryBankAccountActivationResult and the ContentAPI class line; core.api.Event.DialogueClosed; Entity\'s hook '
    'map, dispatch, unhook and both hook overloads; Component.setUnclosable; StringUtils.formatDisplayName; BankerNPC\'s '
    'ODOVACAR_FEE, canAccessBank and BankerDialogueLabellerFile. Recording stand-ins: Player (attributes, location, '
    'inventory amount, quest stage), NPC (new NPC(id) included), PacketDispatch, InterfaceManager, Component, Item, '
    'ItemDefinition (handler map), NPCDefinition, GameWorld, PacketRepository, the bank/ironman/GE/message ContentAPI '
    'members, ServerConstants.SECOND_BANK (an unrecorded field), SlayerManager, OptionHandler, Quests, logging, '
    'System.out.printf and System.err. Scripted DialogueLabeller/DialogueBuilderFile/SkillDialogueHandler subclasses '
    'call only original DSL members. The input op invokes the stored runscript directly (not RunScript.processInput). '
    'Not packet bytes, real interfaces, real definitions, other concrete content or a live session.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if len(found) != 1:
        raise RuntimeError(f'expected one line starting with {prefix}: {found}')
    return found[0]


def markers():
    read = lambda name: c.read(SERVER / name)
    api = read('core/api/ContentAPI.kt')
    entity = read('core/game/node/entity/Entity.java')
    banker = read('content/global/handlers/npc/BankerNPC.kt')
    return {
        'ORIGINAL_FACE': c.method(api, 'fun face(entity: Entity, toFace: Node, duration: Int = -1) {'),
        'ORIGINAL_OPEN_DIALOGUE': c.method(api, 'fun openDialogue(player: Player, dialogue: Any, vararg args: Any) {'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC': c.method(api, 'fun sendInputDialogue(player: Player, numeric: Boolean, prompt: String, handler: (value: Any) -> Unit) {'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_TYPED': c.method(api, 'fun sendInputDialogue(player: Player, type: InputType, prompt: String, handler: (value: Any) -> Unit) {'),
        'ORIGINAL_ACTIVATION_RESULT': c.method(api, 'enum class SecondaryBankAccountActivationResult {'),
        'ORIGINAL_CONTENT_API_CLASS': lines(api, 'private class ContentAPI'),
        'ORIGINAL_EVENT_DIALOGUE_CLOSED': lines(read('core/api/Event.kt'), '@JvmStatic val DialogueClosed ='),
        'ORIGINAL_HOOKS_FIELD': lines(entity, 'private HashMap<Class<?>, ArrayList<EventHook>> hooks'),
        'ORIGINAL_ENTITY_DISPATCH': c.method(entity, 'public void dispatch(Event event)'),
        'ORIGINAL_ENTITY_UNHOOK': c.method(entity, 'public void unhook(EventHook hook)'),
        'ORIGINAL_ENTITY_HOOK_EVENT': c.method(entity, 'public void hook(Event event, EventHook hook)'),
        'ORIGINAL_ENTITY_HOOK_CLASS': c.method(entity, 'public void hook(Class<?> event, EventHook hook)'),
        'ORIGINAL_SET_UNCLOSABLE': c.method(read('core/game/component/Component.java'), 'public static void setUnclosable(Player p, Component c) {'),
        'ORIGINAL_FORMAT_DISPLAY_NAME': c.method(read('core/tools/StringUtils.java'), 'public static String formatDisplayName(String name) {'),
        'ORIGINAL_ODOVACAR_FEE': lines(banker, 'private const val ODOVACAR_FEE'),
        'ORIGINAL_CAN_ACCESS_BANK': c.method(banker, 'fun canAccessBank(player: Player, npc: NPC): Boolean {'),
        'ORIGINAL_BANKER_DIALOGUE': c.method(banker, 'class BankerDialogueLabellerFile : DialogueLabeller() {'),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/dialogue-dsl-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def bundles(inserted):
    """The stand-in, adapter, script and harness bundles, split at their `//// path` markers."""
    out = []
    for bundle in ['Standins.java', 'Adapters.kt', 'Script.kt', 'Harness.java']:
        parts = re.split(r'(?m)^//// (\S+)\n', c.read(HERE / bundle))
        for name, text in zip(parts[1::2], parts[2::2]):
            for marker, original in inserted.items():
                text = text.replace(f'/* {marker} */', original)
            if 'ORIGINAL_' in text:
                raise RuntimeError(f'Unfilled marker in {name}')
            out.append((name, text))
    events = c.read(SERVER / 'core/game/event/Events.kt')
    selected = re.findall(r'(?m)^data class Dialogue\w+Event\(.*\) : Event$', events)
    if len(selected) != 3:
        raise RuntimeError(f'dialogue events changed: {selected}')
    out.append(('core/game/event/Events.kt', 'package core.game.event\nimport core.game.dialogue.DialoguePlugin\n' + '\n'.join(selected) + '\n'))
    return out


CONSTANT_KINDS = ['Items', 'NPCs']


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
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    adapted = []
    for name, text in bundles(markers()):
        adapted.append(text)
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'cases.py')
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    # The unchanged DialogueLabeller's commented snippets name placeholder constants; check the adapters' (BankerNPC's).
    checked = constants_match_names(os.pathsep.join(map(str, libs)), adapted)
    return lock, classpath, checked


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Xss4m', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Oracle'], input=payload, cwd=ROOT, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Index', 'Cast', 'Number', 'Binding')


def generate(check):
    lock, classpath, checked = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked action every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, len(row['expected'][sweep]['events']) + 1):
            actions = [dict(a, failAt=at) if k == sweep else a for k, a in enumerate(case['actions'][:sweep + 1])]
            failing.append(dict(case, name=f"{case['name']} fail {at}", actions=actions))
    rows += execute(classpath, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for action, step in zip(row['actions'], row['expected']):
            error = step['error']
            if error not in EXPECTED_ERRORS or (error == 'Binding' and 'failAt' not in action and 'fail' not in json.dumps(row.get('files', []))
                                                and 'createFails' not in json.dumps(row['actions'])):
                mistakes.setdefault(error, (number, row.get('name'), action))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for action, step in zip(row['actions'], row['expected']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if step['error']:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    # A failure case repeats its designed case up to the swept action: keep only that action's snapshot.
    for row in rows[len(planned):]:
        row['expected'] = row['expected'][-1:]
        row['onlyLast'] = True
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=corpus.groups(cases),
                    designedCases=len(planned), failureCases=len(failing))
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), scope=SCOPE)
    summary = (f'{len(rows)} original dialogue DSL cases ({len(planned)} designed, {len(failing)} failure cases, '
               f'{coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('dialogue DSL fixtures/provenance differ; inspect before regeneration')
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
