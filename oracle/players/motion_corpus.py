"""Unchanged complete NpcList entity phases with default-player BAS bindings."""
import hashlib,json,random,re,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/motion';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    path=source('PathingEntity');npc=source('NpcList');player=source('Player');seq=source('SeqType');bas=source('BasType');buffer=source('Buffer');source('Protocol')
    def fields(text):return '\n'.join(re.findall(r'(?:public|private|protected) (?:static )?(?:final )?(?:int|boolean|byte)(?:\[\])* \w+(?:\s*=\s*[^;]+)?;',text)).replace('private ','public ').replace('protected ','public ')
    (run/'SeqType.java').write_text(seq[seq.index('public final class SeqType'):seq.index('public void decode(')]+method(seq,'public void decode(')+method(seq,'private void decode(')+method(seq,'public void postDecode(')+'}',encoding='utf-8')
    code=r'''import java.io.*;import java.util.*;
class GlobalConfig{static boolean USE_TWEENING=true;}
class Buffer{byte[]data;int offset;Buffer(byte[]b){data=b;} BUFFER }
class BasType{ BAS }
class SlotAnimation{int seqId,currentFrame,nextFrame,delayClock,startDelay,replayCount;}
class PathingEntity{ FIELDS SlotAnimation[]slotAnimations=new SlotAnimation[12];BasType bas;int getSize(){return size;}BasType getBasType(){return bas;} PATH }
class Player extends PathingEntity{ REQUEST }
class PlayerList{static Player self;static int selfId=17;static Player[]players=new Player[2048];}
class Npc extends PathingEntity{NpcType type=new NpcType();}class NpcType{boolean rotationflag=true;}
class Camera{static int originX,originY;}class client{static int loop;}
class GlRenderer{static boolean enabled=false;}class FogManager{static void setInstantFade(){}}
class SeqTypeList{static Map<Integer,SeqType>types=new HashMap<>();static SeqType get(int id){return types.get(id);}}
class SpotAnimType{int seqId=-1;}class SpotAnimTypeList{static SpotAnimType get(int id){return new SpotAnimType();}}
class SoundPlayer{static List<String>events=new ArrayList<>();static void playSeqSound(int y,SeqType s,int x,boolean self,int frame){events.add("["+s.id+","+frame+"]");}}
class NpcList{static Npc[]npcs=new Npc[32768]; PHASES }
public class Probe{
static String track(int id,int frame,int next,int clock,int delay,int repeats){return Arrays.toString(new int[]{id,frame,next,clock,delay,repeats});}
static String snapshot(Player p){List<String>q=new ArrayList<>();for(int i=p.movementQueueSize-1;i>=0;i--)q.add(Arrays.toString(new int[]{p.movementQueueX[i],p.movementQueueY[i],p.movementQueueSpeed[i]}));return "{\"fine\":"+Arrays.toString(new int[]{p.xFine,p.yFine})+",\"head\":"+Arrays.toString(new int[]{p.movementQueueX[0],p.movementQueueY[0],p.movementQueueSpeed[0]})+",\"queue\":"+q+",\"catchup\":"+p.movementCatchupTicks+",\"angles\":"+Arrays.toString(new int[]{p.currentAngle,p.targetAngle,p.standingTurnCounter})+",\"face\":"+Arrays.toString(new int[]{p.faceEntity,p.faceX,p.faceY})+",\"force\":"+Arrays.toString(new int[]{p.forceMoveCyclesToStart,p.forceMoveCyclesToDest})+",\"steps\":"+p.seqMovementSteps+",\"movement\":"+track(p.movementSeqId,p.movementSeqFrame,p.movementSeqNextFrame,p.movementSeqDelayClock,0,0)+",\"action\":"+track(p.seqId,p.seqFrame,p.seqNextFrame,p.seqDelayClock,p.seqDelay,p.seqReplayCount)+",\"sounds\":"+SoundPlayer.events+"}";}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;Player p=null;while((line=r.readLine())!=null){String[]a=line.split(";");int[]v=new int[a.length-1];if(!a[0].equals("S"))for(int i=1;i<a.length;i++)v[i-1]=Integer.parseInt(a[i]);switch(a[0]){
case"R":p=new Player();p.bas=new BasType();p.bas.idleAnimationId=0;p.bas.walkAnimation=1;p.bas.runAnimationId=2;p.bas.standingCCWTurn=3;p.bas.standingCWTurn=4;p.bas.walkFullTurnAnimationId=5;p.bas.walkCCWTurnAnimationId=6;p.bas.walkCWTurnAnimationId=7;PlayerList.self=v[4]!=0?p:null;PlayerList.players[2047]=p;PlayerList.players[23]=new Player();NpcList.npcs[8]=new Npc();Camera.originX=v[2];Camera.originY=v[3];p.movementQueueX[0]=v[0];p.movementQueueY[0]=v[1];p.xFine=v[0]*128+64;p.yFine=v[1]*128+64;SoundPlayer.events.clear();SeqTypeList.types.clear();break;
case"S":{SeqType s=new SeqType();s.id=Integer.parseInt(a[1]);s.decode(new Buffer(HexFormat.of().parseHex(a[2])));s.postDecode();SeqTypeList.types.put(s.id,s);break;}
case"M":p.move(v[0],v[1]);break;
case"P":p.teleport(1,v[0],v[1],v[2]!=0);break;
case"A":Player.animate(v[1],v[0],p);break;
case"F":p.forceMoveStartX=v[0];p.forceMoveStartY=v[1];p.forceMoveDestX=v[2];p.forceMoveDestY=v[3];p.forceMoveCyclesToStart=v[4];p.forceMoveCyclesToDest=v[5];p.forceMoveDirection=v[6];p.movementQueueSize=1;p.seqMovementSteps=0;break;
case"E":p.faceEntity=v[0];break;
case"L":p.faceX=v[0];p.faceY=v[1];break;
case"X":p.xFine=v[0];p.yFine=v[1];break;
case"B":PlayerList.players[23].xFine=v[0];PlayerList.players[23].yFine=v[1];NpcList.npcs[8].xFine=v[2];NpcList.npcs[8].yFine=v[3];break;
case"T":client.loop=v[0];NpcList.processEntity(1,p);System.out.println(snapshot(p));SoundPlayer.events.clear();break;
}}}}
'''.replace('BUFFER',''.join(method(buffer,s)for s in ['public final int g1(', 'public final int g2(', 'public final int g3('])).replace(' BAS ',fields(bas[:bas.index('public final void postDecode(')])).replace(' FIELDS ',fields(path[:path.index('public final BasType getBasType(')])).replace(' PATH ',''.join(method(path,s)for s in ['public final void move(', 'public final void teleport(', 'public final void resetMovementQueue('])).replace(' REQUEST ',method(player,'public static void animate(')).replace(' PHASES ',''.join(method(npc,s)for s in ['public static void processEntity(', 'public static void lerpToForceMoveStart(', 'public static void lerpToForceMoveDest(', 'public static void processMovement(', 'public static void processFacing(', 'public static void processAnimations(']))
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java'),str(run/'SeqType.java')],check=True)
    rng=random.Random(530491);cases=[];inputs=[]
    def u2(n):return n.to_bytes(2,'big')
    for scenario in range(48):
        start=[rng.randrange(25,75),rng.randrange(25,75)];origin=[3152,3168]if scenario%2 else [0,0];is_self=scenario%3!=0;commands=[];sequences=[];inputs.append(';'.join(map(str,['R',*start,*origin,int(is_self)])))
        for id in range(14):
            n=rng.randrange(2,7);wire=bytes([1])+u2(n)+b''.join(u2(rng.randrange(1,7))for _ in range(n))+b''.join(u2(i)for i in range(n))+b'\0\1'*n+bytes([2])+u2(n)+bytes([5,id%4,8,3,9,(id//3)%3,10,id%3,11,1,0]);sequences.append(wire.hex());inputs.append(f'S;{id};{wire.hex()}')
        for tick in range(192):
            cycle=100+tick
            def command(*args):commands.append(list(args));inputs.append(';'.join(map(str,args)))
            if tick%3==0:command('M',rng.randrange(3),rng.randrange(8))
            if tick%17==0:command('A',rng.randrange(8,14),rng.randrange(4))
            if tick%43==9:command('P',rng.randrange(20,80),rng.randrange(20,80),int(tick%2==0))
            if tick%47==13:command('F',rng.randrange(20,80),rng.randrange(20,80),rng.randrange(20,80),rng.randrange(20,80),cycle+3,cycle+14,rng.randrange(4))
            if tick%19==0:command('E',rng.choice([-1,8,32768+23,32768+17]))
            if tick%23==0:command('L',(origin[0]+rng.randrange(20,80))*2,(origin[1]+rng.randrange(20,80))*2)
            if tick%7==0:command('B',rng.randrange(20,80)*128+64,rng.randrange(20,80)*128+64,rng.randrange(20,80)*128+64,rng.randrange(20,80)*128+64)
            if tick in [87,147]:command('X',rng.choice([64,14000,1408]),rng.randrange(30,70)*128)
            command('T',cycle)
        cases.append(dict(start=start,origin=origin,is_self=is_self,sequences=sequences,commands=commands))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==9216
    it=iter(result)
    for case in cases:case['expected']=[json.loads(next(it))for _ in range(192)]
    content=json.dumps(dict(sources=hashes,bindings='Unchanged PathingEntity move/teleport/resetMovementQueue, Player.animate and complete NpcList.processEntity (motion, forced interpolation, facing, animation). Original primitive field defaults. Synthetic default-player BAS/sequence catalog, explicit size=1, origins, target arrays and sound observations. Force mask assigns the original fields and queue size. No accelerated/nondefault BAS or authoritative movement claim.',cases=cases),separators=(',',':')).encode()+b'\n';path=root/'fixtures/players/motion.json'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Actor motion corpus differs')
    else:path.write_bytes(content)
    print('Original complete actor entity phases: 48 scenarios / 9216 ticks')
