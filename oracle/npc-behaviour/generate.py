"""Run the unchanged aggression classes and NPCBehavior, with the original NPC,
Entity and Player members on the combat path, on JDK 11 and record what they do.

Compiled unchanged: AggressiveHandler.java, AggressiveBehavior.java,
NPCBehavior.kt, ContentInterface.kt and ClipMaskSupplier.java. The members
named in `markers` are original text inserted into the adapters of
adapters.py at generation time. Everything else is a recording adapter.
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
RUN = ROOT / 'runs/npc-behaviour-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/npc-behaviour'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('npc_behaviour_adapters', HERE / 'adapters.py')
corpus = load('npc_behaviour_cases', HERE / 'cases.py')

UNCHANGED = [
    'core/game/node/entity/npc/agg/AggressiveHandler.java',
    'core/game/node/entity/npc/agg/AggressiveBehavior.java',
    'core/game/node/entity/npc/NPCBehavior.kt',
    'core/api/ContentInterface.kt',
    'core/game/world/map/path/ClipMaskSupplier.java',
]
ENTITY = ['public boolean inCombat()', 'public void fullRestore()', 'public void checkImpact(BattleState state)',
          'public void onImpact(final Entity entity, BattleState state)',
          'public boolean isAttackable(Entity entity, CombatStyle style, boolean message)',
          'public boolean continueAttack(Entity target, CombatStyle style, boolean message)',
          'public void sendImpact(BattleState state)', 'public boolean canSelectTarget(Entity target)',
          'public boolean isIgnoreMultiBoundaries(Entity victim)', 'public boolean isInvisible()',
          'public void setInvisible(boolean invisible)']
NPC = ['public boolean isInvisible()', 'public boolean isHidden(Player player)', 'public void setDefaultBehavior()',
       'public void configureBossData()', 'public void checkImpact(BattleState state)',
       'public boolean isAttackable(Entity entity, CombatStyle style, boolean message)',
       'public boolean continueAttack(Entity target, CombatStyle style, boolean message)', 'public void tick()',
       'protected void onRespawn()', 'public void handleTickActions()', 'private boolean walkToNextDest()',
       'public int getNextWalk()', 'public void setNextWalk()', 'public void onRegionInactivity()',
       'public CombatSwingHandler getSwingHandler(boolean swing)', 'public boolean canAttack(final Entity victim)',
       'public boolean isIgnoreAttackRestrictions(Entity victim)', 'protected Location getMovementDestination()',
       'public boolean canStartCombat(Entity victim)', 'public NPCDefinition getDefinition()',
       'public boolean isAggressive()', 'public void setAggressive(boolean aggressive)',
       'public AggressiveHandler getAggressiveHandler()', 'public void setAggressiveHandler(AggressiveHandler handler)',
       'public int getRespawnTick()', 'public void setRespawnTick(int respawnTick)', 'public void setWalkRadius(int radius)',
       'public int getWalkRadius()', 'public boolean isNeverWalks()']
PLAYER = ['public boolean isAttackable(Entity entity, CombatStyle style, boolean message)',
          'public boolean continueAttack(Entity target, CombatStyle style, boolean message)',
          'public void onImpact(final Entity entity, BattleState state)']
SCOPE = (
    'Unchanged AggressiveHandler, AggressiveBehavior (DEFAULT and WILDERNESS), NPCBehavior, ContentInterface and ClipMaskSupplier on '
    'JDK 11 with Kotlin 1.8.20. Original text inserted into adapters at generation time: the NPC.java members isInvisible, isHidden, '
    'setDefaultBehavior, configureBossData, checkImpact, isAttackable, continueAttack, tick, onRespawn, handleTickActions, '
    'walkToNextDest, getNextWalk, setNextWalk, onRegionInactivity, getSwingHandler, canAttack, isIgnoreAttackRestrictions, '
    'getMovementDestination, canStartCombat and their field accessors; the Entity.java members inCombat, fullRestore, checkImpact, '
    'onImpact, isAttackable, continueAttack, sendImpact, canSelectTarget, isIgnoreMultiBoundaries, isInvisible, setInvisible; the '
    'Player.java members isAttackable, continueAttack, onImpact; Region.isTolerated; BattleState.neutralizeHits; RandomFunction '
    'random/randomize/nextBool; Location packing, region, regionId, transform, withinDistance and getDistance; Vector normalized, '
    'magnitude, times and betweenLocs; the pinned ServerConstants.MAX_PLAYERS value, Skills and EquipmentContainer slot constants and '
    'NPCConfigParser keys. Everything else is a recording adapter answering from the case: the combat pulse (a small model: attack '
    'sets the victim and the attacking flag unless the case turns that off, stop clears them), locks, zone monitors, DeathTask.isDead, '
    'WildernessZone, RegionManager lookups and local players, definitions and configuration, skills, timers, walking, pulses, masks, '
    'chunks, the repository, attributes (reads of the tick kernel\'s own four keys are not recorded: world::npc keeps them as fields), '
    'packets, the slayer task, the familiar owner and a recording NPC subclass and NPCBehavior subclass. RandomFunction.RANDOM is a '
    'seeded java.util.Random; java.lang.Math\'s generator is replaced once by a seeded one; the wall clock is the case clock. The '
    'harness clears NPCBehavior\'s private id map before each case. Not driven: AbstractNPC, IdleAbstractNPC, NPC.init/clear/transform, '
    'real CombatPulse/ImpactHandler/DeathTask bodies (their own corpora cover them) and whole fights through them.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def markers():
    read = lambda name: c.read(SERVER / name)
    entity = read('core/game/node/entity/Entity.java')
    npc = read('core/game/node/entity/npc/NPC.java')
    player = read('core/game/node/entity/player/Player.java')
    location = read('core/game/world/map/Location.kt')
    vector = read('core/api/utils/Vector.kt')
    random = read('core/tools/RandomFunction.java')
    constants = read('core/ServerConstants.kt')
    parser = read('core/game/system/config/NPCConfigParser.kt')
    within = c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)')
    max_players = re.search(r'var MAX_PLAYERS = (\d+)', constants).group(1)
    keys = re.findall(r'const val (AGGRESSIVE_RADIUS|AGGRESSIVE|RESPAWN_DELAY) = "(.*)"', parser)
    return {
        'ORIGINAL_MAX_PLAYERS': f'public static int MAX_PLAYERS = {max_players};',
        'ORIGINAL_RANDOM_METHODS': '\n'.join(c.method(random, s) for s in [
            'public static final int random(int a, int b)', 'public static final int random(int maxValue)',
            'public static int randomize(int value)', 'public static boolean nextBool()']),
        'ORIGINAL_IS_TOLERATED': c.method(read('core/game/world/map/Region.java'), 'public boolean isTolerated(Player player)'),
        'ORIGINAL_EQUIPMENT_SLOTS': line(read('core/game/container/impl/EquipmentContainer.java'), 'public static final int SLOT_HAT = '),
        'ORIGINAL_SKILL_CONSTANTS': line(read('core/game/node/entity/skill/Skills.java'), 'public static final int ATTACK = 0,'),
        'ORIGINAL_CONFIG_KEYS': ' '.join(f'public static final String {n} = "{v}";' for n, v in keys),
        'ORIGINAL_NEUTRALIZE': c.method(read('core/game/node/entity/combat/BattleState.java'), 'public void neutralizeHits()'),
        'ORIGINAL_ENTITY_METHODS': '\n\n'.join(c.method(entity, s) for s in ENTITY),
        'ORIGINAL_NPC_METHODS': '\n\n'.join(c.method(npc, s) for s in NPC),
        'ORIGINAL_PLAYER_METHODS': '\n\n'.join(c.method(player, s) for s in PLAYER),
        'ORIGINAL_LOCATION_MEMBERS': '\n    '.join([
            line(location, 'val region: Region get()'), line(location, 'val regionId: Int get()'),
            line(location, 'fun transform(diffX: Int, diffY: Int, z: Int)'), line(location, 'fun transform(vector: Vector)'),
            within.replace('@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)',
                           'fun withinDistance(other: Location, dist: Int)')]),
        'ORIGINAL_GET_DISTANCE': c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location)'),
        'ORIGINAL_LOCATION_HASH': c.method(location, 'value class LocationHash('),
        'ORIGINAL_VECTOR_MEMBERS': '\n    '.join(c.method(vector, s) for s in ['fun normalized()', 'fun magnitude()', 'operator fun times (other: Int)']),
        'ORIGINAL_BETWEEN_LOCS': c.method(vector, '@JvmStatic fun betweenLocs'),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/npc-behaviour-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


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
    return lock, classpath


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support'], input=payload, cwd=ROOT, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    return rows


def generate(check):
    lock, classpath = build()
    planned = corpus.cases()
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked operation every recorded call and draw fails once in turn.
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
        for op, step in zip(row['input']['ops'], row['expected']):
            if step['error'] not in (None, 'Null', 'Index', 'Binding') or (step['error'] == 'Binding' and 'failAt' not in op):
                mistakes.setdefault(step['error'], (number, op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['input']['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if step['error']:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=corpus.groups(cases),
                    failureCases=len(failing))
    for needed in ['local_players', 'math_random', 'next_int', 'region_for_id', 'name', 'combat_attack', 'combat_set_victim', 'face',
                   'zone_continue_attack', 'zone_is_pvpable', 'path_find', 'manual_hit', 'handle_recoil_effect', 'chunk_add',
                   'pulse_stop', 'skills_restore', 'behaviour_hook', 'class_hook', 'entity_tick', 'teleport', 'wander', 'construct_return']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED, scope=SCOPE)
    summary = (f'{len(rows)} original NPC behaviour cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('npc-behaviour fixtures/provenance differ; inspect before regeneration')
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
