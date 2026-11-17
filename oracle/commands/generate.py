"""The commands oracle: the original command registry after boot, and the command system, quick chat and abuse
report bodies run on JDK 11.

Part one (registry.py) boots the whole unchanged server as oracle/world-trace does and writes what CommandMapping and
the legacy CommandSet enum hold afterwards; it is accepted only when the boot record is byte-identical to the
world-trace fixture's and two fresh runs agree.

Part two compiles unchanged: Command, CommandSystem, CommandPlugin, the legacy CommandSet enum, Privilege,
sets/CommandSet and the Moderation, System, Teleport and PlayerAttribute command sets; AbuseReport and Rule;
QCRepository, QuickChatPacketHandler (with QCPacketType) and Packet; ChatMessage, Rights, UserAccountInfo, EquipmentSlot,
InputType, Globals, StringUtils, IoBuffer, PacketHeader, IncomingPacket, ISAACCipher, ByteBufferUtils, Plugin,
Initializable and the generated proto.management messages. Original text inserted at generation time: the COMMAND,
REPORT_ABUSE and QUICKCHAT decoders of Decoders530 and its companion process; the Command, ReportAbuse and QuickChat
branches of PacketProcessor.process; ServerConstants.TELEPORT_DESTINATIONS; Player.getRights and
PlayerDetails.getRights; Skills.SKILL_NAME, getSkillByName and getExperienceByLevel; the Log enum. Everything else is a
recording adapter from adapters.py answering from the case. The compiled classes pass through oracle/social's
tool/Rewrite.java (wall clock reads become recorded case inputs) and oracle/social's Launcher.java runs each case in
its own class loader. Every recorded call of the marked operations is then made to fail once in turn.
Java/Kotlin are test oracles only; --check never rewrites a fixture and nothing here downloads.
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
RUN = ROOT / 'runs/commands-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/commands'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('commands_adapters', HERE / 'adapters.py')
corpus = load('commands_cases', HERE / 'cases.py')
registry = load('commands_registry', HERE / 'registry.py')

ORIGINAL = [
    'core/game/system/command/Command.kt',
    'core/game/system/command/CommandSystem.kt',
    'core/game/system/command/CommandPlugin.kt',
    'core/game/system/command/CommandSet.java',
    'core/game/system/command/Privilege.kt',
    'core/game/system/command/sets/CommandSet.kt',
    'core/game/system/command/sets/ModerationCommandSet.kt',
    'core/game/system/command/sets/SystemCommandSet.kt',
    'core/game/system/command/sets/TeleportCommandSet.kt',
    'core/game/system/command/sets/PlayerAttributeCommandSet.kt',
    'core/game/global/report/AbuseReport.java',
    'core/game/global/report/Rule.java',
    'core/net/packet/QCRepository.kt',
    'core/net/packet/in/QuickChatPacketHandler.kt',
    'core/net/packet/in/Packet.kt',
    'core/net/packet/IoBuffer.java',
    'core/net/packet/PacketHeader.java',
    'core/net/packet/IncomingPacket.java',
    'core/cache/crypto/ISAACCipher.kt',
    'core/cache/misc/buffer/ByteBufferUtils.kt',
    'core/tools/StringUtils.java',
    'core/tools/CP1252.java',
    'core/tools/Globals.kt',
    'core/game/world/update/flag/context/ChatMessage.kt',
    'core/game/node/entity/player/info/Rights.java',
    'core/auth/UserAccountInfo.kt',
    'core/api/EquipmentSlot.kt',
    'core/api/InputType.kt',
    'core/plugin/Plugin.java',
    'core/plugin/Initializable.java',
]
SCOPE = (
    'Registry: the whole unchanged server booted as oracle/world-trace boots it (boot record byte-identical to the '
    'world-trace fixture, two fresh JVM runs identical); CommandMapping in getNames() order with each command\'s name, '
    'privilege, usage, description and handler class, the legacy CommandSet plugin lists and getPageIndices(0..3). '
    'Behaviour: on JDK 11 with Kotlin 1.8.20, each case in a fresh class loader, the unchanged Command, CommandSystem, '
    'CommandPlugin, CommandSet (legacy enum), Privilege, sets.CommandSet, ModerationCommandSet, SystemCommandSet, '
    'TeleportCommandSet, PlayerAttributeCommandSet, AbuseReport, Rule, QCRepository, QuickChatPacketHandler, Packet, '
    'ChatMessage, Rights, UserAccountInfo, EquipmentSlot, InputType, Globals, StringUtils, IoBuffer, PacketHeader, '
    'IncomingPacket, ISAACCipher, ByteBufferUtils, Plugin, Initializable and proto.management; original text inserted: '
    'the COMMAND, REPORT_ABUSE and QUICKCHAT decoders and Decoders530.process, the Command, ReportAbuse and QuickChat '
    'branches of PacketProcessor.process, ServerConstants.TELEPORT_DESTINATIONS, Player/PlayerDetails.getRights, '
    'Skills.SKILL_NAME/getSkillByName/getExperienceByLevel and the Log enum. Recording adapters: the repository name '
    'map, account storage and authenticator, ServerStore, the pulse runner (pulses, tasks, input handlers and the '
    'csvmodcr coroutine are kept and run by later operations), messages, chat, dialogues, zones, PlayerMonitor, '
    'teleports, attributes, containers and items, ChargedItem, the system manager, the cache index 24 files, data maps, '
    'item definitions, skills, slayer, clans, the management queue, the chat mask and Discord. The four sets register '
    'in the scan order; three legacy plugins are linked to ADMINISTRATOR as the boot links the originals; content '
    'commands are case-registered stand-ins. Wall clock reads are recorded case inputs (tool/Rewrite.java of the social '
    'oracle). Not driven: ::shutdown (exitProcess), Rule.canRequest, the Spawn, Stats, Misc and development sets and the '
    'content command bodies.'
)


def balanced(text, start):
    """The text from `start` through the parenthesis that closes the first one after it."""
    opening = text.index('(', start)
    depth = 0
    for i in range(opening, len(text)):
        if text[i] == '(':
            depth += 1
        elif text[i] == ')':
            depth -= 1
            if depth == 0:
                return text[start:i + 1]
    raise RuntimeError('unbalanced parentheses')


def markers():
    read = lambda name: c.read(SERVER / name)
    constants = read('core/ServerConstants.kt')
    player = read('core/game/node/entity/player/Player.java')
    details = read('core/game/node/entity/player/info/PlayerDetails.java')
    skills = read('core/game/node/entity/skill/Skills.java')
    logger = read('core/tools/SystemLogger.kt')
    decoders = read('core/net/packet/in/Decoders530.kt')
    processor = read('core/net/packet/PacketProcessor.kt')
    quick = re.search(r'^\s*is Packet\.QuickChat -> QCRepository\.sendQC\(.*\)$', processor, re.M)
    names = re.search(r'^\s*public static final String\[\] SKILL_NAME = \{[^}]*\};', skills, re.M)
    if quick is None or names is None:
        raise RuntimeError('a marked line moved')
    return {
        'ORIGINAL_TELEPORT_DESTINATIONS': balanced(constants, constants.index('val TELEPORT_DESTINATIONS = arrayOf(')),
        'ORIGINAL_PLAYER_GET_RIGHTS': c.method(player, 'public Rights getRights() {'),
        'ORIGINAL_DETAILS_GET_RIGHTS': c.method(details, 'public Rights getRights() {'),
        'ORIGINAL_SKILL_NAME': names.group(0).strip(),
        'ORIGINAL_GET_EXPERIENCE_BY_LEVEL': c.method(skills, 'public int getExperienceByLevel(int level) {'),
        'ORIGINAL_GET_SKILL_BY_NAME': c.method(skills, 'public static int getSkillByName(final String name) {'),
        'ORIGINAL_LOG_ENUM': c.method(logger, 'enum class Log {'),
        'ORIGINAL_DECODERS': ',\n    '.join(c.method(decoders, s) for s in ['COMMAND(44) {', 'REPORT_ABUSE(99) {', 'QUICKCHAT(167) {']),
        'ORIGINAL_DECODERS_COMPANION': c.method(decoders, 'companion object {'),
        'ORIGINAL_PROCESS_ARMS': '\n            '.join([c.method(processor, 'is Packet.Command -> {'),
                                                        c.method(processor, 'is Packet.ReportAbuse -> {'), quick.group(0).strip()]),
    }


def pinned(name, sha256):
    target = RUN / name
    candidates = [target, ROOT / 'runs/world-trace-oracle/jars' / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == sha256), None)
    if jar is None:
        raise RuntimeError(f'{name} matching its pin was not found under runs/; this generator never downloads')
    if jar != target:
        shutil.copyfile(jar, target)
    return target


def build():
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    annotations = next(path for path in jars if path.name.startswith('annotations-'))
    compiler = next(path for path in jars if path.name.startswith('kotlin-compiler-embeddable-'))
    world = json.loads(c.read(ROOT / 'oracle/world-trace/artifacts.lock.json'))
    json_jar = pinned('json-simple-1.1.1.jar', json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))['sha256'])
    protobuf = pinned('protobuf-java-3.19.4.jar', world['protobuf-java-3.19.4.jar']['sha256'])
    constlib = pinned('ConstLib-1.4.28.jar', json.loads(c.read(ROOT / 'oracle/bank/constants.lock.json'))['sha256'])
    libraries = [stdlib, annotations, json_jar, protobuf, constlib]
    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    for path in sorted((SERVER / 'proto/management').glob('*.java')):
        c.read(path)
        java.append(path)
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
    for helper in ['adapters.py', 'cases.py', 'Support.java']:
        c.read(HERE / helper)
    launcher_source = ROOT / 'oracle/social/Launcher.java'
    rewrite_source = ROOT / 'oracle/social/tool/Rewrite.java'
    c.read(launcher_source)
    c.read(rewrite_source)
    java.append(HERE / 'Support.java')
    compiled, classes, launcher, tool = RUN / 'compiled', RUN / 'classes', RUN / 'launcher', RUN / 'tool'
    for path in [compiled, classes, launcher, tool]:
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libraries)), '-d', compiled, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [compiled, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', compiled, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', compiler, '-d', tool, rewrite_source])
    census_path = RUN / 'census.json'
    c.run(['java', '-cp', os.pathsep.join(map(str, [tool, compiler])), 'Rewrite', compiled, classes, census_path])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-d', launcher, launcher_source])
    return libraries, launcher, classes, json.loads(census_path.read_text(encoding='utf-8'))


def execute(libraries, launcher, classes, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    work = RUN / 'work'
    work.mkdir(exist_ok=True)
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode('utf-8')
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-Duser.timezone=UTC', '-cp',
               launcher, 'oracle.Launcher', classes, *libraries]
    result = subprocess.run(list(map(str, command)), cwd=work, input=payload, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-500:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'IllegalState', 'Null', 'Index', 'NumberFormat', 'Cast', 'BufferUnderflow', 'Binding')


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    # oracle/world-trace loads its own copy of the compiler helpers.
    registry.c.JAVA_HOME = c.JAVA_HOME
    record, registry_provenance = registry.dump(lock)
    registry_data = (json.dumps(record, sort_keys=True, indent=1, ensure_ascii=True) + '\n').encode()

    libraries, launcher, classes, census = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(libraries, launcher, classes, cases)
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        calls = [e for e in row['expected'][sweep]['events'] if e[0] not in corpus.NOTES]
        for at in range(1, len(calls) + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(libraries, launcher, classes, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['ops'], row['expected']):
            error = step['error'][0] if step['error'] else None
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, (number, op, step['error']))
            elif error == 'Binding' and 'failAt' not in op:
                mistakes.setdefault(error, (number, op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(HERE / 'registry.py')
    c.read(HERE / 'harness/oracle/worldtrace/CommandRegistry.java')
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if step['error']:
                errors[step['error'][0]] = errors.get(step['error'][0], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=corpus.groups(cases),
                    failureCases=len(failing))
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20',
        locale='en-US', timezone='UTC', scope=SCOPE, unchanged=ORIGINAL, rewrite=census,
        registry=dict(registry_provenance, commands=len(record['registry']), fixtureSha256=hashlib.sha256(registry_data).hexdigest()),
        caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(), coverage=coverage)
    summary = (f'{len(record["registry"])} registered commands; {len(rows)} original command/quick chat/abuse report cases '
               f'({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, {len(failing)} failure cases)')
    if check:
        same = (FIXTURES / 'oracle.jsonl').read_bytes() == data and (FIXTURES / 'registry.json').read_bytes() == registry_data
        if not same or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('commands fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'registry.json').write_bytes(registry_data)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
