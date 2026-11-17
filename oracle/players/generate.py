"""Original RT4 player membership/movement/mask readers with explicit actor bindings."""
import argparse,hashlib,importlib.util,json,pathlib,random,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];SRC=ROOT/'reference/client/client/src/main/java/rt4';RUN=ROOT/'runs/players-oracle';FIX=ROOT/'fixtures/players'
spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)

class Bits:
    def __init__(self):self.values=[]
    def put(self,n,value):self.values.extend((value>>i)&1 for i in range(n-1,-1,-1))
    def bytes(self):
        self.values.extend([0]*(-len(self.values)%8));return bytes(sum(self.values[i+j]<<(7-j)for j in range(8))for i in range(0,len(self.values),8))

def appearance(rng):
    flags=rng.randrange(256);out=bytearray([flags,rng.randrange(256),rng.randrange(256)])
    if rng.randrange(5)==0:out.extend(b'\xff\xff'+struct.pack('>H',rng.randrange(1000))+bytes([rng.randrange(256)]))
    else:
        for _ in range(12):
            value=rng.choice([0,256+rng.randrange(250),32768+rng.randrange(50)])
            out.extend(b'\0'if value==0 else struct.pack('>H',value))
    out.extend(bytes(rng.randrange(256)for _ in range(5)));out.extend(struct.pack('>HqB',rng.randrange(65536),rng.randrange(1,37**11),rng.randrange(256)))
    out.extend(struct.pack('>H',rng.randrange(65536))if flags&4 else bytes([rng.randrange(256),rng.choice([0,5,100,255])]))
    radius=rng.choice([0,0,1,255]);out.append(radius)
    if radius:out.extend(struct.pack('>4H',*[rng.randrange(65536)for _ in range(4)]))
    return bytes(out)

def masks(rng,app):
    flags=rng.choice([0,1,2,4,8,32,64,128,256,512,1024,2048,4095&~16,rng.randrange(4096)&~16]);out=bytearray([flags&255|(16 if flags>>8 else 0)])
    if flags>>8:out.append(flags>>8)
    def smart(n):return bytes([n])if n<128 else struct.pack('>H',32768+n)
    if flags&128:
        text=bytes(rng.randrange(256)for _ in range(rng.randrange(16)));out.extend(struct.pack('<HBB',rng.randrange(65536),rng.randrange(3),len(text))+text)
    if flags&1:out.extend(smart(rng.randrange(32768))+bytes([rng.randrange(256),rng.randrange(256)]))
    if flags&8:out.extend(struct.pack('>HB',rng.choice([65535,808,866]),rng.randrange(256)))
    if flags&4:out.extend(bytes([(len(app)+128)&255])+app)
    if flags&2:
        n=rng.choice([0,65535,rng.randrange(65536)]);out.extend(bytes([n>>8,(n+128)&255]))
    if flags&1024:out.extend(bytes(rng.randrange(256)for _ in range(9)))
    if flags&32:out.extend(rng.choice([b'hello',b'~hello',b'caf\xe9'])+b'\0')
    if flags&512:out.extend(smart(rng.randrange(32768))+bytes([rng.randrange(256)]))
    if flags&2048:
        n=rng.randrange(5);out.append((-n)&255)
        for _ in range(n):out.extend(struct.pack('<H',rng.choice([65535,808,866]))+bytes([rng.randrange(256)])+struct.pack('>H',rng.randrange(65536)))
    if flags&256:
        n=rng.randrange(2**32);b=struct.pack('>I',n);out.extend(struct.pack('<H',rng.choice([65535,100,500]))+bytes([b[1],b[0],b[3],b[2]]))
    if flags&64:
        n=rng.randrange(65536);out.extend(struct.pack('>H',rng.randrange(65536))+bytes([(n+128)&255,n>>8]))
    return bytes(out)

