"""Original NPC32 packet readers, requests and complete default-BAS NPC phases."""
import hashlib,json,random,re,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/npcs';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    protocol=source('Protocol');path=source('PathingEntity');npc=source('NpcList');source('Npc');seq=source('SeqType');bas=source('BasType');buffer=source('Buffer');packet=source('Packet')
    def fields(text):return '\n'.join(re.findall(r'(?:public|private|protected) (?:static )?(?:final )?(?:int|boolean|byte)(?:\[\])* \w+(?:\s*=\s*[^;]+)?;',text)).replace('private ','public ').replace('protected ','public ')
    (run/'SeqType.java').write_text(seq[seq.index('public final class SeqType'):seq.index('public void decode(')]+method(seq,'public void decode(')+method(seq,'private void decode(')+method(seq,'public void postDecode(')+'}',encoding='utf-8')
    bm=''.join(method(buffer,s)for s in ['public final int g1(', 'public final int g2(', 'public final int g3(', 'public final int g1neg(', 'public final int g1sub(', 'public final int g2add(', 'public final int ig2(', 'public final int ig4('])
    bm+=''.join(method(packet,s)for s in ['public final void accessBits(', 'public final void accessBytes(', 'public final int gBits(', 'public final int availableBits('])
    code=r'''import java.io.*;import java.util.*;
class GlobalConfig{static boolean USE_TWEENING=true;}
class JagString{byte[]data;JagString(byte[]v){data=v;}}
class Buffer{byte[]data;int offset,bitOffset;static final int[]BIT_MASKS=new int[33];static{for(int i=0;i<32;i++)BIT_MASKS[i]=(1<<i)-1;BIT_MASKS[32]=-1;}Buffer(byte[]b){data=b;}JagString gjstr(){int start=offset;while(data[offset++]!=0){}return new JagString(Arrays.copyOfRange(data,start,offset-1));} BUFFER }
class BasType{ BAS }
class SlotAnimation{int seqId,currentFrame,nextFrame,delayClock,startDelay,replayCount;}
class PathingEntity{ FIELDS SlotAnimation[]slotAnimations=new SlotAnimation[12];JagString chatMessage;int getSize(){return size;}BasType getBasType(){return Probe.bas;}void addHit(int kind,int cycle,int damage){Probe.events.add("[\"hit\","+Probe.index((Npc)this)+","+damage+","+kind+","+cycle+"]");} PATH }
class Player extends PathingEntity{static int plane;}
class PlayerList{static Player self;static int selfId=17;static Player[]players=new Player[2048];}
class NpcType{int id,size,bastypeid,rotationspeed;boolean rotationflag;NpcType(int n){id=n;size=1+n%3;bastypeid=1000+n;rotationspeed=new int[]{0,16,32,64}[n%4];rotationflag=n%2==0;}boolean hasAreaSound(){return id%3==1;}}
class NpcTypeList{static NpcType get(int id){return new NpcType(id);}}
class Npc extends PathingEntity{NpcType type;void setNpcType(NpcType t){type=t;Probe.events.add("[\"type\","+Probe.index(this)+","+(t==null?"null":t.id)+"]");}}
class AreaSoundManager{static void remove(Npc n){Probe.events.add("[\"remove\","+Probe.index(n)+"]");}static void add(int y,Object a,int z,Npc n,int x,int plane,Object b){Probe.events.add("[\"add\","+Probe.index(n)+","+x+","+y+"]");}}
class PluginRepository{static void OnKillingBlowNPC(int id,int x,int y){Probe.events.add("[\"kill\","+id+","+x+","+y+"]");}}
class Camera{static int originX,originY;}class client{static int loop;}
class GlRenderer{static boolean enabled=false;}class FogManager{static void setInstantFade(){}}
class SeqTypeList{static Map<Integer,SeqType>types=new HashMap<>();static SeqType get(int id){if(id==-1){SeqType s=new SeqType();s.postDecode();return s;}return types.get(id);}}
class SpotAnimType{int seqId;SpotAnimType(int id){seqId=id;}}class SpotAnimTypeList{static SpotAnimType get(int id){return new SpotAnimType(id==17?0:id==18?4:-1);}}
class SoundPlayer{static void playSeqSound(int y,SeqType s,int x,boolean self,int frame){Probe.events.add("[\"sound\","+s.id+","+frame+"]");}}
class NpcList{static Npc[]npcs=new Npc[32768];static int[]ids=new int[32768];static int size; PHASES }
class Protocol{static int length,extendedCount,removedCount;static int[]extendedIds=new int[32768],removedIds=new int[32768];static Buffer inboundBuffer; READERS }
public class Probe{
static List<String>events=new ArrayList<>();static BasType bas;
static int index(Npc n){for(int i=0;i<32768;i++)if(NpcList.npcs[i]==n)return i;throw new RuntimeException();}
static String track(int id,int f,int n,int c,int d,int r){return Arrays.toString(new int[]{id,f,n,c,d,r});}
static String snapshot(){List<String>actors=new ArrayList<>();for(int index=0;index<32768;index++){Npc n=NpcList.npcs[index];if(n==null)continue;List<String>q=new ArrayList<>();for(int i=n.movementQueueSize-1;i>=0;i--)q.add(Arrays.toString(new int[]{n.movementQueueX[i],n.movementQueueY[i],n.movementQueueSpeed[i]}));List<String>slots=new ArrayList<>();for(SlotAnimation t:n.slotAnimations)slots.add(t==null?"null":track(t.seqId,t.currentFrame,t.nextFrame,t.delayClock,t.startDelay,t.replayCount));actors.add("["+index+","+n.type.id+","+n.lastSeenLoop+","+n.getSize()+","+n.basTypeId+","+n.turnSpeed+","+n.type.rotationflag+","+Arrays.toString(new int[]{n.xFine,n.yFine})+","+Arrays.toString(new int[]{n.movementQueueX[0],n.movementQueueY[0],n.movementQueueSpeed[0]})+","+q+","+n.movementCatchupTicks+","+Arrays.toString(new int[]{n.currentAngle,n.targetAngle,n.standingTurnCounter})+","+Arrays.toString(new int[]{n.faceEntity,n.faceX,n.faceY})+","+n.seqMovementSteps+","+track(n.movementSeqId,n.movementSeqFrame,n.movementSeqNextFrame,n.movementSeqDelayClock,0,0)+","+track(n.seqId,n.seqFrame,n.seqNextFrame,n.seqDelayClock,n.seqDelay,n.seqReplayCount)+","+track(n.spotAnimId,n.spotAnimFrame,n.spotAnimNextFrame,n.spotAnimDelayClock,0,0)+","+Arrays.toString(new int[]{n.spotAnimStart,n.spotAnimY})+","+slots+",\""+(n.chatMessage==null?"":HexFormat.of().formatHex(n.chatMessage.data))+"\","+n.chatLoops+","+Arrays.toString(new int[]{n.hitpointsBar,n.hitpointsBarVisibleUntil})+"]");}return "{\"actors\":"+actors+",\"locals\":"+Arrays.toString(Arrays.copyOf(NpcList.ids,NpcList.size))+",\"removed\":"+Arrays.toString(Arrays.copyOf(Protocol.removedIds,Protocol.removedCount))+",\"events\":"+events+"}";}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]a=line.split(";");switch(a[0]){
case"R":Arrays.fill(NpcList.npcs,null);NpcList.size=0;PlayerList.self=new Player();PlayerList.self.movementQueueX[0]=Integer.parseInt(a[1]);PlayerList.self.movementQueueY[0]=Integer.parseInt(a[2]);PlayerList.self.xFine=Integer.parseInt(a[1])*128+64;PlayerList.self.yFine=Integer.parseInt(a[2])*128+64;PlayerList.players[2047]=PlayerList.self;Camera.originX=Integer.parseInt(a[3]);Camera.originY=Integer.parseInt(a[4]);bas=new BasType();bas.idleAnimationId=0;bas.walkAnimation=1;bas.runAnimationId=2;bas.standingCCWTurn=3;bas.standingCWTurn=4;bas.walkFullTurnAnimationId=5;bas.walkCCWTurnAnimationId=6;bas.walkCWTurnAnimationId=7;events.clear();break;
case"S":{SeqType s=new SeqType();s.id=Integer.parseInt(a[1]);s.decode(new Buffer(HexFormat.of().parseHex(a[2])));s.postDecode();SeqTypeList.types.put(s.id,s);break;}
case"P":client.loop=Integer.parseInt(a[1]);Protocol.inboundBuffer=new Buffer(HexFormat.of().parseHex(a[2]));Protocol.length=Protocol.inboundBuffer.data.length;Protocol.readNpcPacket();System.out.println(snapshot());events.clear();break;
case"T":client.loop=Integer.parseInt(a[1]);for(int i=0;i<NpcList.size;i++){Npc n=NpcList.npcs[NpcList.ids[i]];NpcList.processEntity(n.type.size,n);}System.out.println(snapshot());events.clear();break;
}}}}
'''.replace('BUFFER',bm).replace(' BAS ',fields(bas[:bas.index('public final void postDecode(')])).replace(' FIELDS ',fields(path[:path.index('public final BasType getBasType(')])).replace(' PATH ',''.join(method(path,s)for s in ['public final void move(', 'public final void teleport(', 'public final void resetMovementQueue(', 'public final void setSize('])).replace(' PHASES ',''.join(method(npc,s)for s in ['public static void processEntity(', 'public static void lerpToForceMoveStart(', 'public static void lerpToForceMoveDest(', 'public static void processMovement(', 'public static void processFacing(', 'public static void processAnimations('])).replace(' READERS ',''.join(method(protocol,s)for s in ['public static void readNpcPacket(', 'public static void clearAreaNPCs(', 'public static void loadAreaNPCs(', 'public static void npcCombat(', 'public static void animateNpc(', 'public static void updateNpcSlotAnimations(']))
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java'),str(run/'SeqType.java')],check=True)
    from generate import Bits
    rng=random.Random(53032);cases=[];inputs=[]
    def u2(n):return(n&65535).to_bytes(2,'big')
    def mask(flags):
        data=bytearray([flags&255]);
        if flags&8:data.append(flags>>8)
        if flags&64:data.extend([rng.randrange(256),(-rng.randrange(256))&255,(128-rng.randrange(256))&255])
        if flags&2:data.extend([(-rng.randrange(256))&255,(128-rng.randrange(256))&255])
        if flags&16:data.extend(u2(rng.choice([-1,*range(8,14)]))+bytes([rng.randrange(4)]))
        if flags&4:
            n=rng.choice([65535,32785,0,7,32000]);data.extend(u2((n&0xff00)|((n+128)&255)))
        if flags&128:
            n=rng.choice([-1,17,18,19]);data.extend(u2((n&0xff00)|((n+128)&255))+(rng.randrange(100)<<16|rng.randrange(4)).to_bytes(4,'little'))
        if flags&1:data.extend(rng.randrange(16).to_bytes(2,'little'))
        if flags&32:data.extend(bytes([rng.randrange(1,256)for _ in range(rng.randrange(15))])+b'\0')
        if flags&256:
            count=rng.randrange(4);data.append((-count)&255)
            for _ in range(count):data.extend(rng.choice([65535,*range(14)]).to_bytes(2,'little')+bytes([(128-rng.randrange(4))&255])+u2(rng.randrange(65536)))
        if flags&512:
            n=rng.randrange(5000,8000);data.extend(u2((n&0xff00)|((n+128)&255))+u2(rng.randrange(5000,8000)))
        return data
    for scenario in range(64):
        start=[rng.randrange(25,75),rng.randrange(25,75)];origin=[3152,3168]if scenario%2 else [0,0];inputs.append(';'.join(map(str,['R',*start,*origin])));sequences=[]
        for id in range(14):
            n=rng.randrange(2,7);wire=bytes([1])+u2(n)+b''.join(u2(rng.randrange(1,7))for _ in range(n))+b''.join(u2(i)for i in range(n))+b'\0\1'*n+bytes([2])+u2(n)+bytes([5,id%4,8,3,9,(id//3)%3,10,id%3,11,1,0]);sequences.append(wire.hex());inputs.append(f'S;{id};{wire.hex()}')
        ids=[];packets=[]
        for tick in range(24):
            cycle=100+tick//2;bits=Bits();count=rng.randrange(len(ids)+1);bits.put(8,count);new=[];extended=[]
            for index in ids[:count]:
                changed=rng.randrange(2);bits.put(1,changed)
                if not changed:new.append(index);continue
                kind=rng.randrange(4);bits.put(2,kind)
                if kind==3:continue
                new.append(index)
                if kind==0:extended.append(index);continue
                if kind==2:double=rng.randrange(2);bits.put(1,double)
                bits.put(3,rng.randrange(8))
                if kind==2 and double:bits.put(3,rng.randrange(8))
                has=rng.randrange(2);bits.put(1,has)
                if has:extended.append(index)
            candidates=[i for i in ids if i not in new]+rng.sample([*range(24),32000,32766],8)
            for index in candidates[:rng.randrange(5)]:
                if index in new:continue
                new.append(index);bits.put(15,index);bits.put(1,rng.randrange(2));bits.put(3,rng.randrange(8));has=rng.randrange(2);bits.put(1,has)
                if has:extended.append(index)
                bits.put(5,rng.randrange(32));bits.put(14,rng.randrange(16));bits.put(5,rng.randrange(32))
            if extended:bits.put(15,32767)
            data=bytearray(bits.bytes())
            for _ in extended:
                flags=(rng.randrange(1024)&~8) or 16
                if flags>=256:flags|=8
                data.extend(mask(flags))
            wire=bytes(data).hex();packets.append(dict(cycle=cycle,wire=wire));inputs.extend([f'P;{cycle};{wire}',f'T;{cycle+1}']);ids=new
        cases.append(dict(start=start,origin=origin,sequences=sequences,packets=packets))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True)
    if result.returncode:raise SystemExit(result.stderr[-3000:])
    lines=result.stdout.splitlines();assert len(lines)==3072
    it=iter(lines)
    for case in cases:
        for p in case['packets']:p['expected']=json.loads(next(it));p['tick_expected']=json.loads(next(it))
    content=json.dumps(dict(sources=hashes,bindings='Unchanged complete Protocol NPC32 readers/requests and NpcList entity phases plus PathingEntity queue methods. Synthetic size/BAS/rotation/area-sound type catalog and sequence/spot catalog; hit/ambient/particle/plugin callbacks are explicit observations. Ordinary default-acceleration BAS, all tested turn speeds. No NPC AI/content claim.',cases=cases),separators=(',',':')).encode()+b'\n';path=root/'fixtures/players/npcs.json'
    if check:
        if path.read_bytes()!=content:raise SystemExit('NPC corpus differs')
    else:path.write_bytes(content)
    print('Original complete NPC32 readers/entity phases: 64 scenarios / 1536 packets / 1536 ticks')
