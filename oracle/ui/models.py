"""Original type6 UI camera dispatch and widget animation clocks.

GL records projection/matrix calls instead of opening a window. Resolved geometry,
CS1 truth, sequence metadata and interface services are explicit inputs.
"""
import argparse,hashlib,importlib.util,json,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
RUN=ROOT/'runs/ui-model-oracle';FIX=ROOT/'fixtures/ui/models.json'
spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
spec=importlib.util.spec_from_file_location('f',ROOT/'tools/port_component_fields.py');f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)

def generate(check=False):
    RUN.mkdir(parents=True,exist_ok=True);sources={}
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    comp=source('Component')
    fields=''.join('public '+t+' '+n+('='+value if value else '')+';'for t,n,value in f.fields()if t in['int','boolean','byte','short'])
    (RUN/'Component.java').write_text('class Component{'+fields+'Component[]createdComponents;static boolean loadFailed;static ModelCache models=new ModelCache();'+v.method(comp,'public final Model getModel(')+'}',encoding='utf-8')
    gl=source('GlRenderer');methods=['public static void setupModelPreview(', 'public static void setFullscreenCamera(', 'public static void setupPerspectiveView(', 'private static void setPerspectiveFrustum(', 'public static void setViewportBounds(', 'private static void resizeViewport()', 'public static void setDepthBias(']
    (RUN/'GlRenderer.java').write_text('class GlRenderer {static GL2 gl=new GL2();static boolean enabled=true,orthographicActive;static int canvasWidth,canvasHeight,leftMargin,topMargin,viewportWidth,viewportHeight;static float[]matrix=new float[16];static float projectionZScale,projectionZOffset,projectionDistance,depthBias,hFOV,vFOV;static final float NEAR_PLANE_SCALE=0.09765625F;static void restoreLighting(){}static void setDepthTestEnabled(boolean b){}static void setFogEnabled(boolean b){}static void clearDepthBuffer(){}static void disableDepthMask(){}static void enableDepthMask(){}'+''.join(v.method(gl,m)for m in methods)+'}',encoding='utf-8')
    cs=source('Cs1ScriptRunner');part=cs[cs.index('if (component.type == 6)'):];draw=v.method(part,'if (model != null)')
    (RUN/'Draw.java').write_text('class Draw{static int[]clip;static void draw(Component component,int componentX,int componentY,int memory){Model model=new GlModel();int color,cardMemory,temp4,objId,temp5,temp6,clipLeft=clip[0],clipTop=clip[1],clipRight=clip[2],clipBottom=clip[3];'+draw+'}}',encoding='utf-8')
    model=source('GlModel');(RUN/'GlModel.java').write_text('class GlModel extends Model{int uniqueVertexCount=1;void ensureBuffersAndDraw(){}'+v.method(model,'public final void setCamera(')+'}',encoding='utf-8')
    interface=source('InterfaceList');(RUN/'InterfaceList.java').write_text('class InterfaceList{static Object modelProvider=new Object();static Component hoveredComponent;static EmptyMap openInterfaces=new EmptyMap();static int redraws;static boolean isHidden(Component c){return c.hidden;}static void redraw(Component c){redraws++;}static void updateAnimations(int id){}'+v.method(interface,'public static void updateAnimationsRecursive(')+'}',encoding='utf-8')
    (RUN/'SelectionServices.java').write_text(SELECTION_SERVICES,encoding='utf-8')
    (RUN/'MathUtils.java').write_text(source('MathUtils'),encoding='utf-8')
    source('GlobalConfig')
    (RUN/'Support.java').write_text(SUPPORT.replace('c.modelViewAngle=x[14]','c.modelViewAngle=(short)x[14]').replace('c.modelNearClip=x[15]','c.modelNearClip=(short)x[15]'),encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*[str(p)for p in RUN.glob('*.java')]],check=True)
    clip_rng=random.Random(530608);rng=random.Random(530601);camera=[];clocks=[];requests=[]
    for _ in range(1024):
        width=rng.randrange(10,700);height=rng.randrange(10,500)
        x=[rng.randrange(2),rng.randrange(2),width,height,rng.choice([0,width,width*2]),rng.choice([0,height,height*2]),rng.randrange(-50,51),rng.randrange(-50,51),rng.randrange(2048),rng.randrange(2048),rng.randrange(2048),rng.randrange(64,5001),rng.randrange(-400,401),rng.randrange(-400,401),rng.choice([0,64,128,512]),rng.choice([50,3000,32767]),rng.randrange(2),rng.randrange(-200,1100),rng.randrange(-200,700),rng.randrange(-400,401),rng.choice([765,1280,1920]),rng.choice([503,720,1080])]
        x += [clip_rng.randrange(0,200),clip_rng.randrange(0,150),x[20]-clip_rng.randrange(0,200),x[21]-clip_rng.randrange(0,150)]
        camera.append(dict(args=x));requests.append('P\t'+','.join(map(str,x)))
    for i in range(1024):
        delays=[rng.randrange(1,21)for _ in range(rng.randrange(1,9))];n=len(delays)
        x=[rng.randrange(2),rng.randrange(2),rng.randrange(2),rng.choice([-1,1]),rng.choice([-1,2]),rng.randrange(n),rng.choice([-1,0,1,n-1]),rng.randrange(-4,30),rng.randrange(2048),rng.randrange(2048),rng.choice([0,1,-1,0x7fff8000,-2147450881]),rng.choice([-2,-1,0,1,2,20,101]),rng.choice([-1,0,1,n,n+3]),rng.randrange(2)]
        clocks.append(dict(args=x,delays=delays));requests.append('C\t'+','.join(map(str,x))+'\t'+','.join(map(str,delays)))
    selection=[];selection_rng=random.Random(530609)
    for i in range(512):
        q=selection_rng;x=[q.choice([0,1,2,3,4,6,7,8]),q.choice([-1,0,2,65535,0x12340065]),q.randrange(9),q.choice([-1,0,2,65535,0x12340065]),q.randrange(308),q.choice([-1,17,23]),q.choice([-1,19,29]),q.randrange(12),q.choice([-1,0,1,11]),q.randrange(0,80),q.randrange(2)]
        selection.append(dict(args=x));requests.append('S\t'+','.join(map(str,x)))
    output=subprocess.run(['java','-cp',str(RUN),'Support'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(requests)
    for case,line in zip(camera+clocks+selection,output):case['expected']=json.loads(line)
    result=dict(sources=sources,bindings='Complete unchanged type6 resolved-model drawing branch, GlRenderer preview/frustum/depth-bias and GlModel.setCamera methods; GL call recorder supplies standard glOrtho and records raw float bits. Complete InterfaceList.updateAnimationsRecursive with a single component, controlled CS1 truth, sequence lookup and empty interface service. Complete Component.getModel dispatch uses recording, ready metadata/model services and an empty model cache. Geometry resolution, full hierarchy/CS1, OpenGL driver and actual pixels are separate contracts.',camera=camera,clocks=clocks,selection=selection)
    text=json.dumps(result,separators=(',',':'))+'\n'
    if check:
        if json.loads(FIX.read_text(encoding='utf-8'))!=result:raise SystemExit('UI model fixture differs')
    else:FIX.write_text(text,encoding='utf-8')
    print('Original UI model cameras:',len(camera),'animation clock cases:',len(clocks))

SUPPORT=r'''
import java.io.*;import java.util.*;
class GL2{static final int GL_PROJECTION=1,GL_MODELVIEW=2;int mode;float[]projection;List<float[]>ops=new ArrayList<>();int[]viewport;
void glMatrixMode(int n){mode=n;}void glLoadIdentity(){}void glLoadMatrixf(float[]m,int o){projection=m.clone();}
void glRotatef(float d,float x,float y,float z){ops.add(new float[]{d,x,y,z});}void glTranslatef(float x,float y,float z){ops.add(new float[]{x,y,z});}void glPushMatrix(){}void glPopMatrix(){}
void glViewport(int x,int y,int w,int h){viewport=new int[]{x,y,w,h};}
void glOrtho(double l,double r,double b,double t,double n,double f){projection=new float[]{(float)(2/(r-l)),0,0,0,0,(float)(2/(t-b)),0,0,0,0,(float)(-2/(f-n)),0,(float)(-(r+l)/(r-l)),(float)(-(t+b)/(t-b)),(float)(-(f+n)/(f-n)),1};}}
class GlobalConfig{static int VIEW_DISTANCE=7168;}class GameShell{static float canvasScale=1,subpixelX,subpixelY;}class Rasteriser{static int screenLowerX,screenUpperX,screenLowerY,screenUpperY;static void setBounds(int x,int y){}static void prepareOffsets(){}}
class Preferences{static int brightness=3;}class FogManager{static void init(int n){}}class GlRaster{static int[]clip;static void resetClipRegion(){}static void setClip(int a,int b,int c,int d){clip=new int[]{a,b,c,d};}}class ScriptRunner{static boolean glSceneNeedsRender=true;}
class Model{int kind,id,detail,sequence=-1,frame=-1,next=-1,clock=-1;static Model make(int k,int id,int d){Model m=new Model();m.kind=k;m.id=id;m.detail=d;return m;}void setCamera(int a,int b,int c,int d,int e,int f,long key){}}class SoftwareModel extends Model{void renderOnInterface(int a,int b,int c,int d,int e,int f,int g){}}
class EmptyMap{Object get(long n){return null;}}class ComponentPointer{int interfaceId;}class Protocol{static int sceneDelta;}class Cs1ScriptRunner{static boolean active;static boolean isTrue(Component c){return active;}}
class SeqType{int id;int[]frames,frameDelay;int replayoff;Model animateEntity(Model m,int next,int cycle,int frame){m.sequence=id;m.frame=frame;m.next=next;m.clock=cycle;return m;}}class SeqTypeList{static SeqType seq;static SeqType get(int id){return seq;}}
public class Support{
 static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
 static String bits(float[]f){long[]out=new long[f.length];for(int i=0;i<f.length;i++)out[i]=Float.floatToIntBits(f[i])&0xffffffffL;return Arrays.toString(out);}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split("\t");int[]x=ints(p[1]);Component c=new Component();
 if(p[0].equals("P")){c.if3=x[0]!=0;c.modelOrtho=x[1]!=0;c.width=x[2];c.height=x[3];c.modelViewportWidth=x[4];c.modelViewportHeight=x[5];c.modelOriginX=x[6];c.modelOriginY=x[7];c.modelXAngle=x[8];c.modelYAngle=x[9];c.modelYOffset=x[10];c.modelZoom=x[11];c.modelXOffset=x[12];c.modelZOffset=x[13];c.modelViewAngle=x[14];c.modelNearClip=x[15];c.modelTransparent=x[16]!=0;GlRenderer.canvasWidth=x[20];GlRenderer.canvasHeight=x[21];GlRenderer.gl=new GL2();ScriptRunner.glSceneNeedsRender=true;Draw.clip=Arrays.copyOfRange(x,22,26);Draw.draw(c,x[17],x[18],x[19]);GL2 gl=GlRenderer.gl;StringJoiner ops=new StringJoiner(",","[","]");for(float[]op:gl.ops)ops.add(bits(op));System.out.println("["+bits(gl.projection)+","+ops+","+Arrays.toString(gl.viewport)+","+Arrays.toString(GlRaster.clip)+"]");
 }else if(p[0].equals("S")){c.modelType=x[0];c.modelId=x[1];c.activeModelType=x[2];c.activeModelId=x[3];c.modelFrameCycle=x[4];int seqId=x[10]!=0?x[6]:x[5];SeqType seq=null;if(seqId!=-1){seq=new SeqType();seq.id=seqId;}Model m=c.getModel(x[8],seq,x[7],x[9],x[10]!=0,new PlayerAppearance());System.out.println(m==null?"null":Arrays.toString(new int[]{m.kind,m.id,m.detail,m.sequence,m.frame,m.next,m.clock}));
 }else{c.type=6;c.overlayer=-1;c.if3=x[0]!=0;c.hidden=x[1]!=0;Cs1ScriptRunner.active=x[2]!=0;c.modelSeqId=x[3];c.activeModelSeqId=x[4];c.seqFrame=x[5];c.seqNextFrame=x[6];c.seqCycle=x[7];c.modelXAngle=x[8];c.modelYAngle=x[9];c.modelRotationSpeed=x[10];Protocol.sceneDelta=x[11];SeqTypeList.seq=null;if(x[13]!=0){SeqType s=new SeqType();s.frameDelay=ints(p[2]);s.frames=new int[s.frameDelay.length];s.replayoff=x[12];SeqTypeList.seq=s;}InterfaceList.redraws=0;InterfaceList.updateAnimationsRecursive(-1,new Component[]{c});System.out.println(Arrays.toString(new int[]{c.seqFrame,c.seqNextFrame,c.seqCycle,c.modelXAngle,c.modelYAngle,InterfaceList.redraws}));}
 }}}
'''

SELECTION_SERVICES=r'''
class ModelCache{Object get(long n){return null;}void put(Model m,long n){}}
class RawModel{int id;RawModel(int n){id=n;}static RawModel create(Object p,int id){return new RawModel(id);}Model createModel(int a,int b,int x,int y,int z){return Model.make(1,id,0);}}
class NpcTypeList{static NpcType get(int n){return new NpcType(n);}}class NpcType{int id;NpcType(int n){id=n;}Model getHeadModel(SeqType s,int cycle,int next,int frame){Model m=Model.make(2,id,0);return s==null?m:s.animateEntity(m,next,cycle,frame);}Model getBodyModel(Object slots,int a,int b,int next,int cycle,int frame,Object movement,int c,SeqType s){Model m=Model.make(6,id,0);return s==null?m:s.animateEntity(m,next,cycle,frame);}}
class ObjTypeList{static ObjType get(int n){return new ObjType(n);}}class ObjType{int id;ObjType(int n){id=n;}Model getModel(int next,int cycle,SeqType s,int count,int frame){Model m=Model.make(4,id,count);return s==null?m:s.animateEntity(m,next,cycle,frame);}}
class PlayerAppearance{Model getHeadModel(int cycle,SeqType s,int frame,int next){Model m=Model.make(3,0,0);return s==null?m:s.animateEntity(m,next,cycle,frame);}Model getPartialHeadModel(int next,int hair,int jaw,int cycle,SeqType s,int frame,int beard){Model m=Model.make(7,jaw<<16|beard,hair);return s==null?m:s.animateEntity(m,next,cycle,frame);}}
'''

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
