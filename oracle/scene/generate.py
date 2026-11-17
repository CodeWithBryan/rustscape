"""Joined original region/terrain/scenery/door/collision/update/restoration traces."""
import argparse, hashlib, importlib.util, json, os, pathlib, subprocess, urllib.request
ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/scene-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
spec = importlib.util.spec_from_file_location('compiler', ROOT/'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec); spec.loader.exec_module(c)
read, run = c.read, c.run

def smart(n):
    return bytes([n]) if n < 128 else (n + 32768).to_bytes(2, 'big')

def requests():
    out = []
    for x, y in [(7,17), (0,0), (63,63), (8,7)]:
        for rotation in range(4):
            for build in [False, True]:
                l = [3200+x, 3200+y, 0]
                dx, dy = [(-1,0),(0,1),(1,0),(0,-1)][rotation]
                opened = [l[0]+dx, l[1]+dy, 0]
                land = (smart(12349)+smart((x<<6|y)+1)+bytes([rotation,0,0])).hex()
                actions = [dict(op='load', id=12850, build=build),
                           dict(op='lookup', name='closed', location=l),
                           dict(op='door', node='closed'),
                           dict(op='lookup', name='opened', location=opened),
                           dict(op='tick', count=499), dict(op='tick', count=1),
                           dict(op='lookup', name='restored', location=l),
                           dict(op='door', node='restored'),
                           dict(op='lookup', name='again', location=opened),
                           dict(op='door', node='again'), dict(op='tick', count=500)]
                out.append(dict(land=land, actions=actions))
    # Removing a temporary door before it expires, deferred client-only swaps,
    # expiry and respawn use the same real parsers, builder and pulse bodies.
    for mode in ['remove', 'respawn', 'expire', 'client']:
        out.append(dict(land=(smart(12349)+smart((7<<6|17)+1)+bytes([1,0,0])).hex(), actions=[
            dict(op='load', id=12850, build=False),
            dict(op='lookup', name='closed', location=[3207,3217,0]),
            dict(op='door', node='closed'),
            dict(op='lookup', name='opened', location=[3207,3218,0]),
            dict(op=mode, node='opened', ticks=3),
            dict(op='tick', count=2), dict(op='tick', count=1), dict(op='tick', count=497)]))
    out.append(dict(land='00000000', missing=True, actions=[dict(op='load', id=12850, build=False)]))
    return out

def generate(check, audit=None, run_directory=None):
    global RUN
    RUN=(run_directory or ROOT/'runs/scene-oracle').resolve()
    lock = json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        p = ROOT/'reference'/part
        if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        run(['git','-C',p,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True, exist_ok=True)
    classes = RUN/'classes'; classes.mkdir(exist_ok=True)
    jars = c.artifacts(); std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    jar = RUN/'json-simple-1.1.1.jar'; artifact = json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists() or hashlib.sha256(jar.read_bytes()).hexdigest() != artifact['sha256']:
        with urllib.request.urlopen(artifact['url'], timeout=60) as r: data=r.read()
        if len(data) != artifact['size'] or hashlib.sha256(data).hexdigest() != artifact['sha256']:
            raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java=[]; kt=[]; adapters={}
    def adapter(name, text): adapters[name]=text
    paths=['core/cache/def/impl/SceneryDefinition.java','core/cache/def/Definition.java','core/cache/misc/buffer/ByteBufferUtils.kt',
           'core/game/node/scenery/Scenery.java','core/game/node/scenery/Constructed.java','core/game/node/scenery/SceneryBuilder.java',
           'core/game/system/task/Pulse.java','core/game/global/action/DoorActionHandler.java','core/game/system/config/DoorConfigLoader.kt',
           'core/game/world/map/build/MapscapeParser.java','core/game/world/map/build/LandscapeParser.java','core/game/world/map/build/ChunkFlags.java',
           'core/game/world/update/flag/UpdateFlag.java','core/game/world/update/flag/chunk/ObjectUpdateFlag.java',
           'core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java','core/net/packet/OutgoingPacket.java','core/net/packet/Context.java',
           'core/net/packet/out/ConstructScenery.java','core/net/packet/out/ClearScenery.java','core/net/packet/out/UpdateAreaPosition.java',
           'core/net/packet/context/BuildSceneryContext.java','core/net/packet/context/AreaPositionContext.java',
           'core/net/event/GameWriteEvent.java','core/net/IoWriteEvent.java']
    for name in paths:
        p=SERVER/name; read(p); (kt if p.suffix=='.kt' else java).append(p)
    skip={'SceneryBuilder.java','SceneryDefinition.java','RegionManager.java','Player.java','ContentAPIKt.java','ByteBufferUtils.java'}
    for p in sorted((ROOT/'oracle/doors/adapters').rglob('*')):
        if p.is_file() and p.name not in skip: adapter(p.relative_to(ROOT/'oracle/doors/adapters').as_posix(),read(p))
    for p in sorted((ROOT/'oracle/scenery/adapters').rglob('*')):
        if p.is_file() and p.name not in {'RegionChunk.java','RegionManager.java','LandscapeParser.java','Player.java','ContentAPIKt.java','ByteBufferUtils.java'}:
            adapter(p.relative_to(ROOT/'oracle/scenery/adapters').as_posix(),read(p))
    adapter('core/game/world/GameWorld.java',read(ROOT/'oracle/doors/adapters/core/game/world/GameWorld.java').replace('public class GameWorld{','public class GameWorld{public static void prompt(boolean b){throw new UnsupportedOperationException();}'))
    adapter('core/cache/def/impl/VarbitDefinition.java','package core.cache.def.impl;public class VarbitDefinition{public static VarbitDefinition forObjectID(int id){throw new UnsupportedOperationException();}public int getValue(core.game.node.entity.player.Player p){throw new UnsupportedOperationException();}}')
    player=read(ROOT/'oracle/doors/adapters/core/game/node/entity/player/Player.java')
    player=player.replace('public class Player extends Entity{','public class Player extends Entity{public core.net.IoSession getSession(){return oracle.Support.session;}public Player getPlayerFlags(){return this;}public core.game.world.map.Location getLastSceneGraph(){return new core.game.world.map.Location(3207,3216,0);}')
    adapter('core/game/node/entity/player/Player.java',player)
    api=read(ROOT/'oracle/doors/adapters/core/api/ContentAPIKt.java').replace('public class ContentAPIKt{','public class ContentAPIKt{public static int getVarp(Player p,int id){throw new UnsupportedOperationException();}')
    adapter('core/api/ContentAPIKt.java',api)
    adapter('core/tools/StringUtils.java','package core.tools;public class StringUtils{'+c.method(read(SERVER/'core/tools/StringUtils.java'),'public static boolean isPlusN(')+'}')
    adapter('core/game/interaction/OptionHandler.java','package core.game.interaction;public class OptionHandler{}')
    adapter('core/cache/Cache.java','''package core.cache;public class Cache{public static int getObjectDefinitionsSize(){return oracle.Support.definitionCount();}public static Index[]getIndexes(){Index[]a=new Index[17];a[5]=new Index();a[16]=new Index();return a;}public static class Index{public byte[]getFileData(int g,int f){return oracle.Support.definitionFile(g*256+f);}public byte[]getFileData(int g,int f,int[]k){return oracle.Support.landBytes(g);}public int getArchiveId(String s){return oracle.Support.archive(s);}public Index getCacheFile(){return this;}public byte[]getContainerUnpackedData(int g){return oracle.Support.mapBytes(g);}}}''')
    adapter('core/game/system/config/XteaParser.java','package core.game.system.config;public class XteaParser{public static XteaParser Companion=new XteaParser();public int[]getRegionXTEA(int id){return new int[4];}}')
    adapter('core/game/world/map/build/DynamicRegion.java','package core.game.world.map.build;public class DynamicRegion extends core.game.world.map.Region{public DynamicRegion(){super(0,0);}public int getRegionId(){return 0;}}')
    direction=read(SERVER/'core/game/world/map/Direction.java')
    adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST(7),NORTH(0),NORTH_EAST(4),WEST(3),EAST(1),SOUTH_WEST(6),SOUTH(2),SOUTH_EAST(5);public final int value;Direction(int v){value=v;}public int toInteger(){return value;}'+c.method(direction,'public static Direction get(int rotation)')+c.method(direction,'public static Direction getDirection(int diffX, int diffY)')+'}')
    location=read(SERVER/'core/game/world/map/Location.kt')
    adapter('core/game/world/map/Location.kt','''package core.game.world.map
import kotlin.math.sqrt
class Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val regionId:Int get()=((x shr 6) shl 8) or(y shr 6);val chunkX:Int get()=(x and 63) shr 3;val chunkY:Int get()=(y and 63) shr 3;val chunkOffsetX:Int get()=x and 7;val chunkOffsetY:Int get()=y and 7;val chunkBase:Location get()=Location(x and -8,y and -8,z);val regionX:Int get()=x shr 3;val regionY:Int get()=y shr 3;fun getSceneX(l:Location):Int=x-((l.regionX-6) shl 3);fun getSceneY(l:Location):Int=y-((l.regionY-6) shl 3);fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);@JvmOverloads fun withinDistance(other:Location,dist:Int=15)=other.z==z&&getDistance(this,other)<=dist;'''+c.method(location,'override fun equals(other: Any?)')+'\noverride fun hashCode():Int=hash.hash;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z);'+c.method(location,'fun getDistance(first: Location, second: Location)')+'}\n@JvmInline '+c.method(location,'value class LocationHash(')+'}')
    region=read(SERVER/'core/game/world/map/Region.java')
    signatures=['public Region(int x, int y)','public Location getBaseLocation()','public RegionChunk[][][] getChunks()','public static void load(Region r)','public static void load(Region r, boolean build)','public int getId()','public int getX()','public int getY()','public int getObjectCount()','public void setObjectCount(','public boolean isHasFlags()','public boolean isLoaded()','public void setLoaded(','public boolean isBuild()','public void setBuild(']
    adapter('core/game/world/map/Region.java','''package core.game.world.map;import java.util.*;import java.nio.*;import core.cache.Cache;import core.game.node.scenery.*;import core.game.system.task.Pulse;import core.game.system.config.XteaParser;import core.game.world.map.build.*;public class Region{public static final int SIZE=64,CHUNKS_SIZE=8,PLANES=4;private final int x,y;protected RegionChunk[][][]chunks=new RegionChunk[8][8][4];private final Pulse activityPulse;private boolean loaded,build,hasFlags;private int objectCount;private final ArrayList<Scenery>addSceneries=new ArrayList<>(),removeSceneries=new ArrayList<>();public boolean flagInactive(){throw new UnsupportedOperationException();}'''+''.join(c.method(region,s)for s in signatures)+'}')
    chunk=read(SERVER/'core/game/world/map/RegionChunk.java')
    signatures=['public RegionChunk(Location base, int rotation)','public void addStatDyn(','public int add(Scenery object)','public int getIndex(','public Scenery[][][] getObjects()','public Scenery[][][] getStatObjects()','public Location getCurrentBase()','public ChunkFlags getFlags()','public ChunkFlags getProjectileFlags()','public Region getRegion()']
    adapter('core/game/world/map/RegionChunk.java','''package core.game.world.map;import core.game.node.scenery.*;import core.game.world.map.build.ChunkFlags;public class RegionChunk{public static final int SIZE=8,ARRAY_SIZE=4;protected Location base,currentBase;protected int rotation;protected Scenery[][][]statObjects,objects;private ChunkFlags flags,projectileFlags;public void flag(core.game.world.update.flag.UpdateFlag<?>f){oracle.Support.flag(this,(core.game.world.update.flag.chunk.ObjectUpdateFlag)f);}'''+''.join(c.method(chunk,s)for s in signatures)+'}')
    adapter('core/game/world/map/RegionManager.java','''package core.game.world.map;import core.game.node.scenery.Scenery;public class RegionManager{public static Region forId(int id){return oracle.Support.region(id);}public static RegionChunk getRegionChunk(Location l){return forId(l.getRegionId()).getChunks()[l.getChunkX()][l.getChunkY()][l.getZ()];}public static Integer[]getFlags(int k,boolean p){return oracle.Support.flags(k,p);}public static void addClippingFlag(int z,int x,int y,boolean p,int m){oracle.Support.border(new Location(x,y,z),p,m,true);}public static void removeClippingFlag(int z,int x,int y,boolean p,int m){oracle.Support.border(new Location(x,y,z),p,m,false);}public static Scenery getObject(int x,int y,int z,int id,int shape){Region r=forId((x>>6)<<8|(y>>6));Region.load(r);RegionChunk c=getRegionChunk(new Location(x,y,z));int i=c.getIndex(x&7,y&7,id,shape);if(i<0)return null;Scenery o=c.getObjects()[x&7][y&7][i];return o!=null&&!o.isRenderable()?null:o;}public static Scenery getObject(Location l,int shape){return getObject(l.getX(),l.getY(),l.getZ(),-1,shape);}}''')
    for name,text in adapters.items():
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt' else java).append(p)
    read(HERE/'Support.java');java.append(HERE/'Support.java')
    cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java])
    run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests()
    if audit: cases.append(json.loads(audit.read_text(encoding='utf-8')))
    file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8')
    samples=ROOT/'oracle/scenery-definitions/cache-samples.json';configs=ROOT/'fixtures/doors/door_configs.json';read(samples);read(configs)
    exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx4g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support',samples,configs])),cwd=ROOT,stdin=f,capture_output=True,timeout=600 if audit else 180)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    if p.stderr:raise RuntimeError('Unexpected original exception: '+p.stderr.decode(errors='replace'))
    out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
    if audit:
        (RUN/'audit-output.json').write_text(json.dumps(out[-1],sort_keys=True,separators=(',',':'))+'\n',encoding='utf-8')
        out=out[:-1]
    data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Joined unchanged SceneryDefinition/Scenery/Constructed/SceneryBuilder/DoorActionHandler/DoorConfigLoader/MapscapeParser/LandscapeParser/ChunkFlags/Pulse/ObjectUpdateFlag/IoBuffer/outgoing writers; verbatim Region.load and chunk storage methods. Synthetic cache map/land inputs, pinned raw door definitions and full door configurations. Explicit single-owner region routing, fixed-prefix scheduler, player/path/audio/ground services and chunk flag delivery adapters. Compares real stored scenery, clipping arrays, retained references, ordered wire frames and 500-tick restoration. No real-cache landscape, live socket/world entry, entity movement, authoritative sound delivery, dynamic regions or capacity claim.')
    folder=ROOT/'fixtures/scene'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Scene fixtures/provenance differ')
        print(f'{len(out)} original joined scene scenarios reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original joined scene scenarios.')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);p.add_argument('--audit-file',type=pathlib.Path);p.add_argument('--run-dir',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check,a.audit_file,a.run_dir)
