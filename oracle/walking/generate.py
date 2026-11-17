"""Compile unchanged WalkingQueue/Point/Direction with explicit world services."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/walking-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(name,**kw):return dict(op=name,**kw)
def requests():
 out=[];rng=random.Random(530)
 def add(actions,**kw):out.append(dict(actions=actions,**kw))
 for player in [False,True]:
  for running in [False,True]:
   for toggled in [False,True]:
    for disabled in [False,True]:
     add([op('reset'),op('add',x=3210,y=3218),*[op('update')for _ in range(6)]],player=player,fields=dict(running=running,toggled=toggled,disabled=disabled,energy=50))
 for dx,dy in [(-3,-2),(-2,0),(-2,3),(0,0),(0,-3),(0,3),(3,-2),(3,0),(3,3)]:
  for disable in [False,True]:add([op('reset',running=True),op('add',x=3207+dx,y=3216+dy,disabled=disable),*[op('update')for _ in range(5)]])
 for energy in [-1,0,0.99,1,50,99.9,100,101,'NaN']:
  for weight in [-1,0,25,100]:add([op('reset',running=True),op('add',x=3210,y=3216),op('update'),op('update'),op('energy',decrease=True),op('energy')],fields=dict(energy=energy,weight=weight),hamstrung=True)
 for agility in [-2147483648,-1,1,99,174,175,176,2147483647]:add([op('energy'),op('energy',decrease=True)],fields=dict(energy=99,agility=agility))
 for incrementer in [-2147483648,-1,0,1,2,3,4,2147483647]:add([op('energy',decrease=True)for _ in range(6)],fields=dict(incrementer=incrementer),marathon=True)
 for dx in [0,23,24,31,32,-24,-25,-32,-33]:
  for z in [0,1]:add([op('reset',running=True),op('add',x=3208,y=3216),op('set',teleport=[3207+dx,3216,z]),op('update'),op('set',scene=False),op('update')],fields=dict(last=[3207,3216,0]))
 for scene in [False,True]:
  for move in [False,True]:add([op('region',**{'from':[3207,3216,0],'to':[3239,3216,0],'move':move}),op('update')],fields=dict(last=[3207,3216,0],scene=scene))
 for answers in [[False],[True,False],[True,True]]:add([op('reset',running=True),op('add',x=3210,y=3216),op('update'),op('update'),op('back'),op('update')],zoneAnswers=answers,fields=dict(energy=50))
 for frozen in [False,True]:add([op('reset'),op('add',x=3209,y=3216),op('update'),op('update')],frozen=frozen)
 add([op('add',x=3210,y=3216),op('point',x=3209,y=3216),op('update'),op('reset'),op('point',x=3207,y=3216),op('update')])
 add([op('reset',running=True),op('seed',x=3207,y=3216),op('seed',x=3207,y=3216),op('update'),op('update')])
 add([op('reset'),op('point',x=3210,y=3216),op('point',x=3212,y=3216),op('retain',index=1),op('mutatePoint',disabled=True),op('set',running=True),op('update'),op('mutatePoint',disabled=False),op('update')])
 for x,y in [(16383,16383),(-16384,-16384),(2147483647,-2147483648)]:add([op('reset'),op('point',x=x,y=y),op('update')],location=[x,y,3])
 add([op('update'),op('reset'),op('teleport'),op('set',location=[3207,3216,0]),op('back')],location=None)
 add([op('reset'),op('add',x=3208,y=3216),op('update'),op('back'),op('update')],location='start',locations=dict(start=[3207,3216,0]),fields=dict(last='start'))
 for event in ['timer','marathon','energy','toggle','zone','region','clearPulses','create','routeAdd','routeRemove','nullLocation']:
  for hook in [op('fail'),op('set',location=[3220,3220,1]),op('clear'),op('set',running=False,energy=0),op('routeClear'),op('routeReplace')]:
   add([op('reset',running=True),op('add',x=3210,y=3216),op('update'),op('update'),op('back'),op('clear'),op('update'),op('set',location=None),op('reset')],fields=dict(energy=0.5,draw=True),hooks=[dict(event=event,actions=[hook])])
 # Exercise running-only callbacks, second-step vetoes and ArrayList's late
 # iterator checks independently of low-energy cases above.
 for event in ['marathon','timer','energy','zone','region']:
  for at in [1,2]:
   for hook in [op('fail'),op('set',location=[3220,3220,1]),op('reset',running=False),op('set',teleport=[3200,3200,2])]:
    add([op('reset',running=True),op('add',x=3211,y=3218),op('update'),op('update')],fields=dict(energy=50,weight=25),hooks=[dict(event=event,at=at,actions=[hook])])
 for hook in [op('routeClear'),op('routeReplace'),op('routeNull'),op('fail')]:
  for count in [1,2]:add([op('reset'),*[op('add',x=3207+i,y=3216)for i in range(count)],op('clear'),op('update')],fields=dict(draw=True),hooks=[dict(event='routeRemove',actions=[hook])])
 add([op('reset'),op('point',x=-2147480441,y=3216),op('update')])
 add([op('reset'),op('add',x=-2147480441,y=3216),op('update')])
 add([op('reset'),op('point',x=3209,y=3216),op('update'),op('mutateLocation',which='start',x=42,y=-15,z=3),op('back')],location='start',locations=dict(start=[3207,3216,0]))
 add([op('region',**{'from':[0,0,0],'to':[1,1,0],'move':True})],player=False)
 add([op('reset',running=True),op('set',location=None,teleport=[10,10,1]),op('update'),op('teleport')])
 for scene in [False,True]:add([op('reset'),op('add',x=3210,y=3216),op('update')],fields=dict(last=[3175,3216,0],scene=scene))
 for at in [1,2]:add([op('reset',running=True),op('add',x=3211,y=3217),op('update'),op('back')],fields=dict(energy=50),zoneAnswers=[True,False],hooks=[dict(event='zone',at=at,actions=[op('set',location=[3220,3220,1])])])
 for _ in range(40):
  actions=[op('reset',running=bool(rng.randrange(2)))]
  for _ in range(20):
   name=rng.choice(['update','add','point','reset','energy','set','back'])
   actions.append(op(name,x=3207+rng.randrange(-4,5),y=3216+rng.randrange(-4,5),running=bool(rng.randrange(2)),disabled=bool(rng.randrange(2)),decrease=bool(rng.randrange(2))))
  add(actions,fields=dict(energy=rng.randrange(101),agility=rng.randrange(100),draw=bool(rng.randrange(2))))
 return out
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 for name in ['core/game/node/entity/impl/WalkingQueue.java','core/game/world/map/Point.java','core/game/world/map/Direction.java','core/game/world/map/path/ClipMaskSupplier.java']:
  p=SERVER/name;read(p);java.append(p)
 location=read(SERVER/'core/game/world/map/Location.kt')
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
 class Location(x:Int,y:Int,z:Int){var hash=LocationHash(x,y,(z%4+4)%4);var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};val regionX:Int get()=x shr 3;val regionY:Int get()=y shr 3;'''+next(line.strip()for line in location.splitlines()if 'fun transform(diffX: Int, diffY: Int, z: Int)'in line)+''';companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)};@JvmInline '''+c.method(location,'value class LocationHash(')+'}')
 adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;import core.game.world.map.*;import core.game.node.entity.impl.WalkingQueue;public class Entity{public Location location;public Direction direction;public Props props=new Props();public Flags flags=new Flags();public Settings settings=new Settings();public int agility=1,incrementer;public boolean draw;public WalkingQueue queue;public Location getLocation(){return location;}public void setLocation(Location l){location=l;}public void setDirection(Direction d){if(d!=null)direction=d;}public Props getProperties(){return props;}public Zone getZoneMonitor(){return new Zone();}public Pulse getPulseManager(){return new Pulse();}public WalkingQueue getWalkingQueue(){return queue;}public <T>T getAttribute(String key,T fallback){return (T)(key.equals("routedraw")?(Object)draw:key.equals("run-incrementer")?(Object)incrementer:fallback);}public void setAttribute(String k,Object v){incrementer=((Number)v).intValue();}public void incrementAttribute(String k){incrementer++;}public static class Props{public Location teleport;public boolean teleporting;public Location getTeleportLocation(){return teleport;}public void setTeleportLocation(Location l){teleport=l;}public void setTeleporting(boolean b){teleporting=b;}}public static class Flags{public Location last;public boolean update;public Location getLastSceneGraph(){return last;}public void setUpdateSceneGraph(boolean b){update=b;}public boolean isUpdateSceneGraph(){return update;}}public static class Zone{public boolean move(Location a,Location b){return oracle.Support.zone(a,b);}}public static class Pulse{public void clear(){oracle.Support.emit("clearPulses");}}public static class Settings{public double energy=100,weight;public boolean toggled;public double getRunEnergy(){return energy;}public double getWeight(){return weight;}public boolean isRunToggled(){return toggled;}public void setRunToggled(boolean v){toggled=v;oracle.Support.emit("toggle",v);}public void updateRunEnergy(double drain){energy-=drain;if(energy<0)energy=0;else if(energy>100)energy=100;oracle.Support.emit("energy",oracle.Support.bits(drain));}}}''')
 # Settings state mutations above follow these exact methods; dispatch is explicit.
 read(SERVER/'core/game/node/entity/player/link/Settings.java')
 adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;public class Player extends core.game.node.entity.Entity{public Settings getSettings(){return settings;}public Flags getPlayerFlags(){return flags;}public Skills getSkills(){return new Skills();}public class Skills{public int getLevel(int id){return agility;}}}''')
 adapter('core/game/node/entity/skill/Skills.java','package core.game.node.entity.skill;public class Skills{public static final int AGILITY=16;}')
 adapter('core/game/node/item/Item.java','package core.game.node.item;public class Item{public Item(int id){}}')
 adapter('core/game/node/item/GroundItem.java','''package core.game.node.item;import core.game.world.map.Location;public class GroundItem{public int token;public Location location;public GroundItem(int t,Location l){token=t;location=l;}public GroundItem(Item i,Location l,core.game.node.entity.player.Player p){GroundItem g=oracle.Support.create(l);token=g.token;location=g.location;}public Location getLocation(){return location;}}''')
 adapter('core/game/world/map/RegionManager.java','''package core.game.world.map;import core.game.node.item.GroundItem;public class RegionManager{public static void move(core.game.node.entity.Entity e,Location a,Location b){oracle.Support.region(a,b);}public static Chunk getRegionChunk(Location l){return new Chunk();}public static int getClippingFlag(int z,int x,int y){return 0;}public static class Chunk{public void add(GroundItem i){oracle.Support.emit("routeAdd",i.token,oracle.Support.coord(i.location));}public void remove(GroundItem i){oracle.Support.emit("routeRemove",i.token,oracle.Support.coord(i.location));}}}''')
 adapter('content/global/skill/skillcapeperks/SkillcapePerks.java','package content.global.skill.skillcapeperks;public class SkillcapePerks{public static final int MARATHON_RUNNER=1;public static boolean isActive(int id,core.game.node.entity.player.Player p){return oracle.Support.marathon();}}')
 adapter('core/api/ContentAPIKt.java','package core.api;public class ContentAPIKt{public static boolean hasTimerActive(core.game.node.entity.Entity e,String n){return oracle.Support.timer(n);}public static void log(Class c,core.tools.Log l,String s){oracle.Support.emit("nullLocation");}}')
 adapter('core/tools/Log.java','package core.tools;public enum Log{ERR;}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged WalkingQueue, Point and Direction, original packed LocationHash and transform. Explicit entity, timer/perk, region/zone, ground-item, pulse and packet adapters; Settings energy/toggle mutations follow original methods. Compares ordered callbacks, retained points/locations, run energy, teleports, scene flags, vetoes and failure prefixes. Full Entity/Player/Settings, timer registry, pathfinding, collision integration, world phases and live movement remain separate. Malformed native addPath expansion is bounded to 65536 points.')
 folder=ROOT/'fixtures/walking'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Walking fixtures/provenance differ')
  print(f'{len(rows)} original walking-queue scenarios reproduced.')
 else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original walking-queue scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
