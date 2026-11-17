"""Original NPC ambient collection, sound metadata and real synthesized effects."""
import argparse,hashlib,importlib.util,json,pathlib,random,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/audio-npc-oracle'
spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)

def generate(check=False):
 RUN.mkdir(parents=True,exist_ok=True);sources={}
 def source(name):
  raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
 for name in ['Node','LinkedList','AreaSound']:(RUN/(name+'.java')).write_text(source(name),encoding='utf-8')
 manager=source('AreaSoundManager');npc=source('Npc');bas=source('BasType')
 methods=['public static void add(', 'public static void remove(Npc npc)', 'public static void redraw(int loops,', 'public static void redraw(int x,', 'public static void updateMulti(', 'public static void clear(']
 (RUN/'AreaSoundManager.java').write_text('class AreaSoundManager{static LinkedList locSounds=new LinkedList(),npcSounds=new LinkedList();static HashTable playerSounds=new HashTable(16);'+''.join(v.method(manager,s)for s in methods)+'}',encoding='utf-8')
 fields='\n'.join(re.findall(r'public int(?:\[\])* \w+(?:\s*=\s*[^;]+)?;',bas[:bas.index('public final void postDecode(')]))
 code=r'''import java.io.*;import java.util.*;
class BasType{ BAS }
class NpcType{int size,soundRadius,idleSound,crawlSound,walkSound,runSound;int[]multiNpcs;NpcType next;NpcType getMultiNpc(){return next;}}
class Npc{int serial,size,xFine,yFine,movementSeqId;NpcType type;BasType bas;int getSize(){return size;}BasType getBasType(){return bas;} GETSOUND }
class LocType{int width,length,bgsound,bgsoundrange,bgsoundmin,bgsoundmax;int[]bgsounds,multiLocs;LocType getMultiLoc(){return null;}}
class Username{long encode37(){return 0;}}class Player{int xFine,yFine,movementSeqId,soundRadius;Username username=new Username();int getSize(){return 1;}BasType getBasType(){return new BasType();}static int getSound(Player p){return 0;}}
class HashTable{HashTable(int n){}void put(Node n,long key){}Node get(long key){return null;}Node head(){return null;}Node next(){return null;}}
class Preferences{static int ambientSoundsVolume;}class PcmSound{int id;PcmSound(int id){this.id=id;}PcmSound resample(Object r){return this;}}
class SynthSound{int id;SynthSound(int id){this.id=id;}static SynthSound create(Object a,int id,int f){return id%11==0?null:new SynthSound(id);}PcmSound toPcmSound(){return new PcmSound(id);}}
class SoundPcmStream{int serial,id,volume,loops;boolean linked;static int next;static Map<Integer,SoundPcmStream>all=new HashMap<>();static SoundPcmStream create(PcmSound p,int volume){SoundPcmStream s=new SoundPcmStream();s.serial=++next;s.id=p.id;s.volume=volume;all.put(s.serial,s);return s;}void setLoops(int n){loops=n;}void setVolume(int n){volume=n;Probe.events.add("[1,"+serial+","+n+"]");}boolean isLinked(){return linked;}}
class Mixer{void addSubStream(SoundPcmStream s){s.linked=true;Probe.events.add("[0,"+s.serial+","+s.id+","+s.volume+","+s.loops+"]");}void removeSubStream(SoundPcmStream s){s.linked=false;Probe.events.add("[2,"+s.serial+"]");}}
class client{static Mixer soundStream=new Mixer();static Object js5Archive4,resampler;}
public class Probe{static List<String>events=new ArrayList<>();static Map<Integer,Npc>actors=new HashMap<>();static int nextActor;
static int[]ints(String s){return s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
static NpcType definition(String s){if(s.equals("null"))return null;int[]a=ints(s);NpcType d=new NpcType();d.size=a[0];d.soundRadius=a[1];d.idleSound=a[2];d.crawlSound=a[3];d.walkSound=a[4];d.runSound=a[5];return d;}
static BasType bas(String s){int[]a=ints(s);BasType b=new BasType();b.idleAnimationId=a[0];b.walkAnimation=a[1];b.runAnimationId=a[2];b.standingCCWTurn=a[3];b.standingCWTurn=a[4];b.walkFullTurnAnimationId=a[5];b.walkCCWTurnAnimationId=a[6];b.walkCWTurnAnimationId=a[7];b.slowWalkAnimationId=a[8];b.slowWalkFullTurnAnimationId=a[9];b.slowWalkCCWTurnAnimationId=a[10];b.slowWalkCWTurnAnimationId=a[11];b.runFullTurnAnimationId=a[12];b.runCCWTurnAnimationId=a[13];b.runCWTurnAnimationId=a[14];return b;}
static String stream(SoundPcmStream s){return s==null?"null":Arrays.toString(new int[]{s.serial,s.id,s.volume,s.loops,s.linked?1:0});}
static String snapshot(){List<String>rows=new ArrayList<>();Node sentinel=AreaSoundManager.npcSounds.sentinel;for(Node node=sentinel.nextNode;node!=sentinel;node=node.nextNode){AreaSound a=(AreaSound)node;rows.add("{\"actor\":"+a.npc.serial+",\"fields\":"+Arrays.toString(new int[]{a.minXFine,a.minYFine,a.maxXFine,a.maxYFine,a.level,a.sound,a.radius,a.movementSpeed,a.multiLocOrNpc?1:0})+",\"primary\":"+stream(a.primaryStream)+"}");}return "{\"entries\":"+rows+",\"events\":"+events+"}";}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);switch(p[0]){
case"Z":AreaSoundManager.locSounds.clear();AreaSoundManager.npcSounds.clear();actors.clear();events.clear();nextActor=0;SoundPcmStream.next=0;SoundPcmStream.all.clear();continue;
case"N":{int[]a=ints(p[2]);Npc n=new Npc();n.serial=++nextActor;n.xFine=a[0];n.yFine=a[1];n.size=a[2];n.movementSeqId=a[3];n.bas=bas(p[3]);NpcType d=definition(p[4]);if(a[4]!=0){n.type=new NpcType();n.type.multiNpcs=new int[]{1};n.type.next=d;}else n.type=d;actors.put(Integer.parseInt(p[1]),n);break;}
case"P":{Npc n=actors.get(Integer.parseInt(p[1]));int[]a=ints(p[2]);n.xFine=a[0];n.yFine=a[1];n.size=a[2];n.movementSeqId=a[3];n.bas=bas(p[3]);break;}
case"M":{Npc n=actors.get(Integer.parseInt(p[1]));NpcType d=definition(p[2]);if(n.type.multiNpcs!=null)n.type.next=d;else n.type=d;break;}
case"A":{int[]a=ints(p[2]);AreaSoundManager.add(a[1],null,0,actors.get(Integer.parseInt(p[1])),a[0],a[2],null);break;}
case"D":AreaSoundManager.remove(actors.get(Integer.parseInt(p[1])));break;
case"U":AreaSoundManager.updateMulti();break;
case"C":AreaSoundManager.clear(Integer.parseInt(p[1])!=0);break;
case"F":{SoundPcmStream s=SoundPcmStream.all.get(Integer.parseInt(p[1]));if(s!=null)s.linked=false;break;}
case"R":{int[]a=ints(p[1]);Preferences.ambientSoundsVolume=a[4];AreaSoundManager.redraw(a[3],a[0],a[1],a[2]);break;}
default:throw new IllegalArgumentException(p[0]);}System.out.println(snapshot());events.clear();}}}
'''.replace(' BAS ',fields).replace(' GETSOUND ',v.method(npc,'public static int getSound('))
 (RUN/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*[str(p)for p in RUN.glob('*.java')]],check=True)
 rng=random.Random(530_134);cases=[];requests=[]
 def csv(a):return ','.join(map(str,a))
 def wire(c):return ';'.join(csv(x)if isinstance(x,list)else str(x)if x is not None else'null'for x in c)
 def pose():return [rng.randrange(20,80)*128+rng.randrange(128),rng.randrange(20,80)*128+rng.randrange(128),rng.randrange(1,5),rng.choice([-1,0,1,2,3,4,65535])]
 def ids():return [rng.choice([-1,0,1,2,3,4,65535])for _ in range(15)]
 def definition():return [rng.randrange(1,6),rng.choice([0,1,2,8,16]),*[rng.choice([-1,0,11,127,223,387])for _ in range(4)]]
 b=list(range(10,25));d=[1,4,127,223,387,127];larger=[3,8,127,223,387,127]
 guided=[['N',0,[6400,6400,1,10,1],b,d],['A',0,[50,50,0]],['R',[6400,6400,0,1,127]],['F',1],['R',[6400,6400,0,1,127]],['M',0,larger],['U'],['R',[7000,6400,0,1,127]],['P',0,[6528,6400,2,11],b],['R',[6528,6400,0,1,127]],['M',0,None],['U'],['R',[6528,6400,0,1,127]],['M',0,larger],['R',[6528,6400,0,1,127]],['U'],['R',[6528,6400,0,1,127]],['A',0,[50,50,0]],['R',[6528,6400,0,1,127]],['D',0],['N',0,[6400,6400,1,10,0],b,d],['D',0],['M',0,[1,1,223,223,223,223]],['R',[6528,6400,0,1,127]],['C',0],['R',[6528,6400,1,1,127]],['R',[6528,6400,0,1,127]],['C',1]]
 cases.append(dict(commands=guided));requests+=['Z']+[wire(c)for c in guided]
 b=[2150,2150]+[-1]*13;d=[1,15,2411,1605,1605,1605]
 spawn_alias=[['N',0,[6400,6400,1,-1,0],b,d],['A',0,[50,50,0]],['P',0,[6400,6400,1,2150],b],['R',[6400,6400,0,1,127]],['P',0,[6404,6400,1,-1],b],['R',[6400,6400,0,1,127]],['P',0,[6404,6400,1,2150],b],['R',[6400,6400,0,1,127]],['C',1]]
 cases.append(dict(commands=spawn_alias));requests+=['Z']+[wire(c)for c in spawn_alias]
 for scenario in range(64):
  commands=[];multi={}
  for key in range(4):
   multi[key]=(scenario+key)%2;commands.append(['N',key,pose()+[multi[key]],ids(),None if multi[key]and scenario%3==0 else definition()])
  commands += [['A',0,[50,50,scenario%4]],['A',0,[50,50,scenario%4]],['R',[6400,6400,scenario%4,1,127]],['D',0],['N',0,pose()+[multi[0]],ids(),definition()],['D',0],['C',0]]
  for step in range(56):
   key=rng.randrange(4);op=rng.choice(['A','D','P','M','U','R','R','R','F','C','N'])
   if op=='A':c=['A',key,[rng.randrange(40,61),rng.randrange(40,61),rng.randrange(4)]]
   elif op=='D':c=['D',key]
   elif op=='P':c=['P',key,pose(),ids()]
   elif op=='M':c=['M',key,None if multi[key]and rng.randrange(3)==0 else definition()]
   elif op=='U':c=['U']
   elif op=='R':c=['R',[rng.randrange(4900,7900),rng.randrange(4900,7900),rng.randrange(4),rng.choice([0,1,3,100,2147483647]),rng.choice([0,1,32,127,255])]]
   elif op=='F':c=['F',rng.randrange(1,9)]
   elif op=='C':c=['C',int(step%3==0)]
   else:
    multi[key]=rng.randrange(2);c=['N',key,pose()+[multi[key]],ids(),None if multi[key]and rng.randrange(3)==0 else definition()]
   commands.append(c)
  commands.append(['C',1]);cases.append(dict(commands=commands));requests+=['Z']+[wire(c)for c in commands]
 result=subprocess.run(['java','-cp',str(RUN),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True);rows=iter(result.stdout.splitlines())
 for case in cases:case['expected']=[json.loads(next(rows))for _ in case['commands']]
 assert next(rows,None)is None
 data=json.dumps(dict(sources=sources,scope='Complete unchanged AreaSound defaults/update, AreaSoundManager NPC add/remove/updateMulti/clear and collection+single-emitter redraw, Npc.getSound, original Node/LinkedList. Explicit resolved NPC metadata/pose/BAS services; no player/location inputs. Mixer/cache readiness id%11 adapters record exact handles/loop/volume events. Includes duplicated references, index replacement, null morph defaults, movement ID aliases, silent category transitions, attenuation and radius staleness. Full callback reentrancy/transport/platform playback remain separate.',cases=cases),separators=(',',':')).encode()+b'\n';path=ROOT/'fixtures/audio/npc.json'
 if check:
  if path.read_bytes()!=data:raise SystemExit('NPC ambient fixture differs')
 else:path.write_bytes(data)
 print('Original NPC ambient:',len(cases),'scenarios/',sum(len(c['commands'])for c in cases),'ordered commands')
 metadata(check)

def metadata(check):
 sources=v.generate(build_only=True);folder=RUN/'metadata';folder.mkdir(exist_ok=True)
 code='''import java.io.*;import java.util.*;public class NpcAmbientMetadataProbe{public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){NpcDefinitionOracle n=new NpcDefinitionOracle();n.decode(new Buffer(HexFormat.of().parseHex(line)));System.out.println(n.soundRadius);}}}'''
 java=folder/'NpcAmbientMetadataProbe.java';java.write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-cp',str(v.RUN),'-d',str(folder),str(java)],check=True)
 original=ROOT/'fixtures/players/npc-definitions.json';inputs=json.loads(original.read_text());rows=subprocess.run(['java','-cp',str(folder)+';'+str(v.RUN),'NpcAmbientMetadataProbe'],input='\n'.join(c['wire']for c in inputs['cases'])+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(rows)==len(inputs['cases'])
 cases=[dict(id=c['id'],radius=int(radius))for c,radius in zip(inputs['cases'],rows)];data=json.dumps(dict(sources=sources,input=dict(path=original.relative_to(ROOT).as_posix(),sha256=hashlib.sha256(original.read_bytes()).hexdigest()),scope='Original complete NpcType stream decoder soundRadius for every pinned raw NPC definition; raw streams and existing sound-ID observations are retained unchanged in the referenced prior fixture.',cases=cases),separators=(',',':')).encode()+b'\n';path=ROOT/'fixtures/audio/npc-metadata.json'
 if check:
  if path.read_bytes()!=data:raise SystemExit('NPC ambient metadata differs')
 else:path.write_bytes(data)
 audible=[c for c in inputs['cases']if any(c['expected'][13][i]!=-1 for i in [0,2,3])];ids={s for c in audible for s in c['expected'][13]if s>=0}
 print('Original NPC ambient radii:',len(cases),'definitions;',len(audible),'audible types;',len(ids),'distinct effects')
 spec=importlib.util.spec_from_file_location('synthesis',ROOT/'oracle/audio/synthesis.py');synth=importlib.util.module_from_spec(spec);spec.loader.exec_module(synth);synth.generate(check,ids=ids,name='npc-synthesis',bindings='All distinct real effects referenced by audible cached NpcType definitions, original SynthSound/instrument/filter/envelope synthesis before and after getStart at22050Hz. Missing groups explicit; no music or device loopback claim.')
 spec=importlib.util.spec_from_file_location('npc_assets',ROOT/'oracle/players/npc_assets.py');assets=importlib.util.module_from_spec(spec);spec.loader.exec_module(assets);assets.generate(ROOT,v,check,roots=[2207,2208],folder='npc-audio-assets',include_movement=True)

if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
