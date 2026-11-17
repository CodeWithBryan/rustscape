"""Reproduce original revision-530 action decoders and outgoing packet classes."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]; HERE=pathlib.Path(__file__).resolve().parent; RUN=ROOT/'runs/game-packets-oracle'; SERVER=ROOT/'reference/server/Server/src/main'; CLIENT=ROOT/'reference/client/client/src/main/java/rt4'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
OPS=[156,55,153,161,135,78,3,148,30,218,254,194,84,247,170,68,71,180,114,175,66,33,115,248,27,134,101,155,196,124,199,234,168,166,64,53,9,10,132,184,73,195,233,239,253,79,231,81,206,94,92,72,215,39,77,22,21,243,245,75,23,111,157,137,177,93,20,110,98]
def table(text,pattern):return[int(x)for x in re.findall(r'-?\d+',re.sub(r'//[^\n]*','',re.search(pattern,text,re.S)[1]))]
def fields_adapter(protocol,client_table):
 def branch(name):return c.method(protocol,'if (opcode == ServerProt.'+name+')')
 def prefix(name,end):return c.clean_annotations(branch(name).split('{',1)[1].split(end,1)[0])
 cases=[]
 def case(op,code,expr):cases.append('case '+str(op)+':{'+code+'return '+expr+';}')
 def obj(name,**fields):return 'obj("type","'+name+'"'+''.join(',"'+k+'",'+v for k,v in fields.items())+')'
 for op,name,end,expr in [
  (38,'UPDATE_STAT','PlayerSkillXpTable.experience',obj('Skill',skill='skill',boosted='level',experience='xp')),
  (234,'UPDATE_RUNENERGY','InterfaceList.miscTransmitAt',obj('RunEnergy',energy='Player.runEnergy')),
  (159,'UPDATE_RUNWEIGHT','InterfaceList.miscTransmitAt',obj('RunWeight',weight='Player.weight')),
  (179,'LOC_ADD','if (tileX',obj('LocAdd',x='tileX',y='tileY',shape='shape',rotation='rotation',id='locId')),
  (195,'LOC_DEL','if (tileX',obj('LocDel',x='tileX',y='tileY',shape='shape',rotation='rotation')),
  (20,'LOC_ANIM','SceneGraph.addLoc',obj('LocAnim',x='tileX',y='tileY',shape='shape',rotation='rotation',sequence='seqId')),
  (112,'UPDATE_ZONE_FULL_FOLLOWS','for (int x',obj('ZoneOrigin',full='true',x='SceneGraph.currentChunkX',y='SceneGraph.currentChunkY')),
  (26,'UPDATE_ZONE_PARTIAL_FOLLOWS','opcode = -1',obj('ZoneOrigin',full='false',x='SceneGraph.currentChunkX',y='SceneGraph.currentChunkY')),
  (155,'IF_OPENTOP','setVerifyId',obj('OpenInterface',kind='type',pointer='pointer',counter='tracknum',interface='component')),
  (145,'IF_OPENSUB','setVerifyId',obj('WindowsPane',interface='parent',reset='reset',counter='tracknum')),
  (149,'IF_CLOSESUB','setVerifyId',obj('CloseInterface',counter='tracknum',pointer='id')),
  (21,'IF_SETHIDE','setVerifyId',obj('InterfaceHidden',hidden='parent',counter='tracknum',pointer='reset')),
  (171,'IF_SETTEXT1','setVerifyId',obj('InterfaceText',pointer='id',text='unsigned(text.bytes)',counter='tracknum')),
  (144,'UPDATE_INV_CLEAR','Component component',obj('InventoryClear',pointer='id')),
  (65,'CLIENT_SETVARC_SMALL','setVerifyId',obj('Varc',counter='tracknum',id='id',value='value')),
  (60,'VARP_SMALL','VarpDomain.set(',obj('Varp',id='id',value='value')),
  (226,'VARP_LARGE','VarpDomain.set(',obj('Varp',id='id',value='value')),
  (37,'VARBIT_SMALL','VarpDomain.setVarbit(',obj('Varbit',id='id',value='value')),
  (84,'VARBIT_LARGE','VarpDomain.setVarbit(',obj('Varbit',id='id',value='value')),
  (69,'CLIENT_SETVARC_LARGE','setVerifyId',obj('Varc',counter='tracknum',id='id',value='value')),
  (165,'SET_INTERFACE_SETTINGS','setVerifyId',obj('InterfaceEvents',counter='tracknum',pointer='pointer',start='start',end='end',mask='accessMask')),
 ]:case(op,prefix(name,end),expr)
 case(115,prefix('RUN_CS2','setVerifyId'),obj('RunClientScript',counter='tracknum',id='scriptArgs[0]',types='unsigned(argTypes.bytes)',arguments='scriptArguments(scriptArgs)'))
 for op,name,full in [(105,'UPDATE_INV_FULL',True),(22,'UPDATE_INV_PARTIAL',False)]:
  body=branch(name);head=prefix(name,'Component component');loop=c.clean_annotations(c.method(body,'for (int slot'if full else'while (inboundBuffer.offset < length)'))
  code=head+'Component component=null;Inv.records=new ArrayList<>();'+('int total=inboundBuffer.g2();'if full else'')+loop
  result=obj('InventoryFull'if full else'InventoryPartial',pointer='componentHash',container='containerId',**({'items':'Inv.items()'}if full else{'slots':'Inv.records'}))
  case(op,code,result)
 case(230,prefix('UPDATE_ZONE_PARTIAL_ENCLOSED','while (length')+'''List<Object> frames=new ArrayList<>();while(length>inboundBuffer.offset){int op=inboundBuffer.g1();int size=SIZES[op];if(size<0)throw new IllegalArgumentException("variable nested opcode");frames.add(obj("opcode",op,"payload",unsigned(Arrays.copyOfRange(inboundBuffer.data,inboundBuffer.offset,inboundBuffer.offset+size))));inboundBuffer.offset+=size;}''',obj('ZoneEnclosed',x='SceneGraph.currentChunkX',y='SceneGraph.currentChunkY',frames='frames'))
 return '''package oracle;import java.util.*;import rt4.*;
public class UpdateFields {
 static final int[] SIZES={'''+','.join(map(str,client_table))+'''};
 static Buffer inboundBuffer; static int length;
 static Map<String,Object> obj(Object...pairs){Map<String,Object> m=new TreeMap<>();for(int i=0;i<pairs.length;i+=2)m.put((String)pairs[i],pairs[i+1]);return m;}
 static List<Integer> unsigned(byte[] b){return GamePackets.unsigned(b);}
 static List<Object> scriptArguments(Object[] args){List<Object> a=new ArrayList<>();for(int i=1;i<args.length;i++)a.add(args[i] instanceof JagString?obj("type","Text","value",unsigned(((JagString)args[i]).bytes)):obj("type","Int","value",args[i]));return a;}
 static class SceneGraph{static int currentChunkX,currentChunkY;}
 static class InterfaceList{static void redrawActiveInterfaces(){}}
 static class Player{static int runEnergy,weight;}
 static class Loc{static int[] LAYERS=new int[64];}
 static class Component{int[] objTypes,objCounts;}
 static class Inv{static List<Object> records;static void updateContainer(int id,int slot,int amount,int container){records.add(obj("slot",slot,"item",obj("id",id,"amount",amount)));}static List<Object> items(){List<Object> a=new ArrayList<>();for(Object v:records)a.add(((Map)v).get("item"));return a;}}
 public static Object decode(int opcode,byte[] payload){inboundBuffer=new Buffer(payload);length=payload.length;SceneGraph.currentChunkX=SceneGraph.currentChunkY=0;try{switch(opcode){'''+''.join(cases)+'''default:throw new IllegalArgumentException("unsupported update");}}catch(ArrayIndexOutOfBoundsException ex){return obj("error","malformed");}}
}
'''
def requests(sizes):
 rng=random.Random(530); cases=[]
 for op in OPS:
  length=sizes[op] if sizes[op]>=0 else (19 if op==39 else 5)
  for data in [[b]*length for b in [0,1,127,128,129,254,255]]+[[rng.randrange(256)for _ in range(length)]for _ in range(12)]+[[rng.randrange(256)for _ in range(n)]for n in range(length)]+[[rng.randrange(256)for _ in range(length+7)]]:
   cases.append(dict(kind='decode',opcode=op,data=data))
 for op in [215,77,39]:
  for steps in [0,1,2,25,100]:
   for runflag in [0,1,2,128,129,255]:
    data=[runflag,255,254,255,255]+[rng.randrange(256)for _ in range(2*steps)]+([rng.randrange(256)for _ in range(14)]if op==39 else[])
    cases.append(dict(kind='decode',opcode=op,data=data))
 for _ in range(120):
  args=[rng.choice([-2147483648,-70001,-1,0,1,2,127,128,255,32767,32768,65535,65536,2147483647])for _ in range(6)]
  for name in ['locAdd','locDel','origin','enclosed','open','pane','close','hidden','text','clear','varc','events','varp','varbit']:
   cases.append(dict(kind=name,args=args,text=rng.choice(['','Hello','Bank of RuneScape','cafÃƒÂ©','ÃŽÂ£Ã¡Â½â€“Ã°Å¸Ëœâ‚¬'])))
 for pointer in [-2147483648,-70001,-70000,-1,0,9764864,2147483647]:
  for amount in [-2147483648,-1,0,1,254,255,2147483647]:
   items=[None,dict(id=995,amount=amount),dict(id=-1,amount=amount),dict(id=65535,amount=amount)]
   for partial in [False,True]:cases.append(dict(kind='partial'if partial else'full',args=[pointer,65535],items=items,slots=[3,2,1,0,2]))
 for opcode,length in [(20,4),(112,2),(37,3),(84,6),(60,3),(226,6)]:
  for data in [[b]*length for b in [0,1,127,128,255]]+[[rng.randrange(256)for _ in range(length)]for _ in range(20)]+[[1]*n for n in range(length)]:cases.append(dict(kind='readUpdate',opcode=opcode,data=data))
 for types in ['','i','s','is','si','iiss','ssii','sisis','x']:
  for counter in [-2147483648,-1,0,65535,2147483647]:
   objects=[dict(text=rng.choice(['','bank','cafÃƒÂ©','ÃŽÂ£Ã°Å¸Ëœâ‚¬']))if t=='s'else dict(integer=rng.choice([-2147483648,-1,0,2147483647]))for t in types[::-1]]
   objects.append(dict(text='ignored extra argument'))
   cases.append(dict(kind='script',args=[rng.choice([-2147483648,-1,149,2147483647]),0,counter],types=types,objects=objects))
 for id in [-1,0,127,128,4893,65535]:
  for value in [-2147483648,-129,-128,-1,0,127,128,255,256,2147483647]:
   for name in ['varp','varbit']:cases.append(dict(kind=name,args=[id,value,123]))
 for _ in range(120):
  for kind in ['skill','energy']:
   cases.append(dict(kind=kind,args=[rng.choice([-2147483648,-1,0,3,5,24,25,255,256,2147483647]),rng.choice([-2147483648,-129,-1,0,1,99,127,128,255,256,2147483647]),65535],value=rng.choice(['NaN','Infinity','-Infinity','-0.0','123.9','-123.9','2147483647.9','-2147483648.9','1.0E99'])))
 for opcode,length in [(38,6),(234,1),(159,2)]:
  for data in [[b]*length for b in [0,1,127,128,255]]+[[rng.randrange(256)for _ in range(length)]for _ in range(20)]+[[1]*n for n in range(length)]:cases.append(dict(kind='readUpdate',opcode=opcode,data=data))
 return cases

def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 server_table=table(read(SERVER/'core/net/event/GameReadEvent.java'),r'PACKET_SIZES\s*=\s*\{(.*?)\}')
 protocol=read(CLIENT/'Protocol.java');client_table=table(protocol,r'PACKET_LENGTHS\s*=\s*new int\[\]\s*\{(.*?)\}')
 assert len(server_table)==len(client_table)==256
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);java=[];kt=[];jars=c.artifacts();jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(p)
 def source(path):
  p=SERVER/path;read(p);(java if path.endswith('.java')else kt).append(p)
 for path in ['core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java','core/net/packet/Context.java','core/net/packet/OutgoingPacket.java','core/net/IoWriteEvent.java','core/net/event/GameWriteEvent.java','core/net/packet/in/Packet.kt','core/net/packet/in/Decoders530.kt']:source(path)
 for name in ['Interface','WindowsPane','CloseInterface','InterfaceConfig','StringPacket','ContainerPacket','UpdateAreaPosition','ConstructScenery','ClearScenery','VarcUpdate','RunScriptPacket','AccessMask','Config','Varbit','SkillLevel','RunEnergy']:source('core/net/packet/out/'+name+'.java')
 for name in ['Interface','WindowsPane','InterfaceConfig','String','Container','BuildScenery','AreaPosition','VarcUpdate','RunScript','AccessMask','Config','Varbit','Skill','Player']:source('core/net/packet/context/'+name+'Context.java')
 for name in ['Buffer','Node','GlobalConfig','IsaacRandom','Packet']:
  p=CLIENT/(name+'.java');read(p);java.append(p)
 for p in sorted((ROOT/'reference/client/deob-annotations/src/main/java').rglob('*.java')):read(p);java.append(p)
 jag=read(ROOT/'oracle/adapters/rt4/JagString.java');original_jag=read(CLIENT/'JagString.java');jag=jag.replace('public final byte[] bytes;','public final byte[] bytes; public final byte[] chars;').replace('this.bytes = bytes.clone();','this.bytes = bytes.clone(); this.chars = this.bytes;');adapter('rt4/JagString.java',jag[:jag.rfind('}')]+c.clean_annotations(c.method(original_jag,'public final int charAt('))+'}')
 adapter('core/cache/crypto/ISAACCipher.java','package core.cache.crypto;public class ISAACCipher {public int getNextValue(){return 0;}}')
 adapter('core/cache/misc/buffer/ByteBufferUtils.java','package core.cache.misc.buffer;import java.nio.ByteBuffer;public class ByteBufferUtils{public static String getString(ByteBuffer b){throw new UnsupportedOperationException("text outside scope");}public static int packGJString2(int a,byte[] b,String c){throw new UnsupportedOperationException("text outside scope");}}')
 adapter('core/tools/Log.java','package core.tools;public enum Log{ERR,FINE,WARN}')
 adapter('core/tools/SystemLogger.java','package core.tools;public class SystemLogger{}')
 adapter('core/tools/StringUtils.java','package core.tools;import core.net.packet.IoBuffer;public class StringUtils{public static String longToString(long x){throw new UnsupportedOperationException("social outside scope");}public static String decryptPlayerChat(IoBuffer b,int n){throw new UnsupportedOperationException("chat outside scope");}}')
 util=read(SERVER/'core/Util.java');adapter('core/Util.java','package core;public class Util{'+c.method(util,'public static int clamp(')+'}')
 adapter('core/api/Log.kt','package core.api\nimport core.tools.Log\nfun log(owner:Any,level:Log,message:String){throw UnsupportedOperationException("logging outside numeric scope")}')
 qc=read(SERVER/'core/net/packet/in/QuickChatPacketHandler.kt');adapter('core/net/packet/in/QCPacketType.kt','package core.net.packet.`in`\n'+c.method(qc,'enum class QCPacketType'))
 adapter('core/game/node/entity/player/Player.java',read(HERE/'Player.java'))
 adapter('core/game/node/entity/skill/Skills.java','package core.game.node.entity.skill;public class Skills{public static final int PRAYER=5,HITPOINTS=3;public int level;public double experience;public double getPrayerPoints(){return level;}public int getLifepoints(){return level;}public int getLevel(int id,boolean boosted){return level;}public double getExperience(int id){return experience;}}')
 adapter('core/net/IoSession.java',read(HERE/'IoSession.java'))
 adapter('core/game/node/item/Item.java','package core.game.node.item;public class Item{private int id,amount;public Item(int i,int a){id=i;amount=a;}public int getId(){return id;}public int getAmount(){return amount;}}')
 adapter('core/game/container/ContainerEvent.java','package core.game.container;public class ContainerEvent{public static final core.game.node.item.Item NULL_ITEM=new core.game.node.item.Item(-1,1);}')
 adapter('core/game/container/Container.java','package core.game.container;public class Container{public core.game.node.item.Item[] toArray(){throw new UnsupportedOperationException("container logic outside scope");}}')
 adapter('core/game/world/map/Location.java','package core.game.world.map;public class Location{private int x,y;public Location(int x,int y){this.x=x;this.y=y;}public int getChunkOffsetX(){return x&7;}public int getChunkOffsetY(){return y&7;}public Location getChunkBase(){return new Location(x&~7,y&~7);}public int getSceneX(Location l){return x;}public int getSceneY(Location l){return y;}}')
 adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;import core.game.world.map.Location;public class Scenery{int id,type,rotation;Location l;public Scenery(int i,int t,int r,int x,int y){id=i;type=t;rotation=r;l=new Location(x,y);}public int getId(){return id;}public int getType(){return type;}public int getRotation(){return rotation;}public Location getLocation(){return l;}}')
 adapter('oracle/GamePackets.java',read(HERE/'GamePackets.java'))
 adapter('oracle/UpdateFields.java',fields_adapter(protocol,client_table))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests(server_table);file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.GamePackets'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'),ensure_ascii=True)+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(cases),fixtureSha256=hashlib.sha256(data).hexdigest(),clientPacketSizes=server_table,serverPacketSizes=client_table,sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged Decoders530.kt, Packet.kt, IoBuffer.java, GameWriteEvent and sixteen outgoing packet classes with original contexts. Numeric decoders for selected 69 opcodes only; original RT4 Buffer/Packet primitives and verbatim selected Protocol field-read prefixes, client-script argument loops, original JagString.charAt and inventory loops. Layer mapping, scene mutation and UI effects are excluded. Explicit world/entity/item/NULL_ITEM/interface-counter/artificial-player-false/ISAAC-zero/scene-coordinate adapters; skill level/prayer/lifepoints resolved to the controlled same integer, XP and run-energy double values supplied explicitly; no chat/social decoding, action handling, UI reducer, geometry, collision or live world-entry claim.')
 folder=ROOT/'fixtures/game-packets'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Game packet fixtures/provenance differ')
  print(f'{len(out)} original game-packet cases reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original game-packet cases.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
