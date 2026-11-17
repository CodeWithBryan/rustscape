"""Run the unchanged teleport classes on JDK 11 and record what they do, tick by tick.

Compiled unchanged: TeleportManager.java, TeleportPulse.java, Pulse.java,
Animator.java, Animation.java, Graphics.java, EntityFlag.kt, Clocks.kt,
ActionLocks.java, Lock.java, LockElapse.java, WalkingQueue.java, Point.java,
Direction.java, ClipMaskSupplier.java, ZoneRestriction.java and Audio.kt. The
members named in `markers` are original text inserted into the adapters of
adapters.py at generation time. Everything else is a recording adapter
answering from the case. Java/Kotlin are test oracles only; --check never
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
RUN = ROOT / 'runs/teleport-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/teleport'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('teleport_adapters', HERE / 'adapters.py')
corpus = load('teleport_cases', HERE / 'cases.py')

UNCHANGED = [
    'core/game/node/entity/player/link/TeleportManager.java',
    'core/game/node/entity/player/link/TeleportPulse.java',
    'core/game/system/task/Pulse.java',
    'core/game/node/entity/impl/Animator.java',
    'core/game/world/update/flag/context/Animation.java',
    'core/game/world/update/flag/context/Graphics.java',
    'core/game/world/update/flag/EntityFlag.kt',
    'core/game/interaction/Clocks.kt',
    'core/game/node/entity/lock/ActionLocks.java',
    'core/game/node/entity/lock/Lock.java',
    'core/game/node/entity/lock/LockElapse.java',
    'core/game/node/entity/impl/WalkingQueue.java',
    'core/game/world/map/Point.java',
    'core/game/world/map/Direction.java',
    'core/game/world/map/path/ClipMaskSupplier.java',
    'core/game/world/map/zone/ZoneRestriction.java',
    'core/game/node/entity/player/link/audio/Audio.kt',
]
ENTITY = ['public void lock()', 'public void lock(int time)', 'public void unlock()', 'public boolean graphics(Graphics graphics)',
          'public boolean animate(Animation animation)', 'public Player asPlayer()', 'public boolean isPlayer()',
          'public boolean isTeleBlocked()', 'public boolean isTeleporting()', 'public void setAttribute(String key, Object value)',
          'public <T> T getAttribute(String key)', 'public <T> T getAttribute(String string, T fail)', 'public void removeAttribute(String string)']
ATTRIBUTES = ['public void setAttribute(String key, Object value)', 'public <T> T getAttribute(String key)',
              'public <T> T getAttribute(String string, T fail)', 'public void removeAttribute(String string)']
PROPERTIES = ['public boolean isTeleporting()', 'public void setTeleporting(boolean teleporting)', 'public Location getTeleportLocation()',
              'public void setTeleportLocation(Location teleportLocation)']
MONITOR = ['public boolean isRestricted(ZoneRestriction restriction)', 'public boolean isRestricted(int flag)',
           'public boolean teleport(int type, Node node)', 'private boolean canTeleportByJewellery(int type, Node node)']
SOUNDS = ['TELEPORT_ALL_200', 'TELEPORT_REVERSE_201', 'BLOCK_TELEPORT_197', 'POH_TABLET_BREAK_979', 'TELE_OTHER_CAST_199',
          'FT_FAIRY_TELEPORT_1098']
SCOPE = (
    'Unchanged TeleportManager (every TeleportType and its pulse), TeleportPulse, Pulse, Animator, Animation, Graphics, EntityFlag, '
    'Clocks, ActionLocks, Lock, LockElapse, WalkingQueue, Point, Direction, ClipMaskSupplier, ZoneRestriction and Audio on JDK 11 '
    'with Kotlin 1.8.20 and the published ConstLib 1.4.28 (Sounds, Items). Original text inserted into adapters at generation time: '
    'Entity lock/unlock/graphics/animate/asPlayer/isPlayer/isTeleBlocked/isTeleporting and its attribute members; GameAttributes '
    'set/get/remove; the Properties teleport members; ImpactHandler.setDisabledTicks; ZoneMonitor MID_WILDY_TELEPORT_JEWELLERY, '
    'isRestricted, teleport and canTeleportByJewellery; MapZone.isRestricted; AnimationDefinition getDuration/getDurationTicks; '
    'ContentAPI hasTimerActive/getTimer and the playAudio/playGlobalAudio signatures; the Location class header, constructors, '
    'coordinate setters, region getters, transform(Int, Int, Int), create overloads and LocationHash; the last statement of '
    'RegionManager.move. Everything else is a recording adapter answering from the case: the world clock (every read recorded), '
    'the pulser (submission recorded, pulses queued for the harness runner), timers, attribute map operations, the spell book, '
    'dialogue and interface close, script delay, messages, audio, update-mask registration, animation definitions, '
    'PulseManager.clear, the chunk part of RegionManager.move, ZoneMonitor.updateLocation and each MapZone.teleport answer. '
    'The harness runner repeats PulseRunner.updateAll (prefix count, update, re-queue, the caught log and stack trace) without '
    'its telemetry; the per-tick driver (pulses, then every walking queue, then the reset of the teleporting and scene flags with '
    'the last scene set where a rebuild was flagged, then the clock) is an oracle input, not a port. No random generator is '
    'reached by these sources.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def between(text, start, end):
    first = text.index(start)
    return text[first:text.index(end, first)].rstrip()


def markers():
    read = lambda name: c.read(SERVER / name)
    entity = read('core/game/node/entity/Entity.java')
    attributes = read('core/game/node/entity/impl/GameAttributes.java')
    properties = read('core/game/node/entity/impl/Properties.java')
    impact = read('core/game/node/entity/combat/ImpactHandler.java')
    monitor = read('core/game/world/map/zone/ZoneMonitor.java')
    zone = read('core/game/world/map/zone/MapZone.java')
    definition = read('core/cache/def/impl/AnimationDefinition.java')
    api = read('core/api/ContentAPI.kt')
    location = read('core/game/world/map/Location.kt')
    region = read('core/game/world/map/RegionManager.kt')
    if 'entity.zoneMonitor.updateLocation(entity.walkingQueue.footPrint)' not in c.method(region, 'fun move(entity: Entity, src: Location?, dst: Location)'):
        raise RuntimeError('RegionManager.move changed')
    header = between(location, 'class Location(var hash: LocationHash)', '    init {')
    header = header.replace(' : Node(null, null) {', ' {')
    if 'Node(' in header or 'RandomFunction' not in header:
        raise RuntimeError('Location header changed')
    # The randomizing constructor draws from RandomFunction; nothing here calls it.
    header = '\n'.join(l for l in header.split('\n') if 'randomizer' not in l and 'RandomFunction' not in l)
    audio = lambda signature: '@JvmOverloads\n' + line(api, signature)
    events = read('core/game/event/Events.kt')
    return {
        'ORIGINAL_ATTRIBUTE_EVENTS': '\n'.join([line(events, 'data class AttributeSetEvent'), line(events, 'data class AttributeRemoveEvent')]),
        'ORIGINAL_DEFINITION_DURATIONS': '\n\n'.join(c.method(definition, s) for s in ['public int getDuration()', 'public int getDurationTicks()']),
        'ORIGINAL_LOCATION_HEADER': header,
        'ORIGINAL_LOCATION_COORDINATES': between(location, '    var x: Int', '    companion object {'),
        'ORIGINAL_LOCATION_REGION': '\n    '.join([line(location, 'val regionX: Int get()'), line(location, 'val regionY: Int get()')]),
        'ORIGINAL_LOCATION_TRANSFORM': line(location, 'fun transform(diffX: Int, diffY: Int, z: Int)'),
        'ORIGINAL_LOCATION_CREATE': '\n        '.join([line(location, '@JvmStatic @JvmOverloads fun create(x: Int, y: Int, z: Int = 0)'),
                                                       line(location, '@JvmStatic fun create(location: Location)')]),
        'ORIGINAL_LOCATION_HASH': c.method(location, 'value class LocationHash('),
        'ORIGINAL_GAME_ATTRIBUTES': '\n\n'.join(c.method(attributes, s) for s in ATTRIBUTES),
        'ORIGINAL_PROPERTIES': '\n\n'.join(c.method(properties, s) for s in PROPERTIES),
        'ORIGINAL_SET_DISABLED_TICKS': c.method(impact, 'public void setDisabledTicks(int ticks)'),
        'ORIGINAL_MAP_ZONE_RESTRICTED': c.method(zone, 'public boolean isRestricted(int flag)'),
        'ORIGINAL_JEWELLERY': between(monitor, 'static final Set<Integer> MID_WILDY_TELEPORT_JEWELLERY', ');') + '\n\t);',
        'ORIGINAL_ZONE_MONITOR': '\n\n'.join(c.method(monitor, s) for s in MONITOR),
        'ORIGINAL_ENTITY_METHODS': '\n\n'.join(c.method(entity, s) for s in ENTITY),
        'ORIGINAL_HAS_TIMER_ACTIVE': c.method(api, 'fun hasTimerActive (entity: Entity, identifier: String) : Boolean'),
        'ORIGINAL_GET_TIMER': c.method(api, 'fun getTimer (entity: Entity, identifier: String) : RSTimer?'),
        'ORIGINAL_PLAY_AUDIO_HEADER': audio('fun playAudio(player: Player, id: Int,'),
        'ORIGINAL_PLAY_GLOBAL_AUDIO_HEADER': audio('fun playGlobalAudio(location: Location, id: Int,'),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/teleport-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def sounds_match_names(classpath):
    """Every Sounds constant TeleportManager names carries its id as its suffix; check the jar's values agree."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.Sounds'], capture_output=True, check=True).stdout.decode()
    names = sorted(set(re.findall(r'Sounds\.\s*([A-Z0-9_]+)', c.read(SERVER / UNCHANGED[0]))))
    if names != sorted(SOUNDS):
        raise RuntimeError(f'TeleportManager names other sounds: {names}')
    for name in names:
        found = re.search(r'int ' + name + r' = (-?\d+);', text)
        if found is None or found.group(1) != name.rsplit('_', 1)[1]:
            raise RuntimeError(f'Sounds.{name} differs from its suffix')
    items = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.Items'], capture_output=True, check=True).stdout.decode()
    jewellery = re.findall(r'Items\.([A-Z0-9_]+)', markers()['ORIGINAL_JEWELLERY'])
    ids = []
    for name in jewellery:
        found = re.search(r'int ' + name + r' = (-?\d+);', items)
        if found is None:
            raise RuntimeError(f'Items.{name} missing')
        ids.append(int(found.group(1)))
    return names, ids


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libs = [stdlib, *libraries()]
    kotlin, java = [], []
    for name in UNCHANGED:
        path = SERVER / name
        c.read(path)
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
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    sounds, jewellery = sounds_match_names(os.pathsep.join(map(str, libs)))
    return lock, classpath, sounds, jewellery


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support'], input=payload, cwd=ROOT, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-4000:] + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    return rows


