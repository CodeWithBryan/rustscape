"""Original variable classes, ContentAPI algorithms and actual outgoing writers.

Java/Kotlin run only as an oracle. Runtime is native Rust. --check is read-only
with respect to committed sources and fixtures.
"""
import argparse, copy, hashlib, importlib.util, itertools, json, os, pathlib, shutil, subprocess, urllib.request
R = pathlib.Path(__file__).resolve().parents[2]
H = pathlib.Path(__file__).parent
O = R / 'runs/variables-oracle'
S = R / 'reference/server/Server/src/main'
spec = importlib.util.spec_from_file_location('compiler', R / 'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

def requests(custom):
    rows = []
    def add(actions, **kw):
        row = dict(values=[{}, {}], saved=[{}, {}], files={}, hooks=[], actions=actions)
        row.update(kw)
        rows.append(copy.deepcopy(row))
    for id, value in itertools.product([-2147483648, -1, 0, 127, 128, 65535, 65536, 2147483647], [-2147483648, -129, -128, -1, 0, 127, 128, 255, 256, 2147483647]):
        add([dict(op='set', id=id, value=value), dict(op='get', id=id), dict(op='set', id=id, value=value, save=True), dict(op='set', id=id, value=1), dict(op='unsave', id=id), dict(op='varc', id=id, value=value)])
    for op, nil in itertools.product(['get', 'set', 'saveFlag', 'unsave', 'varc', 'reinit', 'getBit', 'setBit'], ['dispatch', 'session', 'pair', 'details', 'detailSession', 'interfaces']):
        add([dict(op='rawValue', id=2, value=5), dict(op=op, id=2, value=256, save=True)], null=nil)
    for op in ['get', 'set', 'saveFlag', 'unsave', 'varc', 'reinit', 'getBit', 'setBit']:
        add([dict(op=op, player=-1, id=2, value=8)])
        for field in ['valuesNull', 'savedNull']:
            add([dict(op=field), dict(op=op, id=2, value=8, save=True), dict(op='capture')])
    for artificial, null in itertools.product([False, True], ['', 'session', 'pair', 'details', 'detailSession']):
        add([dict(op='set', id=1, value=1), dict(op='varc', id=2, value=-1)], artificial=artificial, null=null)
    for count, value in itertools.product([-2147483648, -1, 0, 65535, 2147483647], [-2147483648, -1, 255, 256, 2147483647]):
        add([dict(op='varc', id=-65537, value=value), dict(op='varc', id=65535, value=value)], counter=count)
    for start, end in [(0, 0), (0, 1), (2, 4), (31, 31), (0, 31), (0, 32), (31, 32), (-2, 2), (33, 67), (4, 2), (-33, -1)]:
        for value in [-2147483648, -1, 0, 1, 2147483647]:
            add([dict(op='create', id=7, varp=2, start=start, end=end), dict(op='lookup', id=7), dict(op='mask'), dict(op='getBit', id=7), dict(op='setBit', id=7, value=value, save=True), dict(op='getDef'), dict(op='defValue')], values=[{'2': -1073741777}, {}])
    for id, raw in itertools.product([-2147483648, -1, 0, 1023, 1024, 2147483647], [None, '', '00', '010002000300', 'ff0100021f2000', '0100020003010005040600', '01', '0100', '010002', '01000200', '0100020003', 'ff']):
        add([dict(op='lookup', id=id), dict(op='lookup', id=id, alias='b', kind=1), dict(op='lookup', id=id, alias='c', kind=2), dict(op='lookup', id=id, alias='d', kind=3)], files={str(id): raw})
    add([dict(op='lookup', id=7), dict(op='create', id=7, varp=2, start=2, end=4), dict(op='getDef'), dict(op='defValue'), dict(op='lookup', id=7, alias='b'), dict(op='mapAlias', id=8), dict(op='lookup', id=8, alias='c'), dict(op='mapNull', id=7), dict(op='lookup', id=7, alias='d')], values=[{'0': 1, '2': 28}, {}])
    for hook in [dict(when='file', action='create', id=7, varp=5), dict(when='file', action='fail')]:
        add([dict(op='lookup', id=7), dict(op='lookup', id=7)], files={'7': '010002000100'}, hooks=[hook])
    for n in [0, 1, 2, 12, 13, 40, 65]:
        keys = [i * 64 for i in range(n)]
        actions = [dict(op='rawValue', id=k, value=k+1) for k in keys]
        add(actions + [dict(op='capture'), dict(op='reinit'), dict(op='clear')] + [dict(op='rawValue', id=k, value=k+1) for k in reversed(keys[:9])] + [dict(op='reinit')])
    for when, action, at in itertools.product(['dispatch', 'packetVarp', 'session', 'pair', 'cipher', 'artificial', 'details', 'detailSession', 'write'], ['fail', 'put', 'clear', 'replace', 'savedNull', 'savedReplace'], [1, 3]):
        add([dict(op='capture'), dict(op='reinit'), dict(op='get', id=1)], values=[{'1': 11, '17': 22, '2': 33}, {}], saved=[{'1': True, '17': False, '2': None}, {}], hooks=[dict(when=when, action=action, at=at, id=9, value=77)])
    for values in [{'1': None, '2': 7}, {'1': 3, '2': None}]:
        add([dict(op='reinit')], values=[values, {}])
    good = [dict(index=3, bitArray=[dict(offset=0, value=3), dict(offset=31, value=1), dict(offset=-1, value=-1)]), dict(index='-1', bitArray=[])]
    malformed = [None, {}, [None], [1], [dict(index=None, bitArray=[])], [dict(index=True, bitArray=[])], [dict(index='2147483648', bitArray=[])], [dict(index=1)], [dict(index=1, bitArray={})], [dict(index=1, bitArray=[None])], [dict(index=1, bitArray=[dict(offset=0, value=None)])], [dict(index=1, bitArray=[dict(offset=0.5, value=1)])]]
    for data in [good, *malformed, *[good + x for x in malformed if isinstance(x, list)]]:
        add([dict(op='parse', data=data), dict(op='capture'), dict(op='reinit')])
    for owner in [0, 1]:
        add([dict(op='parse', data=good), dict(op='noopSave', data={'keep': [1, None, 'x']}), dict(op='noopSave', data=None), dict(op='constructorNull')], managerOwner=owner)
    for field in ['valuesNull', 'savedNull']:
        add([dict(op=field), dict(op='parse', data=good)])
    custom_rows = json.loads(custom)
    texts = [custom, '[]', 'null', '{}', '[null]', '[1]', '[', json.dumps(custom_rows + [custom_rows[0]]), json.dumps(custom_rows + [dict(varpId=1)]), json.dumps([dict(varpId='-1', varbitId='-2', startBit='31', endBit='32')]), json.dumps([dict(varpId=True, varbitId=2, startBit=0, endBit=1)]), json.dumps([dict(varpId=1, varbitId=2, startBit=0.0, endBit=1)])]
    for text in texts:
        add([dict(op='reader'), dict(op='load', text=custom), dict(op='reader'), dict(op='load', text=text), dict(op='reader'), dict(op='load', missing=True), dict(op='reader')])
    add([dict(op='load', text=custom)], hooks=[dict(when='log', action='fail')])
    return rows

def generate(check):
    lock = json.loads(c.read(R / 'upstream.lock.json'))
    assert c.run(['git', '-C', R / 'reference/server', 'rev-parse', 'HEAD']).stdout.decode().strip() == lock['server']['commit']
    c.run(['git', '-C', R / 'reference/server', 'diff', '--quiet', 'HEAD', '--'])
    O.mkdir(parents=True, exist_ok=True)
    classes = O / 'classes'; classes.mkdir(exist_ok=True)
    java, kt = [], []
    jars = c.artifacts()
    artifact = json.loads(c.read(R / 'oracle/auth/json.lock.json'))
    jar = O / 'json-simple-1.1.1.jar'
    if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != artifact['sha256']:
        cached = R / 'runs/inventory-oracle/json-simple-1.1.1.jar'
        if cached.is_file() and hashlib.sha256(cached.read_bytes()).hexdigest() == artifact['sha256']:
            shutil.copyfile(cached, jar)
        else:
            with urllib.request.urlopen(artifact['url'], timeout=60) as response: data = response.read()
            assert len(data) == artifact['size'] and hashlib.sha256(data).hexdigest() == artifact['sha256']
            jar.write_bytes(data)
    std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    cp = os.pathsep.join(map(str, [classes, std, jar]))
    def adapter(name, text):
        p = O / 'adapters' / name; p.parent.mkdir(parents=True, exist_ok=True); p.write_text(text, encoding='utf-8', newline='\n'); (java if name.endswith('.java') else kt).append(p)
    for path in ['core/cache/def/impl/VarbitDefinition.java', 'core/game/node/entity/player/VarpManager.kt', 'core/game/node/entity/player/Varbit.kt', 'core/game/system/config/CustomVarbitParser.kt', 'core/net/packet/IoBuffer.java', 'core/net/packet/PacketHeader.java', 'core/net/packet/Context.java', 'core/net/packet/OutgoingPacket.java', 'core/net/packet/out/Config.java', 'core/net/packet/out/VarcUpdate.java', 'core/net/packet/context/ConfigContext.java', 'core/net/packet/context/VarcUpdateContext.java']:
        p = S / path; c.read(p); (java if path.endswith('.java') else kt).append(p)
    adapter('core/game/node/entity/player/Player.java', c.read(H / 'Player.java'))
    adapter('oracle/VarsOracle.java', c.read(H / 'VarsOracle.java'))
    api = c.read(S / 'core/api/ContentAPI.kt')
    signatures = ['fun getVarp (', 'fun getVarbit (player: Player, def:', 'fun getVarbit (player: Player, varbitId:', 'fun setVarp (', 'fun saveVarp (', 'fun unsaveVarp (', 'fun setVarbit (player: Player, def:', 'fun setVarbit (player: Player, varbitId:', 'fun setVarc (', 'fun reinitVarps (']
    methods = '\n'.join(('@JvmOverloads\n' if signature.startswith(('fun setVarp (', 'fun setVarbit (')) else '') + c.method(api, signature) for signature in signatures)
    adapter('core/api/ContentAPI.kt', '''package core.api
import core.game.node.entity.player.Player
import core.cache.def.impl.VarbitDefinition
import core.tools.Log
class ContentAPI
fun log(owner:Any,level:Log,message:String){oracle.VarsOracle.hit("log",message)}
fun logWithStack(owner:Any,level:Log,message:String){oracle.VarsOracle.hit("log",message)}
''' + methods)
    dispatch = c.read(S / 'core/game/node/entity/player/link/PacketDispatch.java')
    adapter('core/game/node/entity/player/link/PacketDispatch.java', 'package core.game.node.entity.player.link;import core.game.node.entity.player.Player;import core.net.packet.*;import core.net.packet.context.*;import core.net.packet.out.*;public class PacketDispatch{private final Player player;public PacketDispatch(Player p){player=p;}' + c.method(dispatch, 'public void sendVarp (') + c.method(dispatch, 'public void sendVarcUpdate(') + '}')
    adapter('core/net/packet/PacketRepository.java', 'package core.net.packet;public class PacketRepository{public static void send(Class<?>type,Object context){oracle.VarsOracle.packet(type,context);}}')
    adapter('core/cache/Cache.java', 'package core.cache;public class Cache{public static Index[] getIndexes(){Index[] a=new Index[23];a[22]=new Index();return a;}public static class Index{public byte[] getFileData(int group,int file){return oracle.VarsOracle.file(group,file);}}}')
    adapter('core/ServerConstants.java', 'package core;public class ServerConstants{public static String CONFIG_PATH;}')
    adapter('core/cache/crypto/ISAACCipher.java', 'package core.cache.crypto;public class ISAACCipher{public int getNextValue(){return 0;}}')
    adapter('core/cache/misc/buffer/ByteBufferUtils.java', 'package core.cache.misc.buffer;import java.nio.ByteBuffer;public class ByteBufferUtils{public static String getString(ByteBuffer b){throw new AssertionError();}public static int packGJString2(int a,byte[] b,String c){throw new AssertionError();}}')
    adapter('core/tools/Log.java', 'package core.tools;public enum Log{FINE,ERR}')
    adapter('core/tools/SystemLogger.java', 'package core.tools;public class SystemLogger{}')
    adapter('content/global/skill/farming/FarmingPatch.java', 'package content.global.skill.farming;public class FarmingPatch{}')
    adapter('core/game/event/VarbitUpdateEvent.java', 'package core.game.event;public class VarbitUpdateEvent{}')
    c.run(['java', '-Xmx512m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-jvm-target', '11', '-classpath', cp, '-d', classes, *kt, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    custom_bytes = c.run(['git', '-C', R / 'reference/server', 'show', 'HEAD:Server/data/configs/varbit_definitions.json']).stdout
    custom = custom_bytes.decode('utf-8')
    cases = requests(custom)
    data = ''.join(json.dumps(row) + '\n' for row in cases).encode()
    (O / 'input.jsonl').write_bytes(data)
    exe = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    p = subprocess.run(list(map(str, [exe, '-Xmx512m', '-Dfile.encoding=UTF-8', '-cp', cp, 'oracle.VarsOracle', O / 'io'])), cwd=R, input=data, capture_output=True, timeout=120)
    (O / 'stderr.log').write_bytes(p.stderr)
    (O / 'output.jsonl').write_bytes(p.stdout)
    assert p.returncode == 0, p.stderr.decode(errors='replace')
    rows = [json.loads(line) for line in p.stdout.splitlines() if line]
    assert len(rows) == len(cases)
    for inp, out in zip(cases, rows): assert {k:v for k,v in out.items() if k != 'expected'} == inp
    fixture = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(fixture).hexdigest(), sourceHashAlgorithm='sha256, CR removed', customDefinitions=dict(gitBlob=c.run(['git', '-C', R / 'reference/server', 'rev-parse', 'HEAD:Server/data/configs/varbit_definitions.json']).stdout.decode().strip(), sha256=hashlib.sha256(custom_bytes).hexdigest()), scope='Complete unchanged VarbitDefinition, VarpManager, Varbit and CustomVarbitParser; selected unchanged ContentAPI variable methods and PacketDispatch methods; unchanged Config/VarcUpdate and IoBuffer. Real plain HashMap public player fields, original UTF8 FileReader/JSON-simple loader in owned temporary files. Player packet/session/artificial/interface-counter and Cache index22 are explicit recording adapters, ISAAC output zero. Snapshots sort exposed maps only for diagnostics; runtime reinit compares original integer HashMap iteration including retained clear capacity, collision/tree bins, callbacks and failure prefixes. Native bounded UTF8 text parser does not establish complete FileReader cursor/alternate charset/read-ahead behavior, arbitrary exposed mapping iteration, whole Player/profile/startup/AIPlayer/session or playable banking. Original getMask nontermination for endBit MAX and spans over one million are excluded; native rejects them.')
    folder = R / 'fixtures/variables'; folder.mkdir(exist_ok=True)
    if check:
        assert (folder / 'oracle.jsonl').read_bytes() == fixture
        assert json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) == provenance
        assert (folder / 'varbit_definitions.json').read_bytes() == custom_bytes
    else:
        (folder / 'oracle.jsonl').write_bytes(fixture)
        (folder / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8')
        (folder / 'varbit_definitions.json').write_bytes(custom_bytes)
    print(len(rows), 'original variable scenarios reproduced' if check else 'original variable scenarios generated')

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--check', action='store_true'); p.add_argument('--java-home', type=pathlib.Path); a = p.parse_args()
    c.JAVA_HOME = a.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(a.check)
