"""Original type5 widget GL draw routing over recording graphics services."""
import argparse,pathlib,importlib.util,hashlib,subprocess,json,random

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-widget-draw-oracle';O.mkdir(exist_ok=True)
    spec=importlib.util.spec_from_file_location('sprite_draw_visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources={}
    def src(name):
     b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    s=src('Cs1ScriptRunner');branch=v.method(s,'if (component.type == 5)');(O/'Draw.java').write_text('class Draw {static boolean active;static boolean isTrue(Component c){return active;}static void draw(Component component,int componentX,int componentY,int alpha,int clipLeft,int clipTop,int clipRight,int clipBottom){int i=0,temp2,temp3,temp4,temp5,temp6,memory,color,cardMemory;'+branch+'}}',encoding='utf-8')
    s=src('IntUtils');(O/'IntUtils.java').write_text('class IntUtils{'+v.method(s,'public static boolean isPowerOfTwo(')+'}',encoding='utf-8')
    (O/'Support.java').write_text('''
    import java.util.*;
    class Log {static List<String>calls=new ArrayList<>();static void call(String name,int...a){calls.add("[\\\""+name+"\\\","+Arrays.toString(a)+"]");}}
    class Sprite {int width,height,innerWidth,innerHeight;void render(int x,int y){Log.call("Natural",x,y);}void renderResized(int x,int y,int w,int h){Log.call("Resized",x,y,w,h);}void renderAlpha(int x,int y,int w,int h,int a){Log.call("AlphaResize",x,y,w,h,a);}void renderAngled(int y,int angle,int zoom,int x){Log.call("Rotated",y,angle,zoom,x);}void renderAlpha(int x,int y,int a){Log.call("AlphaNatural",x,y,a);}}
    class GlSprite extends Sprite {void renderTiled(int x,int y,int nx,int ny){Log.call("Tiled",x,y,nx,ny);}void renderTiledAlpha(int x,int y,int a,int nx,int ny){Log.call("AlphaTile",x,y,a,nx,ny);}}
    class GlRenderer {static boolean enabled=true;}
    class GlRaster {static void setClipRegion(int x,int y,int r,int b){Log.call("Clip",x,y,r,b);}static void setClip(int x,int y,int r,int b){Log.call("Clip",x,y,r,b);}}
    class SoftwareRaster {static void shrinkClip(int x,int y,int r,int b){throw new AssertionError();}static void setClip(int x,int y,int r,int b){throw new AssertionError();}}
    class Component {static boolean loadFailed=false;static GlSprite image;int type=5,objId=-1,outlineThickness,objCount,shadowColor,width,height,angle2d;boolean if3,spriteTiling,objDrawText;Sprite getSprite(boolean active){Log.call("Sprite",active?1:0);return image;}}
    class Inv {static Sprite getObjectSprite(int o,int id,boolean text,int count,int s){Log.call("Item",o,id,text?1:0,count,s);return Component.image;}}
    class InterfaceList {static void redraw(Component c){throw new AssertionError("missing assets outside corpus");}}
    class PluginRepository {static void ComponentDraw(int i,Component c,int x,int y){Log.call("Plugin",x,y);}}
    '''.replace('\\\"','\\"'),encoding='utf-8')
    (O/'Probe.java').write_text('''
    import java.io.*;import java.util.*;
    class Probe {public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String l;while((l=r.readLine())!=null){int[]a=Arrays.stream(l.split(",")).mapToInt(Integer::parseInt).toArray();Component c=new Component();c.if3=a[0]!=0;c.spriteTiling=a[1]!=0;c.width=a[2];c.height=a[3];c.angle2d=a[4];GlSprite image=new GlSprite();image.width=a[5];image.height=a[6];image.innerWidth=a[7];image.innerHeight=a[8];Component.image=image;Draw.active=a[9]!=0;Log.calls.clear();Draw.draw(c,a[10],a[11],a[12],a[13],a[14],a[15],a[16]);System.out.println(Log.calls.toString());}}}
    ''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],check=True)
    rng=random.Random(530500);cases=[]
    for i in range(2048):
     size=[rng.choice([1,2,3,4,7,8,15,16,31,32])for _ in range(2)];inner=[size[a]+rng.randrange(8)for a in range(2)];rect=[rng.randrange(-20,1280),rng.randrange(-20,720),rng.randrange(1,120),rng.randrange(1,120)];clip=[rng.randrange(30),rng.randrange(30),rng.randrange(800,1300),rng.randrange(500,800)];cases.append(dict(if3=bool(i&1),tiling=bool(i&2),rect=rect,angle=rng.choice([0,0,0,1,16384,32768,65535,-1,65536]),size=size,inner=inner,active=bool(i&4),alpha=rng.choice([-1,0,0,1,127,128,255,256,257]),clip=clip))
    for i in range(128):
     size=[[8,8],[8,7],[7,8],[7,7]][i%4];inner=[size[0]+(i%3),size[1]+(i%5)];rect=[31,47,*inner];cases.append(dict(if3=bool(i&1),tiling=bool(i&2),rect=rect,angle=[0,0,16384,65535][i%4],size=size,inner=inner,active=bool(i&4),alpha=[0,0,128,256,257,-1,255,1][(i//4)%8],clip=[0,0,1280,720]))
    commands=[','.join(map(str,[int(c['if3']),int(c['tiling']),c['rect'][2],c['rect'][3],c['angle'],*c['size'],*c['inner'],int(c['active']),*c['rect'][:2],c['alpha'],*c['clip']]))for c in cases]
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(l)for l in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    content=json.dumps(dict(scope='Complete unchanged original Cs1ScriptRunner type5 GL branch and IntUtils power-of-two predicate, with already-ready sprite geometry, condition value and recording graphics/plugin/asset services. Software raster and missing-asset branches excluded. Original draw arguments/order are compared, not GPU shader pixels.',sources=sources,cases=cases),indent=2)+'\n'
    fixture=R/'fixtures/ui/widget-draw.json'
    if check:assert fixture.read_text()==content,'widget draw routing fixture differs'
    else:fixture.write_text(content,encoding='utf-8')
    print('PASS original type5 draw routing',len(cases),'cases')

if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');generate(parser.parse_args().check)
