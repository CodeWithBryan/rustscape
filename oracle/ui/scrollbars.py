"""Unchanged RT4 IF1 scrollbar bodies, with recording input/raster services."""
import hashlib, importlib.util, json, pathlib, random, re, subprocess

def generate(check=False):
    root = pathlib.Path(__file__).resolve().parents[2]
    out = root / 'runs/ui-scrollbar-oracle'
    out.mkdir(exist_ok=True)
    spec = importlib.util.spec_from_file_location('visual', root / 'oracle/visual/generate.py')
    v = importlib.util.module_from_spec(spec); spec.loader.exec_module(v)
    sources = {}
    def source(name):
        raw = (v.SRC / (name+'.java')).read_bytes().replace(b'\r\n', b'\n')
        sources[name+'.java'] = hashlib.sha256(raw).hexdigest()
        return v.clean(raw.decode())
    text = source('InterfaceList')
    (out/'Original.java').write_text('class Original {static boolean scrollbarDragging;static int scrollbarDragMargin;static void redraw(Component c){Probe.redraws++;}'+v.method(text,'public static void handleScrollbar(')+'}')
    text = source('Cs1ScriptRunner')
    constants = ''.join(re.findall(r'public static final int SCROLLBAR_\w+ = 0x[0-9a-f]+;',text))
    assert constants.count(';') == 4
    (out/'Draw.java').write_text('class Draw {'+constants+v.method(text,'public static void renderScrollbar(')+'}')
    text = source('GlRaster')
    methods = ''.join(v.method(text,s) for s in ['public static void drawVerticalLine(', 'public static void drawHorizontalLine(', 'public static void fillRect('])
    (out/'OriginalGl.java').write_text('class OriginalGl {'+methods+'}')
    (out/'Probe.java').write_text(r'''
import java.io.*;import java.util.*;
class Component {int scrollY,width;}
class Mouse {static int pressedButton;}
class MouseWheel {static int wheelRotation;}
class Sprites {static Sprite[]scrollbars={new Sprite(0),new Sprite(1)};}
class Sprite {int frame;Sprite(int f){frame=f;}void renderTransparent(int x,int y){Probe.add(0,frame,x,y);}}
class GlRenderer {static boolean enabled=true;static int canvasHeight;static GL2 gl=new GL2();static void begin2DNoTexture(){}}
class GL2 {static final int GL_LINES=1,GL_TRIANGLE_FAN=6;void glBegin(int n){Probe.add(4,n);}void glColor3ub(byte r,byte g,byte b){Probe.add(5,r&255,g&255,b&255);}void glVertex2f(float x,float y){Probe.add(6,Float.floatToRawIntBits(x),Float.floatToRawIntBits(y));}void glEnd(){Probe.add(7);}}
class GlRaster {
static void fillRect(int x,int y,int w,int h,int c){Probe.add(1,x,y,w,h,c);}
static void drawVerticalLine(int x,int y,int n,int c){Probe.add(2,x,y,n,c);}
static void drawHorizontalLine(int x,int y,int n,int c){Probe.add(3,x,y,n,c);}}
class SoftwareRaster extends GlRaster{}
public class Probe {
static int redraws;static List<String> log=new ArrayList<>();
static void add(int...a){log.add(Arrays.toString(a));}
static int[] ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){String[]p=line.split(";");log.clear();int[]a=ints(p[0]);
if(a[0]==0){Component c=new Component();c.scrollY=a[1];Original.scrollbarDragging=a[2]!=0;Original.scrollbarDragMargin=a[3];List<String>result=new ArrayList<>();for(int k=1;k<p.length;k++){int[]b=ints(p[k]);c.width=b[0];Mouse.pressedButton=b[7];MouseWheel.wheelRotation=b[8];redraws=0;boolean error=false;try{Original.handleScrollbar(b[2],b[5],c,b[1],b[3],b[4],b[6]);}catch(ArithmeticException e){error=true;}result.add("["+c.scrollY+","+Original.scrollbarDragging+","+Original.scrollbarDragMargin+","+redraws+","+error+"]");}System.out.println("["+String.join(",",result)+"]");}
else if(a[0]==1){boolean error=false;try{Draw.renderScrollbar(a[1],a[2],a[3],a[4],a[5]);}catch(ArithmeticException e){error=true;}System.out.println("["+error+",["+String.join(",",log)+"]]");}
else{GlRenderer.canvasHeight=a[1];if(a[2]==1)OriginalGl.fillRect(a[3],a[4],a[5],a[6],a[7]);else if(a[2]==2)OriginalGl.drawVerticalLine(a[3],a[4],a[5],a[7]);else OriginalGl.drawHorizontalLine(a[3],a[4],a[5],a[7]);System.out.println("["+String.join(",",log)+"]");}
}}}
''')
    result = subprocess.run(['javac','-d',str(out),*[str(p) for p in out.glob('*.java')]],capture_output=True,text=True)
    (out/'compile.log').write_text(result.stdout+result.stderr); assert result.returncode == 0,result.stderr
    rng=random.Random(5304532); inputs=[]; cases=[]; draws=[]; gl=[]
    extremes=[-2147483648,-16777217,-1,0,1,16777217,2147483647]
    for index in range(1024):
        initial=[rng.choice(extremes),bool(index%2),rng.choice([0,32,-1])]; calls=[]
        for j in range(8):
            h=rng.choice([0,1,16,32,39,40,41,64,100,255,512,2147483647]); width=rng.choice([0,16,200,2147483647]); x=rng.randrange(-200,500);y=rng.randrange(-200,500)
            r=rng.choice([0,h,h+1 if h<2147483647 else -1,100,1000,2147483647]); mode=(index+j)%8
            mx=x+rng.choice([0,15,16,-1,-32,47,-width]); my=y+rng.choice([0,15,16,h-17,h-16,h-1,h,h+1])
            if mode<3:mx=x+4;my=y+[4,h-4,h//2][mode]
            calls.append([((n+2147483648)%4294967296)-2147483648 for n in [width,mx,my,x,y,h,r,rng.choice([0,1,2,3]),rng.choice([0,0,1,-1,2147483647])]])
        cases.append(dict(initial=initial,calls=calls)); inputs.append(';'.join([','.join(map(str,[0,initial[0],int(initial[1]),initial[2]])),*[','.join(map(str,b))for b in calls]]))
    for index in range(1024):
        h=rng.choice([0,16,32,40,64,100,512,2147483647]); r=rng.choice([0,h,100,1000,2147483647]);a=[rng.choice(extremes),r,rng.choice(extremes),rng.choice(extremes),h]
        draws.append(dict(arguments=a));inputs.append(','.join(map(str,[1,*a])))
    for index in range(512):
        a=[rng.choice([720,1080,2147483647]),rng.choice([1,2,3]),rng.choice(extremes),rng.choice(extremes),rng.choice(extremes),rng.choice(extremes),rng.randrange(-2147483648,2147483648)]
        gl.append(dict(arguments=a));inputs.append(','.join(map(str,[2,*a])))
    result=subprocess.run(['java','-cp',str(out),'Probe'],input='\n'.join(inputs)+'\n',capture_output=True,text=True)
    (out/'stdout.txt').write_text(result.stdout);(out/'stderr.txt').write_text(result.stderr);assert result.returncode==0,result.stderr
    rows=[json.loads(s)for s in result.stdout.splitlines()];assert len(rows)==len(inputs)
    for case,row in zip(cases+draws+gl,rows):case['expected']=row
    content=json.dumps(dict(scope='Unchanged original handleScrollbar and renderScrollbar bodies and actual GlRaster fill/axis-line calls. Input, redraw, indexed-arrow presentation and GL backend are recording services. Preserves shared per-call drag state, wheel boundaries, integer wrapping/division and completed failure prefixes. Complete UI traversal and framebuffer parity remain separate.',sources=sources,cases=cases,draws=draws,geometry=gl),separators=(',',':'))+'\n'
    fixture=root/'fixtures/ui/scrollbars.json'
    if check:assert fixture.read_text()==content,'Scrollbar fixture differs'
    else:fixture.write_text(content)
    print('PASS original scrollbars:1024 eight-call sequences,1024 paint prefixes,512 actual GL geometry cases',flush=True)

if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');generate(p.parse_args().check)
