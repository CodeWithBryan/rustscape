"""Run the original global handler listeners on JDK 11 and record what they do.

Compiled unchanged: the player option handlers (AttackOptionPlugin, FollowOptionPlugin, LoginValidationPlugin,
RequestOption), the tab interfaces (Logout, Settings, Stats, Equipment, Combat, Magic, Prayer, Quest, Clan, Emote, Music),
MainGameInterface, LoginInterface, DeathInterface, CharacterDesignInterface, WorldMapInterface, ExperienceInterface,
FriendsListInterface, RulesAndInfo/RulesListener, ScrollInterface, GenericItemSelect, AutocastSelectPlugin,
TeleotherInterface, RequestAssistInterface, BankDepositBoxInterface, BankChargeInterface, the bank booth, chest and
deposit-box listeners, LookAtOptionPlugin, SearchOptionPlugin, LadderManagingPlugin, NPCDepositListener and
DummyAttackPlugin, with the unchanged OptionHandler, ComponentPlugin, CloseEvent, Pulse, Plugin, InterfaceListener,
IntType, core.api.Container and IfaceSettingsBuilder. The members named in `markers` are original text inserted into the
adapters of adapters.py at generation time. Everything else is a recording adapter answering from the case.
Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/global-handlers-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/global-handlers'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('global_handlers_adapters', HERE / 'adapters.py')
corpus = load('global_handlers_cases', HERE / 'cases.py')

H = 'content/global/handlers/'
UNCHANGED = [
    H + 'player/AttackOptionPlugin.java',
    H + 'player/FollowOptionPlugin.java',
    H + 'player/LoginValidationPlugin.java',
    H + 'player/RequestOption.kt',
    H + 'iface/tabs/LogoutTabInterface.kt',
    H + 'iface/tabs/SettingsTabInterface.kt',
    H + 'iface/tabs/StatsTabInterface.kt',
    H + 'iface/tabs/EquipmentTabInterface.kt',
    H + 'iface/tabs/CombatTabInterface.java',
    H + 'iface/tabs/MagicTabInterface.kt',
    H + 'iface/tabs/PrayerTabInterface.kt',
    H + 'iface/tabs/QuestTabInterface.kt',
    H + 'iface/tabs/ClanTabInterface.kt',
    H + 'iface/tabs/EmoteTabInterface.kt',
    H + 'iface/tabs/MusicTabInterface.kt',
    H + 'iface/MainGameInterface.kt',
    H + 'iface/LoginInterface.kt',
    H + 'iface/DeathInterface.kt',
    H + 'iface/CharacterDesignInterface.java',
    H + 'iface/WorldMapInterface.kt',
    H + 'iface/ExperienceInterface.kt',
    H + 'iface/FriendsListInterface.kt',
    H + 'iface/RulesAndInfo.kt',
    H + 'iface/ScrollInterface.kt',
    H + 'iface/GenericItemSelect.kt',
    H + 'iface/AutocastSelectPlugin.java',
    H + 'iface/TeleotherInterface.kt',
    H + 'iface/RequestAssistInterface.java',
    H + 'iface/bank/BankDepositBoxInterface.kt',
    H + 'iface/bank/BankChargeInterface.kt',
    H + 'scenery/BankBoothListener.kt',
    H + 'scenery/BankChestListener.kt',
    H + 'scenery/BankDepositBoxListener.kt',
    H + 'scenery/LookAtOptionPlugin.java',
    H + 'scenery/SearchOptionPlugin.java',
    H + 'scenery/LadderManagingPlugin.java',
    H + 'npc/NPCDepositListener.kt',
    H + 'npc/DummyAttackPlugin.java',
    'core/game/interaction/OptionHandler.java',
    'core/game/component/ComponentPlugin.java',
    'core/game/component/CloseEvent.java',
    'core/game/system/task/Pulse.java',
    'core/plugin/Plugin.java',
    'core/game/interaction/InterfaceListener.kt',
    'core/game/interaction/IntType.kt',
    'core/api/Container.kt',
    'core/api/IfaceSettingsBuilder.kt',
    'core/game/global/action/ClimbActionHandler.java',
    'core/game/global/action/SpecialLadders.java',
    'core/game/global/action/LadderAchievementCheck.java',
    'core/game/world/map/Direction.java',
]
SCOPE = (
    'The global handlers listed in `unchanged` compiled unchanged on JDK 11 with Kotlin 1.8.20 and the published ConstLib '
    '1.4.28 (Components, Items, NPCs, Scenery, Sounds and Animations values checked against the suffixes of the names '
    'used), together with the unchanged OptionHandler, ComponentPlugin, CloseEvent, Pulse, Plugin, InterfaceListener, '
    'IntType, core.api.Container, IfaceSettingsBuilder, ClimbActionHandler (with ClimbDialogue), SpecialLadders, '
    'LadderAchievementCheck and Direction. Original text inserted into adapters at generation time: the '
    'WeaponInterfaces constants with their component ids, SpellBook constants, PrayerType and DiaryType constant names, '
    'the Skills ids, StringUtils.formatDisplayName and ContentAPI.restrictForIronman. Everything else is a recording '
    'adapter answering from the case: the InterfaceListeners, InteractionListeners and ComponentDefinition registries '
    '(registration is recorded in order), Option.setHandler, SceneryDefinition.setOptionHandler and handler maps, every '
    'other ContentAPI function, the player managers (settings, interface manager, packet dispatch, locks, zone monitor, '
    'prayer, music, quests, diaries, clans, communication, familiars, skull, ironman, timers, saved data), containers, '
    'pulses and the world pulser, MovementPulse construction, DeathTask.getContainers, EquipHandler.unequip, '
    'RegionManager (objects, teleport permission, clipping flags), DialoguePlugin and the interpreter, BankerNPC.checkLunarIsleRestriction, BankUtils.transferX, QuestTabUtils.showRequirementsInterface, '
    'Repository, the bots, activities, MS packets and node fields. GlobalData.setLootShareDelay records its value '
    'relative to the clock the original reads itself.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


def names(text, pattern, what):
    found = re.findall(pattern, text, re.M)
    if not found:
        raise RuntimeError(f'no {what} constants')
    return found


def markers():
    read = lambda name: c.read(SERVER / name)
    weapons = names(read('core/game/node/entity/combat/equipment/WeaponInterface.java'),
                    r'^\t\t([A-Z_0-9]+)\(Components\.([A-Z_0-9]+),', 'WeaponInterfaces')
    books = read('core/game/node/entity/player/link/SpellBookManager.java')
    spell_books = re.findall(r'^\t\t(MODERN|ANCIENT|LUNAR)\((\d+)\)', books, re.M)
    if [name for name, _ in spell_books] != ['MODERN', 'ANCIENT', 'LUNAR']:
        raise RuntimeError(f'SpellBook constants changed: {spell_books}')
    prayers = names(read('core/game/node/entity/player/link/prayer/PrayerType.java'), r'^\t([A-Z_0-9]+)\(\d', 'PrayerType')
    diaries = names(read('core/game/node/entity/player/link/diary/DiaryType.java'), r'^(?:\t| {4})([A-Z_0-9]+)\("', 'DiaryType')
    return {
        'ORIGINAL_WEAPON_INTERFACES': ', '.join(f'{name}(Components.{component})' for name, component in weapons),
        'ORIGINAL_SPELL_BOOKS': ', '.join(f'{name}({value})' for name, value in spell_books),
        'ORIGINAL_PRAYER_TYPES': ', '.join(prayers),
        'ORIGINAL_DIARY_TYPES': ', '.join(diaries),
        'ORIGINAL_SKILL_IDS': lines(read('core/game/node/entity/skill/Skills.java'), 'public static final int ATTACK = 0,'),
        'ORIGINAL_FORMAT_DISPLAY_NAME': c.method(read('core/tools/StringUtils.java'), 'public static String formatDisplayName(String name) {'),
        'ORIGINAL_RESTRICT_FOR_IRONMAN': c.method(read('core/api/ContentAPI.kt'),
                                                  'fun restrictForIronman(player: Player, restriction: IronmanMode, action: () -> Unit) {'),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/global-handlers-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


CONSTANT_KINDS = ['Items', 'Components', 'Sounds', 'NPCs', 'Scenery', 'Animations']


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
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
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
    summary = (f'{len(rows)} original global handler cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('global handler fixtures/provenance differ; inspect before regeneration')
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
