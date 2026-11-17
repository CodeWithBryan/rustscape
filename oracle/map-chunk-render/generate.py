"""Compile the unchanged pinned MapChunkRenderer with explicit world services."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/map-chunk-render-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 cases=[]
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions,**kw):cases.append(dict(actions=actions,**kw))
 render=a('render')
 add([render,render])
 for anchor in [None,[3207,3216,0],[3207,3216,3]]:
  for l in [[3208,3217,0],[3216,3216,0],[3264,3216,0],[3400,3400,0],[3207,3216,1]]:add([render,a('set',location=l),render,render],anchor=anchor)
 for l in [[0,0,0],[16383,16383,3],[-16384,-16384,0],[32768,-1,-1]]:add([render,a('set',anchor=l),render,render],location=l)
 for l in [[3216,3216,0],[3264,3216,1],[3400,3400,2]]:add([render,a('set',anchor=l,location=l),render,render])
 for op in [a('replaceChunk',location=[3207,3216,0]),a('rebase',location=[3207,3216,0],base=[3400,3400,0]),a('alias',location=[3208,3216,0],other=[3207,3216,0]),a('setRow',x=0,length=2),a('setRow',x=0,length=None),a('newViewport')]:add([render,op,render,render])
 for event in ['viewport','scene','location','chunk','region','sync','update','clear','name','increment','decrement','active','flag','inactive']:
  for at in [1,2,170]:add([render,a('set',anchor=[3400,3400,0],location=[3400,3400,1]),render,render],failures={event:at})
 for event in ['chunk','region','sync','update','clear','name','increment','decrement','active','flag','inactive']:
  for action in [a('set',location=[3400,3400,1],anchor=[3400,3400,2],name='changed'),a('setRow',x=0,length=2),a('newViewport')]:add([render,a('set',anchor=[3400,3400,0]),render,render],hooks=[dict(event=event,actions=[action])])
 return cases
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 assert run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];run(['git','-C',ref,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
  assert len(data)==artifact['size']and hashlib.sha256(data).hexdigest()==artifact['sha256'];jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf8');(kt if p.suffix=='.kt'else java).append(p)
 source=SERVER/'core/game/world/update/MapChunkRenderer.kt';read(source);kt.append(source)
 loc=read(SERVER/'core/game/world/map/Location.kt')
 adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,z);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val chunk:RegionChunk get()=oracle.Support.chunk(this);val region:Region get()=oracle.Support.region(this);fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)}\n@JvmInline '+c.method(loc,'value class LocationHash(')+'}')
 adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player;import core.game.world.map.*;public class Player{public Flags flags=new Flags();public Location rawLocation;public String rawName="viewer";public Flags getPlayerFlags(){oracle.Support.emit("viewport");return flags;}public Location getLocation(){Location l=rawLocation;oracle.Support.emit("location");return l;}public String getName(){String n=rawName;oracle.Support.emit("name");return n;}public static class Flags{public RegionChunk[][]viewport=new RegionChunk[13][13];public Location sceneGraph;public RegionChunk[][]getLastViewport(){return viewport;}public Location getLastSceneGraph(){Location l=sceneGraph;oracle.Support.emit("scene");return l;}}}')
 # Player.getPlayerFlags occurs twice in the original body. Only the first
 # access exposes lastViewport; the native interface records both receivers.
 adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;import core.game.node.entity.player.Player;public class RegionChunk{public static final int SIZE=8;public int id;public Location currentBase;public RegionChunk(int i,Location l){id=i;currentBase=l;}public Location getCurrentBase(){return currentBase;}public void update(Player p){oracle.Support.emit("update",id);}public void synchronize(Player p){oracle.Support.emit("sync",id);}}')
 region=read(SERVER/'core/game/world/map/Region.java')
 methods=''.join(c.method(region,s)for s in ['public int incrementViewAmount(String playerName)','public int decrementViewAmount(String playerName)']).replace('incrementViewAmount','originalIncrement').replace('decrementViewAmount','originalDecrement')
 adapter('core/game/world/map/Region.java','package core.game.world.map;import java.util.*;public class Region{public int id;public boolean active;public List<String>viewingPlayers=new ArrayList<>();public Region(int i){id=i;}'+methods+'public int incrementViewAmount(String n){int r=originalIncrement(n);oracle.Support.emit("increment",id,n);return r;}public int decrementViewAmount(String n){int r=originalDecrement(n);oracle.Support.emit("decrement",id,n);return r;}public boolean isActive(){boolean r=active;oracle.Support.emit("active",id,r);return r;}public void flagActive(){oracle.Support.emit("flag",id);active=true;}public boolean checkInactive(){oracle.Support.emit("inactive",id);return true;}}')
 adapter('core/net/packet/context/ClearChunkContext.java','package core.net.packet.context;import core.game.node.entity.player.Player;import core.game.world.map.RegionChunk;public class ClearChunkContext{public RegionChunk chunk;public ClearChunkContext(Player p,RegionChunk c){chunk=c;}}')
 adapter('core/net/packet/out/ClearRegionChunk.java','package core.net.packet.out;public class ClearRegionChunk{}')
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;import core.net.packet.context.ClearChunkContext;public class PacketRepository{public static void send(Class<?>c,ClearChunkContext x){oracle.Support.emit("clear",x.chunk.id);}}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in result.stdout.splitlines()if line];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged MapChunkRenderer with original packed Location and Region viewer-list method bodies. Canonical chunk/region routing, player getter/callback observations, packet send and chunk update/synchronize/activity are explicit adapters. Tests identity, traversal, retained arrays, anchor/current plane, callback errors and partial publication; no RegionChunk serialization, dirty reset or live fanout claim.')
 folder=ROOT/'fixtures/map-chunk-render';folder.mkdir(parents=True,exist_ok=True)
 if check:
  assert(folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original map viewport scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original map viewport scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