def main(check=False):
    RUN.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    protocol=source('Protocol');pathing=source('PathingEntity');buffer=source('Buffer');packet=source('Packet')
    methods=['public static void readPlayerInfoPacket()', 'public static void readSelfPlayerInfo()', 'public static void readPlayerInfo()', 'public static void readNewPlayerInfo()', 'public static void readExtendedPlayerInfo()']
    body=''.join(v.method(protocol,s)for s in methods)
    extended=v.method(protocol,'public static void readExtendedPlayerInfo(int flags, int id, Player player)').replace('void readExtendedPlayerInfo(', 'void maskBody(')
    bm=''.join(v.method(buffer,s)for s in ['public final int g1(', 'public final byte g1b(', 'public final int g1add(', 'public final int g1sub(', 'public final int g1neg(', 'public final int g2(', 'public final int g2add(', 'public final int ig2(', 'public final int ig2add(', 'public final int mg4(', 'public final int gsmarts(', 'public final void gdata(', 'public final void igdata('])
    bm+=''.join(v.method(packet,s)for s in ['public final void accessBits(', 'public final void accessBytes(', 'public final int gBits(', 'public final int availableBits('])
    (RUN/'Probe.java').write_text(SUPPORT.replace('BUFFER_METHODS',bm).replace('PATH_METHODS',v.method(pathing,'public final void move(')+v.method(pathing,'public final void teleport(')).replace('PROTOCOL_METHODS',body+extended),encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),str(RUN/'Probe.java')],check=True)
    rng=random.Random(530);cases=[];inputs=[]
    for case_id in range(96):
        start=[rng.randrange(104),rng.randrange(104)];ids=[];packets=[];base_cycle=rng.choice([0,100,2147483600]);inputs.append('R;'+','.join(map(str,start)))
        for tick in range(24):
            bits=Bits();ext=[];maskbytes=bytearray();kind=rng.randrange(5)
            bits.put(1,kind!=0)
            if kind:
                move=kind-1;bits.put(2,move)
                if move==0:ext.append(2047)
                elif move==1:bits.put(3,rng.randrange(8));has=rng.randrange(2);bits.put(1,has);ext.extend([2047]if has else[])
                elif move==2:
                    run=rng.randrange(2);bits.put(1,run);bits.put(3,rng.randrange(8))
                    if run:bits.put(3,rng.randrange(8))
                    has=rng.randrange(2);bits.put(1,has);ext.extend([2047]if has else[])
                else:
                    bits.put(7,rng.randrange(128));bits.put(1,rng.randrange(2));bits.put(2,rng.randrange(4));has=rng.randrange(2);bits.put(1,has);bits.put(7,rng.randrange(128));ext.extend([2047]if has else[])
            count=rng.randrange(len(ids)+1);bits.put(8,count);new_ids=[]
            for index in ids[:count]:
                changed=rng.randrange(2);bits.put(1,changed)
                if not changed:new_ids.append(index);continue
                move=rng.randrange(4);bits.put(2,move)
                if move==3:continue
                new_ids.append(index)
                if move==0:ext.append(index)
                else:
                    if move==2:
                        run=rng.randrange(2);bits.put(1,run)
                    bits.put(3,rng.randrange(8))
                    if move==2 and run:bits.put(3,rng.randrange(8))
                    has=rng.randrange(2);bits.put(1,has);ext.extend([index]if has else[])
            candidates=[i for i in ids if i not in new_ids]+rng.sample(range(2047),8)
            for index in candidates[:rng.randrange(6)]:
                if index in new_ids:continue
                new_ids.append(index);bits.put(11,index);has=rng.randrange(2);bits.put(1,has);ext.extend([index]if has else[]);bits.put(5,rng.randrange(32));bits.put(3,rng.randrange(8));bits.put(1,rng.randrange(2));bits.put(5,rng.randrange(32))
            bits.put(11,2047);ids=new_ids
            for index in ext:maskbytes.extend(masks(rng,appearance(rng)))
            data=bits.bytes()+maskbytes;cycle=(base_cycle+tick)&0xffffffff;cycle=cycle if cycle<2**31 else cycle-2**32;packets.append(dict(cycle=cycle,wire=data.hex()));inputs.append(f'P;{cycle};{data.hex()}')
        cases.append(dict(start=start,packets=packets))
    result=subprocess.run(['java','-cp',str(RUN),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==sum(len(c['packets'])for c in cases)
    it=iter(result)
    for case in cases:
        for packet in case['packets']:packet['expected']=json.loads(next(it))
    FIX.mkdir(parents=True,exist_ok=True);content=json.dumps(dict(sources=hashes,bindings='Unchanged Protocol player readers and PathingEntity queue methods. Sequence cancellation, fine-coordinate simulation, appearance/cache resolution, chat filtering/decompression, animation priorities, sounds and particles are explicit bindings; complete raw mask fields are compared. Player appearance decode is separately compared.',cases=cases),separators=(',',':')).encode()+b'\n';p=FIX/'oracle.json'
    if check:
        if p.read_bytes()!=content:raise SystemExit('Player corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 player readers: {len(cases)} scenarios / {len(result)} packets')
    spec=importlib.util.spec_from_file_location('appearance_corpus',pathlib.Path(__file__).with_name('appearance_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('session_corpus',pathlib.Path(__file__).with_name('session_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('animation_corpus',pathlib.Path(__file__).with_name('animation_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('motion_corpus',pathlib.Path(__file__).with_name('motion_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('bas_motion',pathlib.Path(__file__).with_name('bas_motion.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(check)
    spec=importlib.util.spec_from_file_location('destination_corpus',pathlib.Path(__file__).with_name('destination_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('wheel_corpus',pathlib.Path(__file__).with_name('wheel_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('overheads_corpus',pathlib.Path(__file__).with_name('overheads_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('morph_corpus',pathlib.Path(__file__).with_name('morph_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('npc_corpus',pathlib.Path(__file__).with_name('npc_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('sound_corpus',pathlib.Path(__file__).with_name('sound_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    spec=importlib.util.spec_from_file_location('blend_corpus',pathlib.Path(__file__).with_name('blend_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v,check)
    spec=importlib.util.spec_from_file_location('npc_definitions',pathlib.Path(__file__).with_name('npc_definitions.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v,check)
    spec=importlib.util.spec_from_file_location('npc_assets',pathlib.Path(__file__).with_name('npc_assets.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v,check)
    spec=importlib.util.spec_from_file_location('spots',pathlib.Path(__file__).with_name('spots.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(check)
    spec=importlib.util.spec_from_file_location('masked',pathlib.Path(__file__).with_name('masked.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(check)
    spec=importlib.util.spec_from_file_location('parts_corpus',pathlib.Path(__file__).with_name('parts_corpus.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(check)
    spec=importlib.util.spec_from_file_location('animation_assets',pathlib.Path(__file__).with_name('animation_assets.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(check)

SUPPORT=r'''
import java.io.*;import java.util.*;
class JagString{String s;JagString(String v){s=v;}int charAt(int n){return s.charAt(n);}JagString substring(int n){return new JagString(s.substring(n));}JagString trim(){return this;}JagString encodeMessage(){return this;}long encode37(){return 1;}static JagString concatenate(JagString[]v){return new JagString("");}}
class Buffer{byte[]data;int offset,bitOffset;static final int[] BIT_MASKS=new int[33];static{for(int i=0;i<32;i++)BIT_MASKS[i]=(1<<i)-1;BIT_MASKS[32]=-1;}Buffer(byte[] b){data=b;}JagString gjstr(){int start=offset;while(data[offset++]!=0){}return new JagString(new String(data,start,offset-start-1,java.nio.charset.StandardCharsets.ISO_8859_1));} BUFFER_METHODS }
class PathingEntity{static int[] ANGLES={768,1024,1280,512,1536,256,0,1792};int[]movementQueueX=new int[10],movementQueueY=new int[10];byte[]movementQueueSpeed=new byte[10];int movementQueueSize,seqId=-1,movementCatchupTicks,seqMovementSteps,xFine,yFine; PATH_METHODS }
class Player extends PathingEntity{static int plane,inTutorialIsland=1;int lastSeenLoop,currentAngle,targetAngle,soundRadius,faceEntity=-1,forceMoveStartX,forceMoveStartY,forceMoveDestX,forceMoveDestY,forceMoveCyclesToStart,forceMoveCyclesToDest,forceMoveDirection,chatEffect,chatColor,chatLoops,hitpointsBarVisibleUntil,hitpointsBar,spotAnimId=-1,spotAnimStart,spotAnimDelayClock,spotAnimFrame,spotAnimY,spotAnimNextFrame,faceX,faceY;Object appearance;JagString username,chatMessage;byte[]rawAppearance;List<Object>hits=new ArrayList<>(),animations=new ArrayList<>(),slots=new ArrayList<>();int getSize(){return 1;}void teleport(int x,boolean discard,int y){super.teleport(1,x,y,discard);}void decodeAppearance(Buffer b){rawAppearance=b.data;appearance=new Object();}JagString getName(){return new JagString("");}void addHit(int kind,int loop,int damage){hits.add(new int[]{damage,kind});}static void animate(int delay,int id,Player p){p.animations.add(new int[]{id,delay});}static void applySlotAnimations(int[]d,int[]ids,Player p,int[]m){for(int i=0;i<d.length;i++)p.slots.add(new int[]{ids[i],d[i],m[i]});}}
class PlayerList{static Player[]players=new Player[2048];static Player self;static int[]ids=new int[2048];static int size;static Buffer[]appearanceCache=new Buffer[2048];}class client{static int loop;}class GlRenderer{static boolean enabled=false;}class FogManager{static void setInstantFade(){}}class SeqType{int movetype=0,priority=5;int[] frames;}class SeqTypeList{static SeqType get(int id){return new SeqType();}}class SpotAnimType{int seqId=-1;}class SpotAnimTypeList{static SpotAnimType get(int id){return new SpotAnimType();}}class AreaSoundManager{static void remove(Player p){}}class SoundPlayer{static void playSeqSound(int y,SeqType s,int x,boolean self,int frame){}}class LoginManager{static boolean playerUnderage,parentalChatConsent,mapQuickChat;}class IgnoreList{static int size;static long[] encodedUsernames=new long[0];}class QuickChatPhraseType{static QuickChatPhrase decodePhrase(Buffer b){return new QuickChatPhrase();}}class QuickChatPhrase{int id;QuickType type=new QuickType();}class QuickType{JagString decodeMessage(Buffer b){return new JagString("");}}class Font{static JagString escape(JagString s){return s;}}class Chat{static void add(Object...v){}}
public class Probe{
static Buffer inboundBuffer,chatBuffer=new Buffer(new byte[65536]);static int length,extendedCount,removedCount;static int[]extendedIds=new int[2048],removedIds=new int[2048];static JagString IMG0=new JagString(""),IMG1=IMG0;static List<Object>masks;
static JagString formatChatMessage(Buffer b){return new JagString("");}
PROTOCOL_METHODS
static Map<String,Object>obj(Object...v){Map<String,Object>o=new TreeMap<>();for(int i=0;i<v.length;i+=2)o.put((String)v[i],v[i+1]);return o;}
static String hex(byte[]v){return v==null?null:HexFormat.of().formatHex(v);}
static void readExtendedPlayerInfo(int flags,int index,Player p){p.hits.clear();p.animations.clear();p.slots.clear();int start=inboundBuffer.offset;List<Object>fields=new ArrayList<>();if((flags&128)!=0){Buffer b=new Buffer(inboundBuffer.data);b.offset=start;int color=b.ig2(),rights=b.g1(),n=b.g1();byte[]payload=new byte[n];b.igdata(n,payload);fields.add(obj("type","PublicChat","color_effect",color,"rights",rights,"payload",hex(payload)));}maskBody(flags,index,p);int hit=0;if((flags&1)!=0){int[]h=(int[])p.hits.get(hit++);fields.add(obj("type","Hit","damage",h[0],"kind",h[1],"bar",p.hitpointsBar));}if((flags&8)!=0){int[]a=(int[])p.animations.get(0);fields.add(obj("type","Animation","id",a[0],"delay",a[1]));}if((flags&4)!=0)fields.add(obj("type","Appearance","bytes",hex(p.rawAppearance)));if((flags&2)!=0)fields.add(obj("type","FaceEntity","value",p.faceEntity));if((flags&1024)!=0)fields.add(obj("type","ForcedMovement","start",new int[]{p.forceMoveStartX,p.forceMoveStartY},"destination",new int[]{p.forceMoveDestX,p.forceMoveDestY},"start_cycle",p.forceMoveCyclesToStart,"end_cycle",p.forceMoveCyclesToDest,"direction",p.forceMoveDirection));if((flags&32)!=0)fields.add(obj("type","OverheadText","text",p.chatMessage.s));if((flags&512)!=0){int[]h=(int[])p.hits.get(hit);fields.add(obj("type","Hit","damage",h[0],"kind",h[1],"bar",null));}if((flags&2048)!=0)fields.add(obj("type","SlotAnimations","values",p.slots));if((flags&256)!=0)fields.add(obj("type","SpotAnimation","id",p.spotAnimId,"height",p.spotAnimY,"start_cycle",p.spotAnimStart));if((flags&64)!=0)fields.add(obj("type","FaceTile","tile",new int[]{p.faceX,p.faceY}));masks.add(obj("index",index,"flags",flags,"fields",fields));}
static Object snapshot(){List<Object>p=new ArrayList<>();for(int i=0;i<2048;i++){Player a=PlayerList.players[i];if(a!=null)p.add(obj("id",i,"x",a.movementQueueX,"y",a.movementQueueY,"speed",a.movementQueueSpeed,"size",a.movementQueueSize,"seen",a.lastSeenLoop,"angles",new int[]{a.currentAngle,a.targetAngle},"appearance",hex(a.rawAppearance)));}return obj("ids",Arrays.copyOf(PlayerList.ids,PlayerList.size),"plane",Player.plane,"players",p,"extended",masks,"removed_candidates",Arrays.copyOf(removedIds,removedCount));}
static String json(Object o){if(o==null)return"null";if(o instanceof String){StringBuilder b=new StringBuilder("\"");for(char c:((String)o).toCharArray()){if(c=='"'||c=='\\')b.append('\\').append(c);else if(c<32)b.append(String.format("\\u%04x",(int)c));else b.append(c);}return b.append('"').toString();}if(o instanceof Number||o instanceof Boolean)return o.toString();if(o instanceof Map){List<String>a=new ArrayList<>();for(var e:((Map<?,?>)o).entrySet())a.add(json(e.getKey())+":"+json(e.getValue()));return"{"+String.join(",",a)+"}";}List<String>a=new ArrayList<>();if(o instanceof Iterable)for(Object v:(Iterable<?>)o)a.add(json(v));else for(int i=0;i<java.lang.reflect.Array.getLength(o);i++)a.add(json(java.lang.reflect.Array.get(o,i)));return"["+String.join(",",a)+"]";}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]x=line.split(";",-1);if(x[0].equals("R")){PlayerList.players=new Player[2048];PlayerList.appearanceCache=new Buffer[2048];PlayerList.ids=new int[2048];PlayerList.size=0;Player.plane=0;PlayerList.self=new Player();String[]pos=x[1].split(",");PlayerList.self.movementQueueX[0]=Integer.parseInt(pos[0]);PlayerList.self.movementQueueY[0]=Integer.parseInt(pos[1]);PlayerList.players[2047]=PlayerList.self;continue;}client.loop=Integer.parseInt(x[1]);inboundBuffer=new Buffer(HexFormat.of().parseHex(x[2]));length=inboundBuffer.data.length;masks=new ArrayList<>();readPlayerInfoPacket();System.out.println(json(snapshot()));}}}
'''

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');main(p.parse_args().check)
