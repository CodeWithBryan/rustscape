"""Run the unchanged social sources on JDK 11 and record what they do.

Compiled unchanged: CommunicationInfo, ClanRepository, ClanEntry, ClanMember,
ClanRank, Contact, GlobalChat, ManagementEvents, ChatMessage, the three social
outgoing packets and their contexts, IoBuffer, PacketHeader, Context,
OutgoingPacket, PacketWriteQueue, Packet, StringUtils, CP1252, Globals, Util,
Rights, UserAccountInfo, MSPacketRepository, ManagementServerState,
ContentInterface, Privilege and the generated proto.management messages.
Everything else is an adapter from adapters.py, with the original text of the
members named in SCOPE inserted at generation time. The compiled classes are
then passed through tool/Rewrite.java, which redirects the wall clock and
unseeded java.util.Random constructions to recorded case inputs. Launcher.java
runs each case in its own class loader. --check never updates a fixture, and
nothing here downloads: the pinned jars must already be under runs/.
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
RUN = ROOT / 'runs/social-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/social'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('social_adapters', HERE / 'adapters.py')
corpus = load('social_corpus', HERE / 'corpus.py')

ORIGINAL = [
    'core/game/system/communication/CommunicationInfo.java',
    'core/game/system/communication/ClanRepository.java',
    'core/game/system/communication/ClanEntry.java',
    'core/game/system/communication/ClanMember.java',
    'core/game/system/communication/ClanRank.java',
    'core/game/system/communication/Contact.java',
    'core/game/system/communication/GlobalChat.kt',
    'core/worker/ManagementEvents.kt',
    'core/game/world/update/flag/context/ChatMessage.kt',
    'core/net/packet/out/ContactPackets.java',
    'core/net/packet/out/CommunicationMessage.java',
    'core/net/packet/out/UpdateClanChat.java',
    'core/net/packet/context/ContactContext.java',
    'core/net/packet/context/MessageContext.java',
    'core/net/packet/context/ClanContext.java',
    'core/net/packet/Context.java',
    'core/net/packet/OutgoingPacket.java',
    'core/net/packet/PacketHeader.java',
    'core/net/packet/IoBuffer.java',
    'core/net/packet/PacketWriteQueue.kt',
    'core/net/packet/in/Packet.kt',
    'core/tools/StringUtils.java',
    'core/tools/CP1252.java',
    'core/tools/Globals.kt',
    'core/Util.java',
    'core/game/node/entity/player/info/Rights.java',
    'core/auth/UserAccountInfo.kt',
    'core/net/amsc/MSPacketRepository.java',
    'core/net/amsc/ManagementServerState.java',
    'core/api/ContentInterface.kt',
    'core/game/system/command/Privilege.kt',
]
SCOPE = (
    'Unchanged on JDK 11, each case in a fresh class loader: CommunicationInfo, ClanRepository, ClanEntry, ClanMember, ClanRank, Contact, '
    'GlobalChat, ManagementEvents, ChatMessage, ContactPackets, CommunicationMessage, UpdateClanChat, ContactContext, MessageContext, '
    'ClanContext, IoBuffer, PacketHeader, Context, OutgoingPacket, PacketWriteQueue, Packet, StringUtils, CP1252, Globals, Util, Rights, '
    'UserAccountInfo, MSPacketRepository, ManagementServerState, ContentInterface, Privilege and all generated proto.management messages. '
    'Original text inserted at generation time: the social branches of PacketProcessor.process (AddFriend, RemoveFriend, AddIgnore, '
    'RemoveIgnore, JoinClan, SetClanRank, KickFromClan, ChatMessage, ChatSetting, PrivateMessage), PacketProcessor.splitChatMessage and '
    'BASE_CHAT_CUTOFF; the ten social decoders of Decoders530 and its companion process; Login.sendMSEvents; Player getName, '
    'getCommunication, getUsername and sendLogoutEvents; Node getName; PlayerDetails accountInfo and communicationInfo fields, constructor, '
    'isMuted, getRights, getCommunication, getUsername and getDetails; Settings player and three chat fields, both updateChatSettings, '
    'getPublicChatSetting, getPrivateChatSetting and getTradeSetting; PacketRepository.send; Repository.getPlayerByName; '
    'WorldCommunicator state field and isEnabled; ContentAPI getAttribute and setAttribute; the Log enum. Recording adapters: '
    'attributes, action locks, the mask registry, game messages, interface strings and opening, varps, the pulse queue, logging, '
    'PlayerMonitor, account storage and the authenticator, command registration, the session write and outgoing cipher, the '
    'management-server session, the clan war activity, the repository player list and name map (case order). kotlinx.coroutines is a '
    'stand-in that runs the management coroutine on the calling thread when the case pumps it and stops it instead of blocking on an '
    'empty queue. Wall-clock reads and unseeded java.util.Random constructions are redirected call sites answered by the case (census '
    'below). UserAccountInfo.default is initialized before each case is recorded. Not exercised: real dispatcher threads, the socket, '
    'the management server, quick chat, clan wars, AIPlayer chat (PacketRepository never queues for a bot) and MessageLog.'
)


def line(text, pattern):
    match = re.search(pattern, text, re.M)
    if not match:
        raise RuntimeError(f'pattern not found: {pattern}')
    return match.group(0).strip()


def between(text, start, end):
    first = text.index(start)
    return text[first:text.index(end, first)].rstrip() + '\n'


def markers():
    source = lambda name: c.read(SERVER / name)
    node = source('core/game/node/Node.java')
    player = source('core/game/node/entity/player/Player.java')
    details = source('core/game/node/entity/player/info/PlayerDetails.java')
    settings = source('core/game/node/entity/player/link/Settings.java')
    api = source('core/api/ContentAPI.kt')
    logger = source('core/tools/SystemLogger.kt')
    communicator = source('core/net/amsc/WorldCommunicator.java')
    repository = source('core/net/packet/PacketRepository.java')
    players = source('core/game/world/repository/Repository.kt')
    processor = source('core/net/packet/PacketProcessor.kt')
    login = source('core/net/packet/in/Login.kt')
    decoders = source('core/net/packet/in/Decoders530.kt')
    return {
        'ORIGINAL_NODE_GET_NAME': c.method(node, 'public String getName()'),
        'ORIGINAL_PLAYER_GET_NAME': c.method(player, 'public String getName()'),
        'ORIGINAL_PLAYER_GET_COMMUNICATION': c.method(player, 'public CommunicationInfo getCommunication()'),
        'ORIGINAL_PLAYER_GET_USERNAME': c.method(player, 'public String getUsername()'),
        'ORIGINAL_PLAYER_SEND_LOGOUT_EVENTS': c.method(player, 'private void sendLogoutEvents()'),
        'ORIGINAL_DETAILS_ACCOUNT_FIELD': line(details, r'^\s*public UserAccountInfo accountInfo = UserAccountInfo\.createDefault\(\);'),
        'ORIGINAL_DETAILS_COMMUNICATION_FIELD': line(details, r'^\s*private final CommunicationInfo communicationInfo = new CommunicationInfo\(\);'),
        'ORIGINAL_DETAILS_CONSTRUCTOR': c.method(details, 'public PlayerDetails(String username)'),
        'ORIGINAL_DETAILS_IS_MUTED': c.method(details, 'public boolean isMuted()'),
        'ORIGINAL_DETAILS_GET_RIGHTS': c.method(details, 'public Rights getRights()'),
        'ORIGINAL_DETAILS_GET_COMMUNICATION': c.method(details, 'public CommunicationInfo getCommunication()'),
        'ORIGINAL_DETAILS_GET_USERNAME': c.method(details, 'public String getUsername()'),
        'ORIGINAL_DETAILS_GET_DETAILS': c.method(details, 'public static PlayerDetails getDetails('),
        'ORIGINAL_SETTINGS_PLAYER_FIELD': line(settings, r'^\s*private final Player player;'),
        'ORIGINAL_SETTINGS_PUBLIC_FIELD': line(settings, r'^\s*private int publicChatSetting = 0;'),
        'ORIGINAL_SETTINGS_PRIVATE_FIELD': line(settings, r'^\s*private int privateChatSetting = 0;'),
        'ORIGINAL_SETTINGS_TRADE_FIELD': line(settings, r'^\s*private int tradeSetting = 0;'),
        'ORIGINAL_SETTINGS_UPDATE_CHAT': c.method(settings, 'public void updateChatSettings() {'),
        'ORIGINAL_SETTINGS_UPDATE_CHAT_VALUES': c.method(settings, 'public void updateChatSettings(int pub, int priv, int trade)'),
        'ORIGINAL_SETTINGS_GET_PUBLIC': c.method(settings, 'public int getPublicChatSetting()'),
        'ORIGINAL_SETTINGS_GET_PRIVATE': c.method(settings, 'public int getPrivateChatSetting()'),
        'ORIGINAL_SETTINGS_GET_TRADE': c.method(settings, 'public int getTradeSetting()'),
        'ORIGINAL_API_GET_ATTRIBUTE': c.method(api, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T'),
        'ORIGINAL_API_SET_ATTRIBUTE': c.method(api, 'fun <T> setAttribute(entity: Entity, attribute: String, value: T)'),
        'ORIGINAL_LOG_ENUM': c.method(logger, 'enum class Log {'),
        'ORIGINAL_WORLD_COMMUNICATOR_STATE_FIELD': line(communicator, r'^\s*private static ManagementServerState state = ManagementServerState\.CONNECTING;'),
        'ORIGINAL_WORLD_COMMUNICATOR_IS_ENABLED': c.method(communicator, 'public static boolean isEnabled()'),
        'ORIGINAL_PACKET_REPOSITORY_SEND': c.method(repository, 'public static void send('),
        'ORIGINAL_REPOSITORY_GET_PLAYER_BY_NAME': c.method(players, 'fun getPlayerByName('),
        'ORIGINAL_PROCESS_FRIENDS': between(processor, '            is Packet.AddFriend ->', '            is Packet.ComponentItemAction,'),
        'ORIGINAL_PROCESS_CLANS': between(processor, '            is Packet.JoinClan -> {', '            is Packet.QuickChat ->'),
        'ORIGINAL_PROCESS_CHAT': between(processor, '            is Packet.ChatMessage -> {', '            is Packet.PacketCountUpdate ->'),
        'ORIGINAL_SPLIT_CHAT_MESSAGE': c.method(processor, 'fun splitChatMessage('),
        'ORIGINAL_BASE_CHAT_CUTOFF': line(processor, r'^\s*const val BASE_CHAT_CUTOFF = \d+'),
        'ORIGINAL_LOGIN_SEND_MS_EVENTS': c.method(login, 'private fun sendMSEvents('),
        'ORIGINAL_DECODERS_SOCIAL': between(decoders, '    CLAN_JOIN(104) {', '    /******************************************\n     * INPUT TRACKING'),
        'ORIGINAL_DECODER_CHAT_SETTINGS': between(decoders, '    CHAT_SETTINGS(157) {', '    CHAT_MESSAGE(237) {'),
        'ORIGINAL_DECODER_CHAT_MESSAGE': between(decoders, '    CHAT_MESSAGE(237) {', '    MUSIC_TRACK_FINISHED(137) {'),
        'ORIGINAL_DECODERS_COMPANION': c.method(decoders, 'companion object {'),
    }


def pinned(path, sha256):
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != sha256:
        raise RuntimeError(f'{path} is missing or differs from its pin; this generator never downloads')
    return path


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
        pinned(ROOT / 'runs/kotlin-1.8.20' / name, entry['sha256'])
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    annotations = next(path for path in jars if path.name.startswith('annotations-'))
    compiler = next(path for path in jars if path.name.startswith('kotlin-compiler-embeddable-'))
    json_jar = pinned(RUN / 'json-simple-1.1.1.jar', json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))['sha256'])
    world_artifacts = json.loads(c.read(ROOT / 'oracle/world-trace/artifacts.lock.json'))
    protobuf = pinned(RUN / 'protobuf-java-3.19.4.jar', world_artifacts['protobuf-java-3.19.4.jar']['sha256'])
    libraries = [stdlib, annotations, json_jar, protobuf]

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
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'corpus.py')
    c.read(HERE / 'Support.java')
    c.read(HERE / 'Launcher.java')
    c.read(HERE / 'tool/Rewrite.java')
    java.append(HERE / 'Support.java')

    compiled, classes, launcher, tool = RUN / 'compiled', RUN / 'classes', RUN / 'launcher', RUN / 'tool'
    for path in [compiled, classes, launcher, tool]:
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', compiled, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [compiled, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', compiled, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', compiler, '-d', tool, HERE / 'tool/Rewrite.java'])
    census_path = RUN / 'census.json'
    c.run(['java', '-cp', os.pathsep.join(map(str, [tool, compiler])), 'Rewrite', compiled, classes, census_path])
    census = json.loads(census_path.read_text(encoding='utf-8'))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-d', launcher, HERE / 'Launcher.java'])

    cases = corpus.requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-Duser.timezone=UTC', '-cp', launcher,
               'oracle.Launcher', classes, *libraries]
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=1200)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-500:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for case, row in zip(cases, rows):
        row['case'] = case
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    events, errors, operations = {}, {}, {}
    for row in rows:
        for event in row['setup']:
            events[event[0]] = events.get(event[0], 0) + 1
        for action, step in zip(row['case']['actions'], row['steps']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if step['error'] is not None:
                category = step['error'].split(':')[0]
                errors[category] = errors.get(category, 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(
        scenarios=len(rows), actions=sum(operations.values()), operations=dict(sorted(operations.items())),
        serviceCalls=sum(events.values()), events=dict(sorted(events.items())), errors=dict(sorted(errors.items())),
        injectedFailures=sum(len(at) for row in rows for at in row['case'].get('failures', {}).values()),
    )
    for needed in corpus.REQUIRED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, rewrite=census, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', timezone='UTC',
        fileEncoding='UTF-8', scope=SCOPE)
    summary = (f'{len(rows)} original social cases ({coverage["actions"]} actions, {coverage["serviceCalls"]} recorded calls, '
               f'{coverage["injectedFailures"]} injected failures)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Social fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary} under {FIXTURES}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
