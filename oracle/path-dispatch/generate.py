"""Verbatim original Pathfinder overloads, with observable external services."""
import argparse, hashlib, importlib.util, json, os, pathlib, random, re, subprocess, urllib.request
ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/path-dispatch-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
spec = importlib.util.spec_from_file_location('compiler', ROOT / 'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
read, run = c.read, c.run

def requests():
    cases = []
    for mode in ['entity', 'entityDefault', 'water', 'explicit', 'start', 'locationDefault', 'locationSize', 'locationFinder']:
        for kind in ['other', 'entity', 'npc', 'scenery', 'item', None]:
            for mover in ['entity', 'npc', None]:
                for supplier in [None, 7]:
                    cases.append(dict(mode=mode, targetKind=kind, moverKind=mover, npcSupplier=supplier, supplier=supplier, near=False))
    for kind in [-2147483648, -1, 0, 1, 2, 6, 7, 8, 9, 10, 11, 12, 22, 2147483647]:
        for rotation in [-2147483648, -33, -4, -1, 0, 1, 2, 3, 4, 5, 31, 32, 33, 2147483647]:
            for flag in [-2147483648, -1, 0, 1, 15, 16, 2147483647]:
                cases.append(dict(mode='start', targetKind='scenery', kind=kind, rotation=rotation, walkingFlag=flag, dimensions=[2, 3], near=True))
    for mode in ['entity', 'water', 'explicit', 'start']:
        for target in ['scenery', 'item', 'entity', None]:
            for key in ['start', 'end', 'finder', 'definition']:
                cases.append(dict(mode=mode, targetKind=target, **{key:None}))
    for mode in ['entity', 'water', 'explicit']:
        for target in ['item', 'entity']:
            for key in ['behavior', 'permitted']:
                for value in [False, True]:
                    cases.append(dict(mode=mode, targetKind=target, moverKind='npc', **{key:value}))
    for callback in ['npc', 'permitted', 'invoke']:
        cases.append(dict(mode='entity', moverKind='npc', targetKind='item', fail=callback))
        cases.append(dict(mode='entity', moverKind='npc', targetKind='item', mutateAt=callback, mutateMover=[3220,3221,2], mutateTarget=[3230,3231,3]))
    rng = random.Random(530)
    for _ in range(200):
        cases.append(dict(mode=rng.choice(['entity','water','explicit','start']), moverKind=rng.choice(['entity','npc']), targetKind=rng.choice(['entity','npc','scenery','item','other']), size=rng.randrange(-4,8), moverSize=rng.randrange(-4,8), dimensions=[rng.randrange(-4,8),rng.randrange(-4,8)], rotation=rng.randrange(-8,9), kind=rng.randrange(-1,24), walkingFlag=rng.randrange(-128,256), near=bool(rng.randrange(2)), permitted=bool(rng.randrange(2)), npcSupplier=rng.choice([None,2,5]), supplier=rng.choice([None,4,8]), finder=rng.choice([None,0,1,7])))
    return cases

def generate(check):
    lock = json.loads(read(ROOT / 'upstream.lock.json'))
    p = ROOT / 'reference/server'
    if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    run(['git','-C',p,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True, exist_ok=True)
    classes = RUN / 'classes'; classes.mkdir(exist_ok=True)
    jar = RUN / 'json-simple-1.1.1.jar'
    artifact = json.loads(read(ROOT / 'oracle/auth/json.lock.json'))
    if not jar.exists() or hashlib.sha256(jar.read_bytes()).hexdigest() != artifact['sha256']:
        with urllib.request.urlopen(artifact['url'], timeout=60) as r: data = r.read()
        if len(data) != artifact['size'] or hashlib.sha256(data).hexdigest() != artifact['sha256']: raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java = []
    def adapter(name, text):
        p = RUN / 'adapters' / name; p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding='utf-8'); java.append(p)
    source = read(SERVER / 'core/game/world/map/path/Pathfinder.java')
    methods = [c.method(source, m.group()) for m in re.finditer(r'public static Path (?:find|findWater)\([^)]*\)', source)]
    adapter('core/game/world/map/path/Pathfinder.java', 'package core.game.world.map.path;import core.game.node.Node;import core.game.node.entity.Entity;import core.game.node.entity.npc.NPC;import core.game.node.item.GroundItem;import core.game.node.scenery.Scenery;import core.game.world.map.*;public class Pathfinder{public static final Pathfinder SMART=new Pathfinder(0);public int id;public Pathfinder(int i){id=i;}public Path find(Location a,int s,Location b,int sx,int sy,int r,int t,int f,boolean n,ClipMaskSupplier c){return oracle.Support.invoke(id,a,s,b,sx,sy,r,t,f,n,c);}'+''.join(methods)+'}')
    adapter('core/game/world/map/path/Path.java','package core.game.world.map.path;public class Path{public Object value;}')
    adapter('core/game/world/map/path/ClipMaskSupplier.java','package core.game.world.map.path;public interface ClipMaskSupplier{int getClippingFlag(int z,int x,int y);}')
    # Location is an explicit metadata adapter. Packing follows the pinned hash;
    # native packed Location itself is covered by the primitive/movement corpora.
    read(SERVER / 'core/game/world/map/Location.kt')
    adapter('core/game/world/map/Location.java','package core.game.world.map;public class Location{int x,y,z;public Location(int x,int y,int z){this.x=(x&32767)>=16384?(x&32767)-32768:x&32767;this.y=(y&32767)>=16384?(y&32767)-32768:y&32767;this.z=(z%4+4)%4;}public int getX(){return x;}public int getY(){return y;}public int getZ(){return z;}}')
    adapter('core/game/node/Node.java','package core.game.node;import core.game.world.map.Location;public class Node{public Location location;public int size=1;public Location getLocation(){return location;}public int size(){return size;}}')
    adapter('core/game/node/entity/Entity.java','package core.game.node.entity;public class Entity extends core.game.node.Node{}')
    adapter('core/game/node/entity/npc/NPC.java','package core.game.node.entity.npc;public class NPC extends core.game.node.entity.Entity{public Behavior behavior=new Behavior();public static class Behavior{public core.game.world.map.path.ClipMaskSupplier getClippingSupplier(NPC n){return oracle.Support.npc(n);}}}')
    adapter('core/game/node/item/GroundItem.java','package core.game.node.item;public class GroundItem extends core.game.node.Node{}')
    adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;public class Scenery extends core.game.node.Node{public int type=10,rotation;public Definition definition=new Definition();public int getType(){return type;}public int getRotation(){return rotation;}public Definition getDefinition(){return definition;}public static class Definition{public int sizeX=1,sizeY=1,walkingFlag;public int getWalkingFlag(){return walkingFlag;}}}')
    adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static int getClippingFlag(int z,int x,int y){return 0;}public static int getWaterClipFlag(int z,int x,int y){return 0;}public static boolean isTeleportPermitted(Location l){return oracle.Support.permitted(l);}}')
    read(HERE / 'Support.java'); java.append(HERE / 'Support.java')
    run(['javac','--release','11','-encoding','UTF-8','-cp',jar,'-d',classes,*java])
    cases = requests(); file = RUN / 'input.jsonl'
    file.write_text(''.join(json.dumps(x)+'\n' for x in cases), encoding='utf-8')
    exe = c.JAVA_HOME/'bin'/('java.exe' if os.name=='nt' else 'java') if c.JAVA_HOME else 'java'
    with file.open('rb') as f: p = subprocess.run(list(map(str,[exe,'-cp',os.pathsep.join(map(str,[classes,jar])),'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=60)
    if p.returncode: raise RuntimeError(p.stderr.decode(errors='replace'))
    rows = [json.loads(line) for line in p.stdout.splitlines() if line]; assert len(rows)==len(cases)
    data = ''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n' for x in rows).encode()
    read(pathlib.Path(__file__)); read(ROOT / 'oracle/login/generate.py')
    provenance = dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Every verbatim Pathfinder Node/entity/location find overload, including water/explicit/NPC suppliers, destination geometry and Java signed flag rotation. Explicit metadata, behavior, teleport and recording finder services; callback mutation/failure order and location aliases observed. Actual search, Path.walk, queue/world composition and live movement are separate.')
    folder = ROOT / 'fixtures/path-dispatch'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance: raise RuntimeError('Path dispatch fixtures/provenance differ')
        print(f'{len(rows)} original path-dispatch scenarios reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True); (folder/'oracle.jsonl').write_bytes(data)
        (folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8')
        print(f'Generated {len(rows)} original path-dispatch scenarios.')

if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args()
    c.JAVA_HOME=a.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(a.check)
