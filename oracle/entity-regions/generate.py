"""Original RegionManager spatial queries/move and RegionChunk entity lists."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/entity-regions-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 cases=[];rng=random.Random(53015)
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions,nodes=None,**kw):cases.append(dict(actions=actions,nodes=nodes or[dict(id=1,kind='player',location=[3207,3216,0]),dict(id=2,kind='npc',location=[3208,3216,0])],**kw))
 for kind in ['player','npc','other']:
  for src in [None,[3207,3216,0],[3208,3216,0],[3264,3216,0],[3207,3216,1]]:
   for dst in [[3207,3216,0],[3208,3216,0],[3264,3216,0],[3207,3216,1]]:add([a('move',node=1,src=src,dst=dst)],nodes=[dict(id=1,kind=kind,location=dst)])
 for kind in ['players','npcs']:
  for op in ['add','remove']:
   for node in [None,1 if kind=='players'else 2]:add([a(op,list=kind,chunk=[3207,3216,0],node=node),a(op,list=kind,chunk=[3207,3216,0],node=node)])
 for distance in [-16,-9,-8,-7,-1,0,1,7,8,15,16,31,64]:
  for select in ['all','players','npcs']:
   nodes=[dict(id=i+1,kind=['player','npc','other'][i%3],location=[3207+rng.randint(-18,18),3216+rng.randint(-18,18),i%4])for i in range(24)]
   actions=[a('add',list=['players','npcs'][i%2],node=n['id'],chunk=n['location'],raw=True)for i,n in enumerate(nodes)]+[a('query',location=[3207,3216,z],distance=distance,select=select)for z in range(4)];add(actions,nodes=nodes)
 for phase in ['chunk','region','asPlayer','active','inactive','name','clock','footprint','zone','location']:
  for at in [1,2,3]:add([a('add',list='players',chunk=[3207,3216,0],node=1),a('move',node=1,src=[3207,3216,0],dst=[3264,3216,0]),a('query',location=[3207,3216,0],distance=15)],failures={phase:at})
 for phase in ['chunk','region','asPlayer','active','inactive','name','clock','footprint','zone','location']:
  for action in [a('set',node=1,location=None,footprint=[5,6,2],name='changed'),a('rebase',chunk=[3207,3216,0],base=[3264,3264,1]),a('clear',chunk=[3207,3216,0],list='players'),a('add',chunk=[3207,3216,0],list='players',node=1),a('query',location=[3207,3216,0],distance=0)]:
   add([a('add',list='players',chunk=[3207,3216,0],node=1),a('move',node=1,src=[3207,3216,0],dst=[3264,3216,0]),a('query',location=[3207,3216,0],distance=15)],hooks=[dict(event=phase,actions=[action])])
 for t in [None,-9223372036854775808,-1,0,1,600000,600001,9223372036854775807]:
  for clock in [[0],[600000],[600001],[9223372036854775807,-9223372036854775808]]:add([a('tolerance',region=[3207,3216,0],name='p1',value=t),a('tolerated',region=[3207,3216,0],node=1)],clock=clock)
 add([a('tolerated',region=[3207,3216,0],node=1)],clock=[1000000,0])
 # Duplicate order, out-of-date memberships and live location reads are observable.
 add([a('add',list='npcs',chunk=[3208,3216,0],node=2),a('add',list='npcs',chunk=[3208,3216,0],node=2),a('query',location=[3207,3216,0],distance=15),a('remove',list='npcs',chunk=[3208,3216,0],node=2),a('query',location=[3207,3216,0],distance=15)])
 for action in [a('set',node=2,location=[3208,3300,0]),a('set',node=2,location=None),a('clear',chunk=[3208,3216,0],list='npcs')]:add([a('add',list='npcs',chunk=[3208,3216,0],node=2),a('query',location=[3207,3216,0],distance=15)],hooks=[dict(event='location',actions=[action])])
 for base in [[0,0,0],[16383,16383,3],[32768,-1,-1]]:add([a('move',node=2,src=None,dst=base),a('set',node=2,location=base),a('query',location=base,distance=15)])
 return cases
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 manager=read(SERVER/'core/game/world/map/RegionManager.kt');methods=c.method(manager,'fun move(entity: Entity, src: Location?, dst: Location)')+'\n'+c.method(manager,'private inline fun<reified T: Entity> getLocalEntitiesOfType(')
 adapter('core/game/world/map/RegionManager.kt','package core.game.world.map\nimport core.game.node.entity.*\nimport core.game.node.entity.player.Player\nimport core.game.node.entity.npc.NPC\nimport core.game.world.map.zone.ZoneBorders\nimport core.tools.Log\nimport core.api.log\nobject RegionManager{@JvmStatic '+methods.replace('System.currentTimeMillis()','oracle.Support.now()')+'\n@JvmStatic fun all(l:Location,d:Int)=getLocalEntitiesOfType<Entity>(l,d)\n@JvmStatic fun players(l:Location,d:Int)=getLocalEntitiesOfType<Player>(l,d)\n@JvmStatic fun npcs(l:Location,d:Int)=getLocalEntitiesOfType<NPC>(l,d)\n}')
 chunk=read(SERVER/'core/game/world/map/RegionChunk.java');methods=''.join(c.method(chunk,s)for s in ['public void add(NPC npc)','public void remove(NPC npc)','public void addPlayer(Player player)','public void removePlayer(Player player)'])
 adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;import java.util.*;import core.game.node.entity.npc.NPC;import core.game.node.entity.player.Player;public class RegionChunk{public Location currentBase;public List<Player>players=new ArrayList<>();public List<NPC>npcs=new ArrayList<>();public RegionChunk(Location l){currentBase=l;}public List<Player>getPlayers(){return players;}public List<NPC>getNpcs(){return npcs;}'+methods+'}')
 region=read(SERVER/'core/game/world/map/Region.java');adapter('core/game/world/map/Region.java','package core.game.world.map;import java.util.*;import java.util.concurrent.TimeUnit;import core.game.node.entity.player.Player;public class Region{public int id;public HashMap<String,Long>tolerances=new HashMap<>();public Region(int id){this.id=id;}public HashMap<String,Long>getTolerances(){return tolerances;}public void flagActive(){oracle.Support.emit("active",id);}public void checkInactive(){oracle.Support.emit("inactive",id);}'+c.method(region,'public boolean isTolerated(Player player)').replace('System.currentTimeMillis()','oracle.Support.now()')+'}')
 location=read(SERVER/'core/game/world/map/Location.kt');adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val absChunkX:Int get()=x shr 3;val absChunkY:Int get()=y shr 3;val chunk:RegionChunk get()=oracle.Support.chunk(this);val region:Region get()=oracle.Support.region(this);@JvmInline '+c.method(location,'value class LocationHash(')+'}')
 borders=read(SERVER/'core/game/world/map/zone/ZoneBorders.java');adapter('core/game/world/map/zone/ZoneBorders.java','package core.game.world.map.zone;import java.util.*;public class ZoneBorders{int southWestX,southWestY,northEastX,northEastY,plane;boolean zeroPlaneCheck;List<ZoneBorders>exceptions;'+''.join(c.method(borders,s)for s in ['public ZoneBorders(int x1, int y1, int x2, int y2)','public boolean insideBorder(int x, int y)','public boolean insideBorder(int x, int y, int z)'])+'}')
 adapter('core/game/node/entity/Entity.java','package core.game.node.entity;import core.game.world.map.Location;import core.game.node.entity.player.Player;public class Entity{public int slot;private Location location;private String name;public Location rawLocation(){return location;}public void rawLocation(Location l){location=l;}public String rawName(){return name;}public void rawName(String n){name=n;}public Queue walkingQueue;public Zone zoneMonitor;public Entity(int s){slot=s;walkingQueue=new Queue(s);zoneMonitor=new Zone(s);}public Location getLocation(){Location l=location;oracle.Support.emit("location",slot);return l;}public Player asPlayer(){oracle.Support.emit("asPlayer",slot);return (Player)this;}public String getName(){String n=name;oracle.Support.emit("name",slot);return n;}public Queue getWalkingQueue(){return walkingQueue;}public Zone getZoneMonitor(){return zoneMonitor;}public static class Queue{int slot;private Location footPrint;public Location rawFootprint(){return footPrint;}public void rawFootprint(Location l){footPrint=l;}Queue(int s){slot=s;}public Location getFootPrint(){Location l=footPrint;oracle.Support.emit("footprint",slot);return l;}}public static class Zone{int slot;Zone(int s){slot=s;}public void updateLocation(Location l){oracle.Support.emit("zone",slot,oracle.Support.coord(l));}}}')
 for kind in ['player.Player','npc.NPC']:
  pkg,cls=kind.split('.');adapter(f'core/game/node/entity/{pkg}/{cls}.java',f'package core.game.node.entity.{pkg};public class {cls} extends core.game.node.entity.Entity{{public {cls}(int s){{super(s);}}}}')
 adapter('core/api/Log.kt','package core.api\nfun log(c:Class<*>,l:core.tools.Log,s:String){oracle.Support.emit("unknown")}')
 adapter('core/tools/Log.java','package core.tools;public enum Log{ERR}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:r=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if r.returncode:raise RuntimeError(r.stderr.decode(errors='replace'))
 rows=[json.loads(l)for l in r.stdout.splitlines()if l];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Original RegionManager.move and generic typed local query bodies; four RegionChunk entity methods, Region.isTolerated, ZoneBorders constructor/contains and packed Location. Clock calls explicitly redirected to deterministic inputs. Entity getter/zone/activity/log and region/chunk lookup services are adapters; cache/region loading, activity pulses, dynamic-region routing and complete Entity classes remain separate.')
 folder=ROOT/'fixtures/entity-regions';folder.mkdir(parents=True,exist_ok=True)
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Entity region fixtures/provenance differ')
  print(f'{len(rows)} original entity region scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original entity region scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