EXPECTED_ERRORS = {None, 'Null', 'ClassCast', 'ConcurrentModification', 'Binding'}


def generate(check):
    lock, classpath, sounds, jewellery = build()
    planned = corpus.cases()
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, row['expected'][sweep]['calls'] + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'])]
            failing.append(dict(case, ops=ops, group=case['group'] + '-failure'))
    rows += execute(classpath, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['input']['ops'], row['expected']):
            if step['error'] not in EXPECTED_ERRORS or (step['error'] == 'Binding' and 'failAt' not in op):
                mistakes.setdefault(step['error'], (number, op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors, groups = {}, {}, {}, {}
    for row in rows:
        groups[row['input']['group']] = groups.get(row['input']['group'], 0) + 1
        for op, step in zip(row['input']['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if step['error']:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=dict(sorted(groups.items())),
                    failureCases=len(failing))
    for needed in ['ticks', 'submit', 'get_timer', 'message', 'zone_teleport', 'script_delay', 'close_dialogue', 'get_attribute',
                   'set_attribute', 'remove_attribute', 'clear_pulses', 'close_interfaces', 'spell_book', 'global_audio', 'audio',
                   'definition', 'register', 'region_move', 'zone_update', 'log', 'print_stack_trace']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', soundsCheckedAgainstNames=sounds, jewelleryItemIds=jewellery), scope=SCOPE)
    summary = (f'{len(rows)} original teleport cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('teleport fixtures/provenance differ; inspect before regeneration')
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
