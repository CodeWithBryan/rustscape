"""Original tile height, item support placement and LocType support metadata.

Only LocType opcodes17/27/74/75 are supplied to the copied branches. Complete
postDecode and SceneGraph methods run unchanged with explicit scene/entity inputs.
"""
import argparse,hashlib,importlib.util,json,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
RUN=ROOT/'runs/ground-geometry-oracle';FIX=ROOT/'fixtures/ui/ground-geometry.json'

def generate(check=False):
    spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    sources={}
    def source(name):
        b=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    scene=source('SceneGraph');loc=source('LocType')
    code='class SceneGraph{static int[][][]tileHeights=new int[4][105][105];static byte[][][]renderFlags=new byte[4][104][104];static Tile[][][]tiles=new Tile[4][104][104];'
    code+=v.method(scene,'public static int getTileHeight(')+v.method(scene,'public static void setObjStack(')+'}'
    code+='class LocType{static final int CENTREPIECE_STRAIGHT=10;int interactable=-1,supportitems=-1,blockwalk=2;int[]models,shapes;String[]ops=new String[5];boolean blockrange=true,breakroutefinding;void decode(Buffer buffer){while(true){int opcode=buffer.g1();if(opcode==0)return;'
    for op in [17,27,74,75]:code+=v.method(loc,f'else if (opcode == {op})')
    code+='else throw new IllegalArgumentException("adapter opcode" );}}'+v.method(loc,'public void postDecode()')+'}'
    RUN.mkdir(parents=True,exist_ok=True);(RUN/'GroundGeometryOracle.java').write_text(code+SUPPORT,encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),str(RUN/'GroundGeometryOracle.java')],check=True)
    rng=random.Random(53075);cases=[];requests=[]
    for i in range(512):
        raw=bytearray()
        for _ in range(rng.randrange(12)):
            op=rng.choice([17,27,74,75]);raw.append(op)
            if op==75:raw.append(rng.randrange(256))
        raw.append(0);cases.append(dict(kind='metadata',bytes=raw.hex()));requests.append('M\t'+raw.hex())
    for i in range(1024):
        plane=rng.randrange(4);point=[rng.randrange(104*128),rng.randrange(104*128)]
        if i%19==0:point[i%2]=rng.choice([-2147483648,-129,-1,104*128,2147483647])
        heights=[[rng.randrange(-2**31,2**31)if i%11==0 else rng.randrange(-4000,1000)for _ in range(4)]for _ in range(4)]
        flags=rng.randrange(256);cases.append(dict(kind='height',plane=plane,point=point,heights=heights,flags=flags));requests.append('\t'.join(map(str,['H',plane,*point,flags,*[n for row in heights for n in row]])))
    for i in range(512):
        supports=[[rng.choice([0,0x400000,-1,0x400001,0x800000]),rng.choice([-32768,-32767,-1,0,1,32767,rng.randrange(-32768,32768)])]for _ in range(rng.randrange(5))]
        cases.append(dict(kind='support',supports=supports));requests.append('\t'.join(map(str,['S',len(supports),*[n for pair in supports for n in pair]])))
    p=subprocess.run(['java','-cp',str(RUN),'GroundGeometryOracle'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True)
    lines=p.stdout.splitlines();assert len(lines)==len(cases)
    for c,line in zip(cases,lines):c['expected']=int(line)
    out=dict(sources=sources,bindings='Complete original SceneGraph.getTileHeight/setObjStack and LocType.postDecode with unchanged opcode17/27/74/75 branches. Explicit height grids/bridge flags, entity minY and support-key bits. Metadata inputs contain only those four opcodes; getMinY values are inputs rather than GPU bounds; this does not establish all original scene rendering or occlusion.',cases=cases)
    if check:assert json.loads(FIX.read_text())==out,'Ground geometry fixture differs'
    else:FIX.write_text(json.dumps(out,separators=(',',':'))+'\n')
    print('Original ground placement/support metadata:',len(cases),'cases')

SUPPORT=r'''
class Buffer{byte[]data;int offset;Buffer(byte[]data){this.data=data;}int g1(){return data[offset++]&255;}}
class Entity{int minY;Entity(int value){minY=value;}int getMinY(){return minY;}}
class Scenery{long key;Entity entity;Scenery(long key,int minY){this.key=key;entity=new Entity(minY);}}
class ObjStackEntity{Entity primary,secondary,tertiary;int xFine,yFine,zFine,heightOffset;long key;}
class Tile{Scenery[]scenery=new Scenery[5];int sceneryLen;ObjStackEntity objStack;Tile(int p,int x,int y){}}
public class GroundGeometryOracle{
static int number(String[]s,int n){return Integer.parseInt(s[n]);}
public static void main(String[]args)throws Exception{var r=new java.io.BufferedReader(new java.io.InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=r.readLine())!=null){String[]s=line.split("\t");
if(s[0].equals("M")){LocType t=new LocType();t.decode(new Buffer(java.util.HexFormat.of().parseHex(s[1])));t.postDecode();System.out.println(t.supportitems);continue;}
if(s[0].equals("H")){int plane=number(s,1),fx=number(s,2),fy=number(s,3),x=fx>>7,y=fy>>7;if(x>=0&&y>=0&&x<104&&y<104){SceneGraph.renderFlags[1][x][y]=(byte)number(s,4);int p=5;for(int l=0;l<4;l++){SceneGraph.tileHeights[l][x][y]=number(s,p++);SceneGraph.tileHeights[l][x+1][y]=number(s,p++);SceneGraph.tileHeights[l][x][y+1]=number(s,p++);SceneGraph.tileHeights[l][x+1][y+1]=number(s,p++);}}System.out.println(SceneGraph.getTileHeight(plane,fx,fy));continue;}
Tile t=new Tile(0,1,2);t.sceneryLen=number(s,1);for(int i=0;i<t.sceneryLen;i++)t.scenery[i]=new Scenery(Long.parseLong(s[2+i*2]),number(s,3+i*2));SceneGraph.tiles[0][1][2]=t;SceneGraph.setObjStack(0,1,2,-42,new Entity(0),1610612993L,null,null);System.out.println(t.objStack.heightOffset);
}}}
'''
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');generate(p.parse_args().check)
