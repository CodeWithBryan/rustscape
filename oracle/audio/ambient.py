"""Run-only original single-location area sound lifecycle and attenuation probe."""
import hashlib,importlib.util,json,pathlib,random,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
run=ROOT/'runs/audio-ambient-oracle';run.mkdir(parents=True,exist_ok=True);sources={}
def source(name):
 raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
(run/'Node.java').write_text(source('Node'),encoding='utf-8')
(run/'AreaSound.java').write_text(source('AreaSound'),encoding='utf-8')
manager=source('AreaSoundManager')
(run/'AreaSoundManager.java').write_text('class AreaSoundManager{static LinkedList locSounds=new LinkedList(),npcSounds=new LinkedList();static HashTable playerSounds=new HashTable();'+v.method(manager,'public static void add(').replace('Math.random()','RandomTape.next()')+v.method(manager,'public static void redraw(int x,') .replace('Math.random()','RandomTape.next()')+'}',encoding='utf-8')
# clean() removes annotation text before method lookup.
p=run/'AreaSoundManager.java'
if not p.exists():raise AssertionError()
support='''import java.io.*;import java.util.*;
class LinkedList{void addTail(AreaSound s){Probe.current=s;}}class HashTable{void put(AreaSound s,long k){}}
class LocType{int id,width,length,bgsound=-1,bgsoundrange,bgsoundmin,bgsoundmax;int[]bgsounds,multiLocs;LocType next;LocType getMultiLoc(){return next;}}
class NpcType{int[]multiNpcs;int soundRadius,size;NpcType getMultiNpc(){return this;}}
class Npc{NpcType type;static int getSound(Npc n){return 0;}}
class Username{long encode37(){return 0;}}class Player{int soundRadius;Username username=new Username();int getSize(){return 1;}static int getSound(Player p){return 0;}}
class Preferences{static int ambientSoundsVolume;}
class RandomTape{static double[]values;static int at;static double next(){return values[at++];}}
class PcmSound{int id;PcmSound(int n){id=n;}PcmSound resample(Object o){return this;}}
class SynthSound{int id;SynthSound(int n){id=n;}static SynthSound create(Object o,int id,int f){return id%11==0?null:new SynthSound(id);}PcmSound toPcmSound(){return new PcmSound(id);}}
class SoundPcmStream{int serial,id,volume,loops;boolean linked;static int next;static SoundPcmStream create(PcmSound s,int volume){SoundPcmStream a=new SoundPcmStream();a.id=s.id;a.volume=volume;a.serial=++next;return a;}void setLoops(int n){loops=n;}void setVolume(int n){volume=n;Probe.events.add("[1,"+serial+","+n+"]");}boolean isLinked(){return linked;}}
class Mixer{void addSubStream(SoundPcmStream s){s.linked=true;Probe.events.add("[0,"+s.serial+","+s.id+","+s.volume+","+s.loops+"]");}void removeSubStream(SoundPcmStream s){s.linked=false;Probe.events.add("[2,"+s.serial+"]");}}
class client{static Mixer soundStream=new Mixer();static Object js5Archive4,resampler;}
public class Probe{static AreaSound current;static LocType base;static List<String>events=new ArrayList<>();
 static int[]ints(String s){return s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
 static LocType definition(String a,String b){int[]n=ints(a);LocType d=new LocType();d.width=n[0];d.length=n[1];d.bgsound=n[2];d.bgsoundrange=n[3];d.bgsoundmin=n[4];d.bgsoundmax=n[5];d.bgsounds=b.equals("null")?null:ints(b);return d;}
 static String stream(SoundPcmStream s){return s==null?"null":Arrays.toString(new int[]{s.serial,s.id,s.volume,s.loops,s.linked?1:0});}
 static String snapshot(){AreaSound a=current;return "{\\\"fields\\\":"+Arrays.toString(new int[]{a.minXFine,a.minYFine,a.maxXFine,a.maxYFine,a.level,a.sound,a.radius,a.minInterval,a.maxInterval,a.remainingLoops,a.multiLocOrNpc?1:0})+",\\\"sounds\\\":"+Arrays.toString(a.sounds)+",\\\"primary\\\":"+stream(a.primaryStream)+",\\\"secondary\\\":"+stream(a.secondaryStream)+",\\\"randomCalls\\\":"+RandomTape.at+",\\\"events\\\":"+events+"}";}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);switch(p[0]){
 case"R":{int[]n=ints(p[1]);SoundPcmStream.next=0;events.clear();base=definition(p[2],p[3]);if(n[4]!=0){base.multiLocs=new int[]{1};base.next=base;}RandomTape.values=new double[]{Double.parseDouble(p[4])};RandomTape.at=0;AreaSoundManager.add(n[1],base,n[3],null,n[0],n[2],null);System.out.println(snapshot());events.clear();break;}
 case"U":{base.next=p[1].equals("null")?null:definition(p[1],p[2]);current.update();break;}
 case"K":{int n=Integer.parseInt(p[1]);if((n&1)!=0&&current.primaryStream!=null)current.primaryStream.linked=false;if((n&2)!=0&&current.secondaryStream!=null)current.secondaryStream.linked=false;break;}
 case"T":{int[]n=ints(p[1]);Preferences.ambientSoundsVolume=n[4];RandomTape.values=Arrays.stream(p[2].split(",")).mapToDouble(Double::parseDouble).toArray();RandomTape.at=0;AreaSoundManager.redraw(n[0],current,n[2],n[3],n[1]);System.out.println(snapshot());events.clear();break;}
 default:throw new IllegalArgumentException(p[0]);}}}}
'''.replace('\\\\\\"','\\"')
(run/'Probe.java').write_text(support,encoding='utf-8')
subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
rng=random.Random(5304);cases=[];requests=[]
def definition():return dict(fields=[rng.randrange(1,8),rng.randrange(1,8),rng.choice([-1,0,11,127,223,387]),rng.randrange(0,9),rng.randrange(0,50),rng.randrange(0,100)],sounds=rng.choice([None,[1],[11],[127,223,387]]))
def text(d):return ','.join(map(str,d['fields']))+';'+('null'if d['sounds']is None else','.join(map(str,d['sounds'])))
for scenario in range(128):
 args=[rng.randrange(104),rng.randrange(104),rng.randrange(4),rng.randrange(4),scenario%2];d=definition();initial=rng.random();steps=[];requests.append('R;'+','.join(map(str,args))+';'+text(d)+';'+str(initial))
 for tick in range(32):
  step={}
  if args[4]and tick%5==0:
   step['update']=None if tick%10==0 else definition();requests.append('U;null;'if step['update']is None else'U;'+text(step['update']))
  if tick%7==0:step['finish']=rng.randrange(4);requests.append('K;'+str(step['finish']))
  step['args']=[args[0]*128+rng.randrange(-1100,1401),args[1]*128+rng.randrange(-1100,1401),rng.choice([args[2],args[2],rng.randrange(4)]),rng.choice([0,1,2,3,20,100,2147483647]),rng.choice([0,1,32,127,255])];step['random']=[rng.random(),rng.random()];steps.append(step);requests.append('T;'+','.join(map(str,step['args']))+';'+','.join(map(str,step['random'])))
 cases.append(dict(args=args,definition=d,initialRandom=initial,steps=steps))
out=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(out)==128*33
it=iter(out)
for c in cases:
 c['initialExpected']=json.loads(next(it))
 for s in c['steps']:s['expected']=json.loads(next(it))
data=json.dumps(dict(sources=sources,bindings='Original AreaSound defaults/update, unchanged complete AreaSoundManager.add and single-emitter redraw methods. Location-only inputs; resolved morph metadata explicit; Math.random uses controlled tape; synth readiness id%11, PCM/mixer records exact stream IDs/linkage/loop/volume events and finish inputs. Full collection ordering, NPC/player services and sample playback are separate.',cases=cases),separators=(',',':'))+'\n'
path=ROOT/'fixtures/audio/ambient.json'
if '--check'in sys.argv:
 if path.read_bytes()!=data.encode():raise SystemExit('Original ambient corpus differs')
else:path.write_bytes(data.encode())
print('Original location ambient probe:',128,'constructions +',128*32,'ticks')
