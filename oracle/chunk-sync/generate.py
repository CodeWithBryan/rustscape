"""Original chunk rebuild/update, dirty tracker and complete GroundItem replay."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/chunk-sync-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 cases=[]
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions,**kw):cases.append(dict(actions=actions,**kw))
 def ground(**kw):return a('newGround',location=[3201,3202,0],**kw)
 def obj(**kw):return a('newObject',location=[3201,3202,0],**kw)
 sync=a('sync');update=a('update');reset=a('resetAll')
 add([sync,update,reset])
 for shape in [-1,0,3,4,8,9,10,11,17,21,22,23]:
  for other in [None,0,4,10,22]:
   actions=[obj(id=10,shape=shape),a('place',array='stat',o=0)]
   if other is not None:actions +=[obj(id=11,shape=other),a('place',array='dyn',o=1,i=1)]
   add([*actions,sync,a('setObject',o=0,renderable=False),sync])
 for same in [True,False]:
  for renderable in [True,False]:add([obj(id=10,renderable=renderable),obj(id=10 if same else 11,renderable=renderable),a('place',array='stat',o=0),a('place',array='dyn',o=1),sync])
 for ctor in ['item','location','owner','uid','decay']:
  for tick in [-2147483648,0,100,2147483647]:
   add([ground(ctor=ctor,p=0,uid=7,decay=200,absolute=100),a('itemPrivate',g=0),a('itemActive',g=0),a('droppedBy',g=0,p=1),a('droppedBy',g=0,p=0),a('string',g=0),a('setDecay',g=0,delay=2147483647),a('itemPrivate',g=0)],tick=tick)
 for tick in [99,100,101,199,200,201,2147483647,-2147483648]:
  for flags in [{},{'removed':True},{'remainPrivate':True},{'forceVisible':True,'remainPrivate':True}]:
   add([ground(ctor='owner',p=0),a('setGround',g=0,**flags),a('tick',value=tick),a('setItems',items=[None,0]),sync,a('sync',p=1),a('string',g=0)])
 for kind in [0,1,2,-1,123]:
  add([ground(ctor='owner',p=0),a('newFlag',kind='item',g=0,type=kind,old=-1),a('flag',f=0),update,a('update',p=1),update,reset,update])
 add([obj(id=10),a('newFlag',kind='object',o=0),a('flag',f=0),a('flag',f=0,c=1),a('flag',f=0),update,a('update',c=1),reset,update,a('update',c=1)])
 add([a('flag',f=None),update,reset,update])
 for attr in ['chunkdraw','regiondraw','clippingdraw']:
  for base in [[3200,3200,0],[3208,3216,1],[3256,3256,3]]:add([a('base',location=base),a('attr',name=attr,value=True),a('draw'),sync])
 for event in ['item','default','uid','ticks','scene','attr','send','dirty','foreign','clip','username']:
  for at in [1,2]:add([ground(ctor='owner',p=0),a('setItems',items=[0]),a('newFlag',kind='foreign',value=88),a('flag',f=0),update,sync,a('string',g=0),reset],failures={event:at})
 mutations=[a('resetAll'),a('flag',f=0),a('setItems',items=None),a('setGround',g=0,location=None,decay=123,removed=True),a('tick',value=200),a('player',p=0,uid=9,scene=[3264,3264,1]),a('mutForeign',f=0,value=99)]
 for event in ['uid','ticks','scene','attr','send','dirty','foreign','username']:
  for action in mutations:add([ground(ctor='owner',p=0),a('setItems',items=[0]),a('newFlag',kind='foreign',value=88),a('flag',f=0),update,sync,a('string',g=0),a('update',p=1),reset],hooks=[dict(event=event,actions=[action])])
 add([ground(ctor='uid',uid=-1,absolute=500),a('player',p=1,uid=-1),a('droppedBy',g=0,p=1),a('captureItems'),a('setItems',items=[0]),a('captureItems'),a('setItems',items=None),a('captureItems'),a('draw')])
 # Preserve the first 212 scenarios while extending arithmetic, callback-time
 # captures and failed writer prefixes observed through retained foreign buffers.
 for kind in [0,1,2,-1]:
  add([ground(ctor='owner',p=0),a('newFlag',kind='foreign',value=88),a('newFlag',kind='item',g=0,type=kind,old=2147483647),a('flag',f=0),a('flag',f=1),a('update',p=1),reset],hooks=[dict(event='foreign',actions=[a('setGround',g=0,location=None)])])
 for id,amount,charge in [(-1,-1,-1),(65536,2147483647,65536),(2147483647,-2147483648,-2147483648),(32768,65536,1000),(42,0,65535),(42,65535,0)]:
  add([ground(ctor='decay',p=0,id=id,amount=amount,charge=charge,decay=-1),a('setGround',g=0,removed=True,forceVisible=True),a('setItems',items=[0]),sync,a('newFlag',kind='item',g=0,type=2,old=-2147483648),a('flag',f=0),a('update',p=1),reset])
 for decay in [-2147483648,-1,0,100,101,2147483647]:
  for tick in [-2147483648,0,2147483647]:
   add([ground(ctor='uid',uid=7,absolute=decay),a('tick',value=tick),a('itemPrivate',g=0),a('itemActive',g=0),a('setItems',items=[0]),sync,a('sync',p=1)])
 for query in ['itemPrivate','itemActive']:
  for at in [2,3]:add([ground(ctor='owner',p=0),a(query,g=0),a(query,g=0),a('string',g=0)],hooks=[dict(event='ticks',at=at,actions=[a('setGround',g=0,decay=-1)])])
 for i in range(4):
  for j in range(4):
   if i!=j:add([obj(id=10,shape=0),obj(id=11,shape=3,renderable=False),a('place',array='stat',o=0,i=i),a('place',array='dyn',o=1,i=j),sync])
 add([a('attr',name=name,value=True)for name in ['chunkdraw','regiondraw','clippingdraw']]+[a('draw'),sync])
 add([ground(ctor='owner',p=0),a('setItems',items=[0]),sync],hooks=[dict(event='attr',actions=[a('setItems',items=None)])])
 add([obj(id=10,shape=23),obj(id=11,shape=23),a('place',array='stat',o=0),a('place',array='dyn',o=1,i=1),sync],hooks=[dict(event='invalidLayer',actions=[a('place',array='dyn',o=None,i=1)])])
 return cases
def generate(check,extension=None,kind='chunk-sync'):
 global RUN
 RUN=ROOT/'runs'/(kind+'-oracle')
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server';assert run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True)
 def source(name):
  p=SERVER/name
  if not p.exists():
   data=run(['git','-C',ref,'show',lock['server']['commit']+':Server/src/main/'+name]).stdout;p=RUN/'original'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
  return p,read(p)
 classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  assert len(data)==artifact['size']and hashlib.sha256(data).hexdigest()==artifact['sha256'];jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  if extension is not None:text=extension.transform(name,text,source,c)
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf8');(kt if p.suffix=='.kt'else java).append(p)
 for name in ['core/game/node/item/GroundItem.java','core/game/world/update/flag/UpdateFlag.java','core/game/world/update/flag/chunk/ItemUpdateFlag.java','core/game/world/update/flag/chunk/ObjectUpdateFlag.java','core/game/world/update/ChunkDirtyListener.kt']:
  p,_=source(name);(kt if p.suffix=='.kt'else java).append(p)
 _,io=source('core/net/packet/IoBuffer.java');methods=''.join(c.method(io,s)for s in ['public IoBuffer put(int val)','public IoBuffer putA(int val)','public IoBuffer putC(int val)','public IoBuffer putS(int val)','public IoBuffer putShort(int val)','public IoBuffer putLEShort(int val)','public IoBuffer putShortA(int val)'])
 adapter('core/net/packet/IoBuffer.java','package core.net.packet;import java.nio.*;public class IoBuffer{public int opcode;private ByteBuffer buf=ByteBuffer.allocate(262144);public IoBuffer(int op){opcode=op;}public IoBuffer(int op,PacketHeader h){opcode=op;}public String hex(){byte[]b=new byte[buf.position()];System.arraycopy(buf.array(),0,b,0,b.length);StringBuilder s=new StringBuilder();for(byte v:b)s.append(String.format("%02x",v&255));return s.toString();}'+methods+'}')
 adapter('core/net/packet/PacketHeader.java','package core.net.packet;public enum PacketHeader{SHORT}')
 for cls,arg in [('ConstructScenery','Scenery object'),('ClearScenery','Scenery object'),('ConstructGroundItem','Item item'),('ClearGroundItem','Item item'),('UpdateGroundItemAmount','Item item, int oldAmount')]:
  _,text=source('core/net/packet/out/'+cls+'.java');method=c.method(text,'public static IoBuffer write(IoBuffer buffer, '+arg+')');adapter('core/net/packet/out/'+cls+'.java','package core.net.packet.out;import core.game.node.item.*;import core.game.node.scenery.*;import core.game.world.map.Location;import core.net.packet.IoBuffer;public class '+cls+'{'+method+'}')
 _,area=source('core/net/packet/out/UpdateAreaPosition.java');adapter('core/net/packet/out/UpdateAreaPosition.java','package core.net.packet.out;import core.game.node.entity.player.Player;import core.game.world.map.Location;import core.net.packet.*;public class UpdateAreaPosition{'+c.method(area,'public static IoBuffer getChunkUpdateBuffer(Player player, Location base)')+'}')
 _,chunk=source('core/game/world/map/RegionChunk.java');methods=''.join(c.method(chunk,s)for s in ['public void flag(UpdateFlag<?> flag)','public void synchronize(Player player)','protected boolean appendUpdate(Player player, IoBuffer buffer)','public ArrayList<GroundItem> drawItems(List<GroundItem> items, Player player)','public void update(Player player)','public boolean isUpdated()','public void resetUpdateFlags()','public List<GroundItem> getItems()','public void setItems(List<GroundItem> items)'])
 adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;import java.util.*;import core.game.node.item.*;import core.game.node.entity.player.Player;import core.game.node.scenery.*;import core.game.world.update.*;import core.game.world.update.flag.UpdateFlag;import core.net.packet.*;import core.net.packet.out.*;import org.rs09.consts.Items;public class RegionChunk{public static final int SIZE=8,ARRAY_SIZE=4;public static ChunkDirtyListener dirtyListener=c->{};public int id;public Location currentBase;public Scenery[][][]statObjects=new Scenery[8][8][4],objects=new Scenery[8][8][4];public List<GroundItem>items=new ArrayList<>();private final List<UpdateFlag<?>>updateFlags=new ArrayList<>(20);public List<UpdateFlag<?>>rawFlags(){return updateFlags;}public RegionChunk(int id,Location l){this.id=id;currentBase=l;}'+methods+'}')
 _,location=source('core/game/world/map/Location.kt');adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,z);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val localX:Int get()=x and 63;val localY:Int get()=y and 63;val chunkOffsetX:Int get()=x and 7;val chunkOffsetY:Int get()=y and 7;fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);fun getSceneX(l:Location)=x-(((l.x shr 3)-6) shl 3);fun getSceneY(l:Location)=y-(((l.y shr 3)-6) shl 3);override fun equals(o:Any?)=o is Location&&o.hash.hash==hash.hash;override fun hashCode()=hash.hash;@JvmInline '+c.method(location,'value class LocationHash(')+'}')
 _,scenery=source('core/game/node/scenery/Scenery.java');adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;import core.game.world.map.Location;public class Scenery{public int id,type,rotation;public boolean renderable=true;public Location location;public Scenery(int id,Location l,int type,int rot){this.id=id;location=l;this.type=type;rotation=rot;}public int getId(){return id;}public int getType(){return type;}public int getRotation(){return rotation;}public Location getLocation(){return location;}public boolean isRenderable(){return renderable;}public enum Layer{WALL,WALL_DECOR,GROUND,GROUND_DECOR}enum Log{ERR}static void log(Class<?>c,Log l,String s){oracle.Support.emit("invalidLayer",s);}'+c.method(scenery,'public Layer getLayer()')+c.method(scenery,'public boolean equals(Object obj)')+'}')
 for p in sorted((HERE/'adapters').rglob('*.java')):
  text=read(p)
  if extension is None:java.append(p)
  else:adapter(p.relative_to(HERE/'adapters').as_posix(),text)
 text=read(HERE/'Support.java')
 if extension is None:java.append(HERE/'Support.java')
 else:
  adapter('oracle/Support.java',text)
  extension.register(adapter,source,read,c)
 cp=os.pathsep.join(map(str,[classes,std,jar]));run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests()if extension is None else extension.requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
 if result.returncode:
  (RUN/'failure.stdout.jsonl').write_bytes(result.stdout);(RUN/'failure.stderr.log').write_bytes(result.stderr)
  raise RuntimeError(result.stderr.decode(errors='replace'))
 rows=[json.loads(x)for x in result.stdout.splitlines()if x];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged GroundItem, Item/ObjectUpdateFlag, UpdateFlag and ChunkUpdateTracker; verbatim RegionChunk sync/update/drawItems/flag/reset/item-list methods and outgoing writers. Original scenery equality/layer and integer IoBuffer methods. Item/Node constructors, player identity/clock/attributes, clipping, foreign flag bodies and session are explicit adapters. No full ground-item manager, region activity, live fanout or capacity claim.')
 if extension is not None:provenance['scope']=extension.SCOPE
 folder=ROOT/'fixtures'/kind;folder.mkdir(parents=True,exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original chunk/item scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original chunk/item scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
