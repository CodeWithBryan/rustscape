"""Pinned original minimap raster, masks, projection, HD UV and click observations."""
import argparse,hashlib,importlib.util,json,pathlib,random,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]

def generate(check=False):
    spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    run=ROOT/'runs/minimap-oracle';run.mkdir(parents=True,exist_ok=True);sources={}
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    mini=source('MiniMap');component=source('Component');gl=source('GlSprite');cs1=source('Cs1ScriptRunner')
    tables=''.join(re.search(r'public static final int\[\]\[\] '+name+r' = .*?;',mini).group()for name in ['tileShapeMasks','tileShapeRotations'])
    (run/'MiniMap.java').write_text('class MiniMap {static int compassAngleOffset,zoomOffset;'+tables+v.method(mini,'public static void renderTile(')+v.method(mini,'public static void renderMapIcon(')+'}',encoding='utf-8')
    (run/'Component.java').write_text('class Component {int spriteId,width,height;int[] clickMaskStart,clickMaskWidth;Sprite getSprite(boolean b){return new GlSprite();}'+v.method(component,'public final boolean buildClickMask(')+'}',encoding='utf-8')
    (run/'MathUtils.java').write_text(source('MathUtils'),encoding='utf-8')
    (run/'GlSprite.java').write_text('class GlSprite extends SoftwareSprite {int textureId,powerOfTwoWidth,powerOfTwoHeight;void setTextureFilter(int n){}void renderClipped(int x,int y,GlSprite mask){Probe.position=new int[]{x,y};}'+v.method(gl,'public final void renderRotatedTransparent(')+'}',encoding='utf-8')
    first=cs1.index('temp1 = ScriptRunner.interfaceMouseX - componentX;');last=cs1.index('if (MiniMenu.isTargeting',first)
    click=cs1[first:last].replace('continue;','return null;')
    code='''import java.io.*;import java.util.*;
class Sprite{int innerWidth,innerHeight;}
class SoftwareSprite extends Sprite{int width,height;void drawClipped(int x,int y,int[]s,int[]w){Probe.position=new int[]{x,y};}}
class SoftwareIndexedSprite{int width,height;byte[]pixels;void trim(){}}
class SpriteLoader{static SoftwareIndexedSprite sprite;static SoftwareIndexedSprite loadSoftwareIndexedSprite(int id,Object p){return sprite;}}
class InterfaceList{static Object spriteProvider;}
class Camera{static float yawTarget;}
class PlainTile{int flatColor;}
class ShapedTile{int shape,rotation,textureId,flatColor;}
class Tile{PlainTile plainTile;ShapedTile shapedTile;}
class SceneGraph{static Tile[][][]tiles=new Tile[4][104][104];}
class GlobalConfig{static boolean BILINEAR_MINIMAP=true;}
class GL2{static int GL_TEXTURE0,GL_TEXTURE1,GL_TEXTURE_2D,GL_TEXTURE_ENV,GL_COMBINE_RGB,GL_REPLACE,GL_SRC0_RGB,GL_PREVIOUS,GL_TRIANGLE_FAN,GL_MODULATE,GL_TEXTURE;List<Integer>bits=new ArrayList<>();void glActiveTexture(int a){}void glEnable(int a){}void glBindTexture(int a,int b){}void glTexEnvi(int a,int b,int c){}void glBegin(int a){}void glColor4f(float a,float b,float c,float d){}void glMultiTexCoord2f(int a,float b,float c){}void glTexCoord2f(float u,float v){bits.add(Float.floatToRawIntBits(u));bits.add(Float.floatToRawIntBits(v));}void glVertex2f(float a,float b){}void glEnd(){}void glDisable(int a){}}
class GlRenderer{static boolean enabled=true;static int canvasHeight=800;static GL2 gl=new GL2();static void begin2DReplace(){}static void setTextureId(int n){}}
class ScriptRunner{static int interfaceMouseX,interfaceMouseY;}
class Player{int xFine,yFine;}class PlayerList{static Player self=new Player();}
public class Probe{static int[]position;
static int[]click(Component component,int px,int py,int fx,int fy){
 if(px<0||py<0||px>=component.width||py>=component.height)return null;
 ScriptRunner.interfaceMouseX=px;ScriptRunner.interfaceMouseY=py;PlayerList.self.xFine=fx;PlayerList.self.yFine=fy;
 int componentX=0,componentY=0,temp1,temp2,temp3,memory,color,cardMemory,objId,temp4,temp5,temp6;
 '''+click+'''return new int[]{temp5,temp6};}
static String array(int[]a){return a==null?"null":Arrays.toString(a);}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[] p=line.split(";",-1);int[]n=p[1].isEmpty()?new int[0]:Arrays.stream(p[1].split(",")).mapToInt(Integer::parseInt).toArray();Component c=new Component();switch(p[0]){
case "T":{int[]pixels=new int[512*512];Arrays.fill(pixels,n[4]);Tile t=new Tile();if(n[0]<2){t.plainTile=new PlainTile();t.plainTile.flatColor=n[0]==0?n[2]:n[3];}else{t.shapedTile=new ShapedTile();t.shapedTile.shape=n[0];t.shapedTile.rotation=n[1];t.shapedTile.textureId=n[2];t.shapedTile.flatColor=n[3];}SceneGraph.tiles[0][1][1]=t;MiniMap.renderTile(pixels,0,0,1,1);int[]out=new int[16];for(int i=0;i<16;i++)out[i]=pixels[i/4*512+i%4];System.out.println(array(out));break;}
case "M":{SoftwareIndexedSprite s=new SoftwareIndexedSprite();s.width=n[0];s.height=n[1];s.pixels=HexFormat.of().parseHex(p[2]);SpriteLoader.sprite=s;c.buildClickMask();int[]out=new int[n[1]*2];for(int i=0;i<n[1];i++){out[i*2]=c.clickMaskStart[i];out[i*2+1]=c.clickMaskWidth[i];}System.out.println(array(out));break;}
case "I":{c.width=n[0];c.height=n[1];GlSprite s=new GlSprite();s.innerWidth=n[2];s.innerHeight=n[3];Camera.yawTarget=n[4];MiniMap.compassAngleOffset=n[5];MiniMap.zoomOffset=n[6];position=null;MiniMap.renderMapIcon(c,s,n[8],n[7],0,0);System.out.println(array(position));break;}
case "C":{c.width=n[0];c.height=n[1];Camera.yawTarget=n[2];MiniMap.compassAngleOffset=n[3];MiniMap.zoomOffset=n[4];int[]m=Arrays.stream(p[2].split(",")).mapToInt(Integer::parseInt).toArray();c.clickMaskStart=new int[c.height];c.clickMaskWidth=new int[c.height];for(int i=0;i<c.height;i++){c.clickMaskStart[i]=m[i*2];c.clickMaskWidth[i]=m[i*2+1];}System.out.println(array(click(c,n[5],n[6],n[7],n[8])));break;}
case "U":{GlSprite s=new GlSprite();s.powerOfTwoWidth=n[6];s.powerOfTwoHeight=n[7];GlSprite mask=new GlSprite();mask.width=n[0];mask.height=n[1];mask.powerOfTwoWidth=mask.powerOfTwoHeight=1024;GlRenderer.gl.bits.clear();s.renderRotatedTransparent(0,0,n[0],n[1],n[2],n[3],n[4],n[5],mask);System.out.println(GlRenderer.gl.bits.toString());break;}
default:throw new IllegalArgumentException(p[0]);}}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[];requests=[]
    def add(kind,args,extra=None):
        c=dict(kind=kind,args=args);line=kind+';'+','.join(map(str,args))
        if extra is not None:c['extra']=extra;line+=';'+(extra if isinstance(extra,str)else','.join(map(str,extra)))
        cases.append(c);requests.append(line)
    for shape in range(13):
        for rotation in range(4):
            for i in range(8):add('T',[shape,rotation,rng.choice([0,1,0x113366,0xeeffff]),rng.choice([0,1,0xaaffcc,0xff33]),0x123456])
    for _ in range(256):
        w=rng.randrange(1,65);h=rng.randrange(1,65);pixels=bytes(rng.choice([0,0,0,1,3,255])for _ in range(w*h));add('M',[w,h],pixels.hex())
    for _ in range(1024):
        size=[rng.randrange(1,513),rng.randrange(1,513)];view=[rng.randrange(2048),rng.randrange(-16,17),rng.choice([-255,-128,0,128,1024])]
        add('I',size+[rng.randrange(1,129),rng.randrange(1,129)]+view+[rng.choice([-100000,-32768,-300,-1,0,1,300,32768,100000]),rng.randrange(-512,513)])
        mask=[]
        for y in range(size[1]):
            start=rng.randrange(size[0]);mask.extend([start,rng.randrange(size[0]-start+1)])
        add('C',size+view+[rng.randrange(-1,size[0]+1),rng.randrange(-1,size[1]+1),rng.randrange(-1024,16384),rng.randrange(-1024,16384)],mask)
        add('U',size+[rng.randrange(-64,577),rng.randrange(-64,577),rng.randrange(2048),rng.randrange(1,1025),rng.choice([64,128,256,512,1024]),rng.choice([64,128,256,512,1024])])
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(cases)
    for c,line in zip(cases,output):c['expected']=json.loads(line)
    data=(json.dumps(dict(sources=sources,bindings='Unchanged original tile raster, component first-run mask, map icon placement, HD rotated UVs, and exact minimap click-expression span. Scene/sprite/GL calls are explicit recorders; click rectangle/state/menu gates are controlled outside that span. Actual GPU and full-scene integration are separate.',cases=cases),separators=(',',':'))+'\n').encode();path=ROOT/'fixtures/minimap/oracle.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original minimap corpus differs')
    else:path.parent.mkdir(exist_ok=True);path.write_bytes(data)
    print(f'Original minimap: {len(cases)} tile/mask/icon/click/HD UV cases')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
