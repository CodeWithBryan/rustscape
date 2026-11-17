"""Verbatim Player requests, SeqType decoder and NpcList animation clocks."""
import hashlib,json,random,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/animation';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    player=source('Player');npc=source('NpcList');seq=source('SeqType');buffer=source('Buffer');protocol=source('Protocol')
    fields=seq[seq.index('public final class SeqType'):seq.index('public void decode(')].replace('private ','public ')
    (run/'SeqType.java').write_text(fields+method(seq,'public void decode(')+method(seq,'private void decode(')+method(seq,'public void postDecode(')+'}',encoding='utf-8')
    spot=method(method(protocol,'public static void readExtendedPlayerInfo(int flags'),'if ((flags & 0x100) != 0)')
    code=r'''import java.io.*;import java.util.*;
class GlobalConfig{static boolean USE_TWEENING=true;}
class Buffer{byte[]data;int offset;int a,b;Buffer(byte[]b){data=b;}int ig2(){return a;}int mg4(){return b;} BUFFER }
class SlotAnimation{int seqId,currentFrame,nextFrame,delayClock,startDelay,replayCount;}
class PathingEntity{int xFine=6500,yFine=6400,movementSeqId=-1,movementSeqFrame,movementSeqNextFrame=-1,movementSeqDelayClock,seqId=-1,seqFrame,seqNextFrame=-1,seqDelayClock,seqDelay,seqReplayCount,seqMovementSteps,movementQueueSize,spotAnimId=-1,spotAnimFrame,spotAnimNextFrame,spotAnimDelayClock,spotAnimStart,spotAnimY,forceMoveCyclesToStart,forceMoveCyclesToDest;boolean seqStretches;SlotAnimation[]slotAnimations=new SlotAnimation[12];}
class Player extends PathingEntity{ REQUESTS }
class PlayerList{static Player self;}
class client{static int loop;}
class SeqTypeList{static Map<Integer,SeqType>types=new HashMap<>();static SeqType get(int id){if(id==-1){SeqType s=new SeqType();s.id=-1;s.postDecode();return s;}return types.get(id);}}
class SpotAnimType{int seqId;SpotAnimType(int id){seqId=id;}}class SpotAnimTypeList{static Map<Integer,Integer>types=new HashMap<>();static SpotAnimType get(int id){return new SpotAnimType(types.get(id));}}
class SoundPlayer{static List<String>events=new ArrayList<>();static void playSeqSound(int y,SeqType s,int x,boolean self,int frame){events.add("["+s.id+","+frame+"]");}}
class NpcList{ CLOCKS }
public class Probe{
static Buffer inboundBuffer;
static void spot(Player player,int id,int packed){int flags=256,int1,int2,savedOffset;inboundBuffer=new Buffer(new byte[0]);inboundBuffer.a=id==-1?65535:id;inboundBuffer.b=packed; SPOT }
static String track(int id,int frame,int next,int clock,int delay,int repeats){return Arrays.toString(new int[]{id,frame,next,clock,delay,repeats});}
static String metadata(){List<String>rows=new ArrayList<>();for(int id=0;id<12;id++){SeqType s=SeqTypeList.get(id);List<String>sounds=new ArrayList<>();if(s.soundeffect!=null)for(int[]v:s.soundeffect)sounds.add(v==null?"[]":Arrays.toString(v));rows.add("["+Arrays.toString(s.frames)+","+Arrays.toString(s.frameDelay)+","+s.replayoff+","+s.priority+","+s.replaycount+","+s.looptype+","+s.movetype+","+s.exactmove+","+s.mainhand+","+s.offhand+","+(s.frameset==null?"[]":Arrays.toString(s.frameset))+","+Arrays.toString(s.framegroup)+","+s.stretches+","+s.hasModelTransforms+","+s.tween+","+s.updateShadows+","+sounds+"]");}return rows.toString();}
static String snapshot(Player p){List<String>s=new ArrayList<>();for(SlotAnimation t:p.slotAnimations)s.add(t==null?"null":track(t.seqId,t.currentFrame,t.nextFrame,t.delayClock,t.startDelay,t.replayCount));return "{\"movement\":"+track(p.movementSeqId,p.movementSeqFrame,p.movementSeqNextFrame,p.movementSeqDelayClock,0,0)+",\"action\":"+track(p.seqId,p.seqFrame,p.seqNextFrame,p.seqDelayClock,p.seqDelay,p.seqReplayCount)+",\"spot\":"+track(p.spotAnimId,p.spotAnimFrame,p.spotAnimNextFrame,p.spotAnimDelayClock,0,0)+",\"spot_start\":"+p.spotAnimStart+",\"spot_height\":"+p.spotAnimY+",\"steps\":"+p.seqMovementSteps+",\"stretches\":"+p.seqStretches+",\"slots\":"+s+",\"sounds\":"+SoundPlayer.events+"}";}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;Player p=null;while((line=r.readLine())!=null){String[]a=line.split(";");switch(a[0]){
case"R":p=new Player();PlayerList.self=p;SeqTypeList.types.clear();SpotAnimTypeList.types.clear();SoundPlayer.events.clear();break;
case"S":{SeqType s=new SeqType();s.id=Integer.parseInt(a[1]);s.decode(new Buffer(HexFormat.of().parseHex(a[2])));s.postDecode();SeqTypeList.types.put(s.id,s);break;}
case"P":SpotAnimTypeList.types.put(Integer.parseInt(a[1]),Integer.parseInt(a[2]));break;
case"D":System.out.println(metadata());break;
case"A":p.movementQueueSize=Integer.parseInt(a[3]);Player.animate(Integer.parseInt(a[2]),Integer.parseInt(a[1]),p);break;
case"L":Player.applySlotAnimations(new int[]{Integer.parseInt(a[2])},new int[]{Integer.parseInt(a[1])},p,new int[]{Integer.parseInt(a[3])});break;
case"G":client.loop=Integer.parseInt(a[3]);spot(p,Integer.parseInt(a[1]),Integer.parseInt(a[2]));break;
case"M":p.movementSeqId=Integer.parseInt(a[1]);break;
case"Q":p.seqMovementSteps=Integer.parseInt(a[1]);p.forceMoveCyclesToStart=Integer.parseInt(a[2]);p.forceMoveCyclesToDest=Integer.parseInt(a[3]);break;
case"T":client.loop=Integer.parseInt(a[1]);NpcList.processAnimations(p);System.out.println(snapshot(p));SoundPlayer.events.clear();break;
}}}}
'''.replace('BUFFER',''.join(method(buffer,s)for s in ['public final int g1(', 'public final int g2(', 'public final int g3('])).replace('REQUESTS',method(player,'public static void animate(')+method(player,'public static void applySlotAnimations(')).replace('CLOCKS',method(npc,'public static void processAnimations(')).replace('SPOT',spot)
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java'),str(run/'SeqType.java')],check=True)
    def u2(n):return (n&65535).to_bytes(2,'big')
    def signed(n):return (n+2**31)%2**32-2**31
    rng=random.Random(530225);cases=[];inputs=[]
    for case_index in range(64):
        inputs.append('R');sequences=[]
        for id in range(12):
            n=rng.randrange(1,9);wire=bytes([1])+u2(n)+b''.join(u2(rng.randrange(1,6))for _ in range(n))+b''.join(u2(i)for i in range(n))+b'\0\1'*n
            if rng.randrange(2):wire+=bytes([2])+u2(rng.randrange(n+2))
            if rng.randrange(2):wire+=bytes([3,3,0,5,17])
            wire+=bytes([5,rng.randrange(8),8,rng.randrange(1,5),11,rng.randrange(3)])
            for op in [9,10]:
                if rng.randrange(2):wire+=bytes([op,rng.randrange(3)])
            for op in [6,7]:
                if rng.randrange(2):wire+=bytes([op])+u2(rng.randrange(65536))
            if rng.randrange(2):
                count=rng.randrange(5);wire+=bytes([12,count])+b''.join(u2(rng.randrange(65536))for _ in range(count*2))
            if rng.randrange(2):
                count=rng.randrange(5);wire+=bytes([13])+u2(count)
                for _ in range(count):
                    m=rng.randrange(4);wire+=bytes([m])
                    if m:wire+=rng.randrange(2**24).to_bytes(3,'big')+b''.join(u2(rng.randrange(65536))for _ in range(m-1))
            for op in [14,15,16]:
                if rng.randrange(2):wire+=bytes([op])
            if rng.randrange(2):wire+=bytes([4])
            wire+=bytes([0]);sequences.append(wire.hex());inputs.append(f'S;{id};{wire.hex()}')
        spots={17:0,18:4,19:-1 if case_index%2 else 8};inputs.extend(f'P;{i};{s}'for i,s in spots.items());inputs.append('D');commands=[]
        base=100 if case_index<32 else 2147483600
        for tick in range(128):
            cycle=signed(base+tick)
            if tick%5==0:
                command=['A',rng.randrange(-1,12),rng.randrange(5),rng.randrange(10)];commands.append(command);inputs.append(';'.join(map(str,command)))
            if tick%7==0:
                command=['L',rng.randrange(-1,12),rng.randrange(4),rng.randrange(65536)];commands.append(command);inputs.append(';'.join(map(str,command)))
            if tick%11==0:
                command=['G',rng.choice([-1,17,18,19]),signed((rng.randrange(-400,401)<<16)+rng.randrange(6)),cycle];commands.append(command);inputs.append(';'.join(map(str,command)))
            if tick%9==0:
                command=['M',rng.randrange(-1,12)];commands.append(command);inputs.append(';'.join(map(str,command)))
            if tick%13==0:
                command=['Q',rng.randrange(4),signed(cycle+rng.randrange(-2,3)),signed(cycle+rng.randrange(-2,3))];commands.append(command);inputs.append(';'.join(map(str,command)))
            commands.append(['T',cycle]);inputs.append(f'T;{cycle}')
        cases.append(dict(sequences=sequences,spots=spots,commands=commands))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==8192+64
    it=iter(result)
    for case in cases:case['metadata']=json.loads(next(it));case['expected']=[json.loads(next(it))for _ in range(128)]
    path=root/'fixtures/players/animation.json';content=json.dumps(dict(sources=hashes,bindings='Unchanged SeqType decode/postDecode, Player.animate/applySlotAnimations, Protocol spot mask block, NpcList.processAnimations. Typed spot field reader, sound callback observations, synthetic sequence/spot catalog and movement/force-step inputs are explicit. No model/mixer or full movement simulation claim.',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Actor animation corpus differs')
    else:path.write_bytes(content)
    print(f'Original actor request/animation clocks: {len(cases)} scenarios / 8192 ticks / 768 full sequence metadata records')
