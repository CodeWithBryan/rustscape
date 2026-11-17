"""Original minimap state/hints, hint projection, morph and startup metadata helpers."""
import argparse,hashlib,importlib.util,json,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]

def generate(check=False):
    spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    run=ROOT/'runs/minimap-state-oracle';run.mkdir(parents=True,exist_ok=True);sources={}
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    protocol=source('Protocol');mini=source('MiniMap');buffer=source('Buffer');loc=source('LocType');equipment=source('Equipment');msi=source('MsiTypeList')
    (run/'MapMarker.java').write_text(source('MapMarker'),encoding='utf-8');(run/'MathUtils.java').write_text(source('MathUtils'),encoding='utf-8')
    (run/'Protocol.java').write_text('class Protocol{static int opcode;static Buffer inboundBuffer;static boolean apply(int op,byte[]data){opcode=op;inboundBuffer=new Buffer(data);'+v.method(protocol,'if (opcode == ServerProt.SET_MINIMAP_STATE)')+v.method(protocol,'if (opcode == ServerProt.HINT_ARROW)')+'return false;}}',encoding='utf-8')
    (run/'MiniMap.java').write_text('class MiniMap{static int state,compassAngleOffset,zoomOffset;static MapMarker[]hintMapMarkers=new MapMarker[4];'+v.method(mini,'public static void renderMapIcon(')+v.method(mini,'public static void renderHintMarker(')+'}',encoding='utf-8')
    (run/'Equipment.java').write_text('class Equipment{static int[]objIds;'+v.method(equipment,'public static void init()')+'}',encoding='utf-8')
    (run/'MsiTypeList.java').write_text('class MsiTypeList{static Js5 archive,spritesArchive;static int redDelta,greenDelta,blueDelta;'+v.method(msi,'public static void init(').replace('Math.random()','RandomTape.next()')+'}',encoding='utf-8')
    (run/'LocType.java').write_text('class LocType{int id,multiLocVarbit=-1,multiLocVarp=-1;int[]multiLocs;'+v.method(loc,'public LocType getMultiLoc()')+'}',encoding='utf-8')
    code='''import java.io.*;import java.util.*;
class Buffer{byte[]data;int offset;Buffer(byte[]d){data=d;}'''+v.method(buffer,'public final int g1()')+v.method(buffer,'public final int g2()')+'''}
class ServerProt{static int SET_MINIMAP_STATE=192,HINT_ARROW=217;}
class Sprite{int innerWidth,innerHeight;}
class SoftwareSprite extends Sprite{void drawClipped(int x,int y,int[]s,int[]w){}void drawRotatedFixed(int x,int y,double a){}}
class GlSprite extends Sprite{void renderClipped(int x,int y,GlSprite m){Probe.projected=new int[]{0,x,y};}void renderRotatedFixed(int x,int y,int a){Probe.projected=new int[]{1,x,y,a};}}
class GlRenderer{static boolean enabled=true;}
class Component{int width,height;int[]clickMaskStart,clickMaskWidth;Sprite getSprite(boolean b){return new GlSprite();}}
class Camera{static float yawTarget;}
class Sprites{static Sprite[]headhints=new Sprite[3],mapmarkhints=new Sprite[3],hintMapEdge=new Sprite[3];}
class VarpDomain{static int bit;static int[]activeVarps=new int[16];static int getVarbit(int id){return bit;}}
class LocTypeList{static LocType get(int id){LocType d=new LocType();d.id=id;return d;}}
class ObjType{int manwear,womanwear;}
class ObjTypeList{static int capacity;static ObjType[]types;static ObjType get(int id){return types[id];}}
class Js5{int getGroupCapacity(int n){return 0;}}
class RandomTape{static double[]values;static int at;static double next(){return values[at++];}}
public class Probe{static int[]projected;
static int[]ints(String s){return s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
static String hints(){String[]out=new String[4];for(int i=0;i<4;i++){MapMarker h=MiniMap.hintMapMarkers[i];out[i]=h==null?"null":Arrays.toString(new int[]{h.type,h.arrowSpriteId,h.targetXFine,h.targetYFine,h.targetX,h.targetY,h.targetHeight,h.actorTargetId,h.playerModelId});}return "["+MiniMap.state+",["+String.join(",",out)+"]]";}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);int[]n=ints(p[1]);switch(p[0]){
case "P":Protocol.apply(n[0],HexFormat.of().parseHex(p[2]));System.out.println(hints());break;
case "H":{Component c=new Component();c.width=n[0];c.height=n[1];Camera.yawTarget=n[2];MiniMap.compassAngleOffset=n[3];MiniMap.zoomOffset=n[4];for(int i=0;i<3;i++){GlSprite s=new GlSprite();s.innerWidth=n[7];s.innerHeight=n[8];Sprites.mapmarkhints[i]=s;Sprites.hintMapEdge[i]=new GlSprite();}projected=null;MiniMap.renderHintMarker(0,0,0,n[5],n[6],c);System.out.println("["+(projected==null?"null":Arrays.toString(projected))+","+(n[9]%20<10)+"]");break;}
case "R":{LocType d=new LocType();d.multiLocVarbit=n[0];d.multiLocVarp=n[1];d.multiLocs=ints(p[2]);VarpDomain.bit=n[2];if(n[1]>=0)VarpDomain.activeVarps[n[1]]=n[3];LocType result=d.getMultiLoc();System.out.println(result==null?"null":result.id);break;}
case "E":{ObjTypeList.capacity=n[0];ObjTypeList.types=new ObjType[n[0]];for(int i=0;i<n[0];i++){ObjType d=new ObjType();d.manwear=n[1+i*2];d.womanwear=n[2+i*2];ObjTypeList.types[i]=d;}Equipment.init();System.out.println(Arrays.toString(Equipment.objIds));break;}
case "J":{RandomTape.values=Arrays.stream(p[2].split(",")).mapToDouble(Double::parseDouble).toArray();RandomTape.at=0;MsiTypeList.init(new Js5(),new Js5());System.out.println(Arrays.toString(new int[]{MsiTypeList.redDelta,MsiTypeList.greenDelta,MsiTypeList.blueDelta}));break;}
default:throw new IllegalArgumentException(p[0]);}}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[];requests=[]
    def add(kind,args,extra=None):
        cases.append(dict(kind=kind,args=args,extra=extra));requests.append(kind+';'+','.join(map(str,args))+(';' + (extra if isinstance(extra,str)else','.join(map(str,extra)))if extra is not None else''))
    for state in range(256):add('P',[192],bytes([state,255,17]).hex())
    for _ in range(1024):
        kind=rng.choice([0,1,2,3,4,5,6,7,10,63]);flags=rng.randrange(4)<<6|kind;sprite=rng.choice([0,1,2,3,255]);payload=bytes([flags,sprite])+rng.randbytes(5)+rng.choice([65535,rng.randrange(65536)]).to_bytes(2,'big');add('P',[217],payload.hex())
    for _ in range(1024):add('H',[rng.randrange(20,513),rng.randrange(20,513),rng.randrange(2048),rng.randrange(-16,17),rng.choice([-255,-128,0,128,1024]),rng.choice([-32768,-601,-600,-300,-1,0,1,300,600,601,32768]),rng.randrange(-650,651),rng.randrange(1,41),rng.randrange(1,41),rng.choice([-2147483648,-21,-20,-1,0,9,10,19,20,2147483647])])
    for _ in range(512):add('R',[rng.choice([-1,2,5]),rng.choice([-1,2,5]),rng.choice([-2147483648,-1,0,1,4,8,2147483647]),rng.choice([-2147483648,-1,0,1,4,8,2147483647])],[rng.choice([-1,0,1,33,65534])for _ in range(rng.randrange(2,10))])
    for _ in range(128):
        n=rng.randrange(1,129);add('E',[n]+[rng.choice([-2147483648,-2,-1,0,1,65535,2147483647])for _ in range(n*2)])
    for _ in range(128):add('J',[],[rng.random()for _ in range(4)])
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(cases)
    for c,line in zip(cases,output):c['expected']=json.loads(line)
    data=(json.dumps(dict(sources=sources,bindings='Unchanged original minimap192/hint217 bodies, full MapMarker defaults, hint/icon projection, LocType.getMultiLoc, Equipment.init and MsiTypeList.init with explicit RandomTape service. Sprite/GL calls, resolved item/varp metadata and packet framing are controlled; valid payloads retain ignored tails.',cases=cases),separators=(',',':'))+'\n').encode();path=ROOT/'fixtures/minimap/state.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original minimap state corpus differs')
    else:path.write_bytes(data)
    print(f'Original minimap state: {len(cases)} packet/hint/morph/equipment/jitter cases')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
