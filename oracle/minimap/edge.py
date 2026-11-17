"""Original HD edge-arrow transform commands, with trimmed-sheet offsets zero."""
import argparse, hashlib, importlib.util, json, pathlib, random, subprocess
ROOT = pathlib.Path(__file__).resolve().parents[2]

def generate(check=False):
    spec = importlib.util.spec_from_file_location('v', ROOT/'oracle/visual/generate.py')
    v = importlib.util.module_from_spec(spec); spec.loader.exec_module(v)
    raw = (ROOT/'reference/client/client/src/main/java/rt4/GlSprite.java').read_bytes().replace(b'\r\n',b'\n')
    method = v.method(v.clean(raw.decode()),'public final void renderRotatedFixed(')
    run = ROOT/'runs/minimap-edge-oracle'; run.mkdir(parents=True,exist_ok=True)
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class GL2 {float cx,cy,angle,px,py;int calls;void glTranslatef(float x,float y,float z){if(calls++==0){cx=x;cy=-y;}else{px=-x;py=y;}}void glRotatef(float a,float x,float y,float z){angle=a;}void glCallList(int n){}void glLoadIdentity(){}}
class GlRenderer {static GL2 gl;static int canvasHeight;static void begin2DReplace(){}static void setTextureId(int n){}}
class GlSprite {int textureId,xOffset,yOffset,displayListId;void setTextureFilter(int n){if(n!=2)throw new AssertionError();}'''+method+'''}
public class Probe {public static void main(String[]args)throws Exception {BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){int[]n=Arrays.stream(line.split(",")).mapToInt(Integer::parseInt).toArray();GlRenderer.gl=new GL2();new GlSprite().renderRotatedFixed(n[0],n[1],n[2]);GL2 g=GlRenderer.gl;System.out.println(Arrays.toString(new int[]{Float.floatToIntBits(g.cx),Float.floatToIntBits(g.cy),Float.floatToIntBits(g.angle),Float.floatToIntBits(g.px),Float.floatToIntBits(g.py)}));}}}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(530)
    cases=[dict(args=[rng.randrange(-16000,16001),rng.randrange(-16000,16001),rng.choice([-2147483648,-65535,-32768,-1,0,1,32768,65535,2147483647])])for _ in range(512)]
    out=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(','.join(map(str,c['args']))for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines()
    assert len(out)==len(cases)
    for c,line in zip(cases,out):c['expected']=json.loads(line)
    data=(json.dumps(dict(sources=dict(GlSprite=hashlib.sha256(raw).hexdigest()),bindings='Unchanged GlSprite.renderRotatedFixed with recorded GL commands, canvasHeight0 and trimmed sprite offsets0. Screen Y reverses GL Y; texture filtering is asserted2. This verifies transform arguments, not framebuffer pixels.',cases=cases),separators=(',',':'))+'\n').encode()
    path=ROOT/'fixtures/minimap/edge.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original minimap edge corpus differs')
    else:path.write_bytes(data)
    print(f'Original minimap HD edge transforms: {len(cases)} cases')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
