"""Original world SpotAnim packets, finite clocks and scene lifetime."""
import pathlib,re,json,hashlib,subprocess,random,importlib.util,struct,argparse

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/world-spot-oracle';O.mkdir(exist_ok=True);FIX=R/'fixtures/ui/world-spots.json'
    spec=importlib.util.spec_from_file_location('world_visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    sources={}
    def src(n):
     b=(v.SRC/(n+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[n+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    for n in ['Node','SecondaryNode','LinkedList','SpotAnimNode']:(O/(n+'.java')).write_text(src(n),encoding='utf-8')
    b=src('Buffer');(O/'Buffer.java').write_text('class Buffer {byte[]data;int offset;Buffer(byte[]b){data=b;}'+''.join(v.method(b,'public final int '+n+'(')for n in ['g1','g2','ig2','img4','ig2add'])+'}',encoding='utf-8')
    s=src('SpotAnim');fields=s[s.index('public final class SpotAnim'):s.index('public SpotAnim(')];fields=fields.replace('extends Entity','').replace('private ','public ')
    (O/'SpotAnim.java').write_text(fields+v.method(s,'public SpotAnim(')+v.method(s,'public final void advanceAnimation(')+'}',encoding='utf-8')
    p=src('Protocol');a=v.method(p,'if (opcode == ServerProt.SPOTANIM_SPECIFIC)');b=v.method(p,'if (opcode == ServerProt.SPOTANIM_ENTITY)');(O/'Protocol.java').write_text('class Protocol {static int sceneDelta,opcode;static Buffer inboundBuffer;static boolean receive(){'+a+'else '+b+'return true;}}',encoding='utf-8')
    s=src('ScriptRunner');(O/'ScriptRunner.java').write_text('class ScriptRunner {'+v.method(s,'public static void updateSceneSpotAnims()')+'}',encoding='utf-8')
    (O/'Support.java').write_text('''
    class ParticleSystem {}
    class SeqType {int[]frames,frameDelay;int priority;}
    class SeqTypeList {static SeqType seq;static SeqType get(int id){return seq;}}
    class SpotAnimType {int seqId;}
    class SpotAnimTypeList {static SpotAnimType type=new SpotAnimType();static SpotAnimType get(int id){return type;}}
    class ServerProt {static final int SPOTANIM_SPECIFIC=17,SPOTANIM_ENTITY=56;}
    class client {static int loop;}
    class Camera {static int originX,originY;}
    class Actor {int spotAnimDelayClock,spotAnimId=-1,spotAnimStart,spotAnimFrame,spotAnimY,spotAnimNextFrame,xFine,yFine;}
    class Player extends Actor {static int plane;}
    class Npc extends Actor {}
    class PlayerList {static int selfId=1;static Player self=new Player();static Player[]players=new Player[65536];}
    class NpcList {static Npc[]npcs=new Npc[65536];}
    class SoundPlayer {static void playSeqSound(int y,SeqType s,int x,boolean self,int frame){throw new AssertionError("world spot must not emit actor sequence sounds");}}
    class SceneGraph {static int currentChunkX,currentChunkY;static LinkedList spotanims=new LinkedList();static java.util.List<int[]>draws=new java.util.ArrayList<>(),heights=new java.util.ArrayList<>();static int getTileHeight(int p,int x,int y){heights.add(new int[]{p,x,y});return p*97+x/17-y/13-400;}static void add(int p,int x,int y,int z,int radius,SpotAnim s,int angle,long key,boolean f){draws.add(new int[]{p,x,y,z,radius,angle,(int)key,f?1:0});}}
    ''',encoding='utf-8')
    (O/'Probe.java').write_text('''
    import java.io.*;import java.util.*;
    class Probe {static String snapshot(){List<int[]>s=new ArrayList<>();for(SpotAnimNode n=(SpotAnimNode)SceneGraph.spotanims.head();n!=null;n=(SpotAnimNode)SceneGraph.spotanims.next()){SpotAnim e=n.spotAnim;s.add(new int[]{e.spotAnimTypeId,e.plane,e.posX,e.posY,e.z,e.endLoop,e.seqFrame,e.frameClock,e.finished?1:0});}return Arrays.deepToString(s.toArray());}static byte[]hex(String s){return HexFormat.of().parseHex(s);}public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split("\\t");if(p[0].equals("C")){SceneGraph.spotanims.clear();SceneGraph.heights.clear();SceneGraph.draws.clear();Protocol.opcode=Integer.parseInt(p[1]);Protocol.inboundBuffer=new Buffer(hex(p[2]));SceneGraph.currentChunkX=Integer.parseInt(p[3]);SceneGraph.currentChunkY=Integer.parseInt(p[4]);Camera.originX=Integer.parseInt(p[5]);Camera.originY=Integer.parseInt(p[6]);Player.plane=Integer.parseInt(p[7]);client.loop=Integer.parseInt(p[8]);SpotAnimTypeList.type.seqId=Integer.parseInt(p[9]);SeqType q=new SeqType();q.frameDelay=Arrays.stream(p[10].split(",")).mapToInt(Integer::parseInt).toArray();q.frames=new int[q.frameDelay.length];SeqTypeList.seq=q;Protocol.receive();}else{Player.plane=Integer.parseInt(p[1]);client.loop=Integer.parseInt(p[2]);Protocol.sceneDelta=Integer.parseInt(p[3]);SceneGraph.draws.clear();ScriptRunner.updateSceneSpotAnims();}System.out.println("["+snapshot()+","+Arrays.deepToString(SceneGraph.draws.toArray())+","+Arrays.deepToString(SceneGraph.heights.toArray())+"]");}}}
    ''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],check=True)
    rng=random.Random(530017);cases=[];commands=[]
    def wire17(coords,id,height,delay):return bytes([coords])+struct.pack('>H',id)+bytes([height])+struct.pack('>H',delay)
    def wire56(target,id,height,delay):return struct.pack('>H',delay)+struct.pack('<H',height)+bytes([(target>>8)&255,target&255,(target>>24)&255,(target>>16)&255])+bytes([(id+128)&255,(id>>8)&255])
    spot_cases=json.loads((R/'fixtures/players/spots/oracle.json').read_text());catalog={c['id']:c for c in spot_cases['decoded']}
    for i in range(640):
     op=17 if i<256 else 56;origin=[rng.choice([0,3152,3168,16300]),rng.choice([0,3168,3264,16300])];zone=[rng.choice([-8,-1,0,48,96,103,104]),rng.choice([-8,-1,0,48,96,103,104])];plane=rng.randrange(4);cycle=rng.choice([-2147483648,-1,0,1,99,2147483630]);delay=rng.choice([0,1,2,50,65535]);height=rng.choice([0,1,255]if op==17 else[0,1,255,256,65535]);id=rng.choice([0,92,184,65535]);seq=-1 if i%13==0 else 7;durations=[rng.choice([0,1,2,5,255])for _ in range(rng.randrange(1,9))]
     cached=None
     if i>=512:
      op=17;height&=255;id=[92,93,111,184,369,457,503,1000][i%8];spot_raw=bytes.fromhex(catalog[id]['raw']);seq=catalog[id]['expected']['definition']['sequence'];seq_raw=(R/'fixtures/players/spots'/('seq_'+str(seq)+'.bin')).read_bytes();prefix=2 if seq_raw[0]==8 else 0;assert seq_raw[prefix]==1;n=struct.unpack_from('>H',seq_raw,prefix+1)[0];durations=list(struct.unpack_from('>'+str(n)+'H',seq_raw,prefix+3));zone=[48,48];delay=i%5;cycle=500;cached=dict(spotRaw=spot_raw.hex(),sequenceRaw=seq_raw.hex(),spotSha256=hashlib.sha256(spot_raw).hexdigest(),sequenceSha256=hashlib.sha256(seq_raw).hexdigest())
     if op==17:raw=wire17(rng.randrange(256),id,height,delay)
     else:
      tile=[(origin[0]+rng.choice([-1,0,1,103,104]))&16383,(origin[1]+rng.choice([-1,0,1,103,104]))&16383];target=(rng.choice([0x40000000,0x80000000,0xc0000000,0x20000000,0x10000000])|(plane<<28)|(tile[0]<<14)|tile[1])&0xffffffff;raw=wire56(target,id,height,delay)
     c=dict(opcode=op,payload=raw.hex(),origin=origin,zone=zone,plane=plane,cycle=cycle,sequence=seq,delays=durations,updates=[],cached=cached);commands.append('\t'.join(map(str,['C',op,raw.hex(),*zone,*origin,plane,cycle,seq,','.join(map(str,durations))])))
     for k in range(7):
      cyc=(cycle+delay+k*2+2**31)%2**32-2**31;u=[plane if k<6 else(plane+1)%4,cyc,rng.choice([0,1,2,5,100])];c['updates'].append(u);commands.append('\t'.join(map(str,['U',*u])))
     cases.append(c)
    res=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True,check=True);rows=[json.loads(s)for s in res.stdout.splitlines()];assert len(rows)==len(commands)
    for c in cases:c['expected']=[rows.pop(0)for _ in range(8)]
    result=dict(scope='Unchanged original SpotAnim constructor/advanceAnimation, full ScriptRunner.updateSceneSpotAnims, original17 and56 packet branches and Buffer reads. Definition/sequence/terrain/draw services are explicit recording adapters; actor56 branches have absent actors and are not claimed. No sound is emitted by original world effect lifecycle.',sources=sources,cases=cases)
    if check:assert json.loads(FIX.read_text())==result,'world spot oracle fixture differs'
    else:FIX.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print('PASS',len(cases),'packet/lifetime cases',len(commands),'snapshots')

    preview=dict(scope='Explicit offline original-format17/56 world effects; no live spell/combat/gameplay claim. Cached assets unchanged.',start=[3207,3214],sceneOrigin=[3152,3168],frames=[])
    for tick,op,id,x,y,height,delay in [(1,17,184,3208,3216,0,0),(35,56,92,3206,3216,128,5),(70,17,369,3208,3216,0,6),(110,56,503,3205,3216,0,0)]:
     if op==17:
      preview['frames'].append(dict(tick=tick,opcode=26,payload=[(-(x-preview['sceneOrigin'][0]))&255,y-preview['sceneOrigin'][1]]));raw=wire17(0,id,height,delay)
     else:raw=wire56(0x40000000|(x<<14)|y,id,height,delay)
     preview['frames'].append(dict(tick=tick,opcode=op,payload=list(raw)))
    p=R/'fixtures/players/world-spots-replay.json'
    if check:assert json.loads(p.read_text())==preview
    else:p.write_text(json.dumps(preview,indent=2)+'\n',encoding='utf-8')

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');args=parser.parse_args();generate(args.check)
