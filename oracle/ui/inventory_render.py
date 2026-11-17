"""Extract complete original type2 body with recording sprite services."""
import hashlib,importlib.util,json,pathlib,random,subprocess
R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-inventory-render-oracle';O.mkdir(exist_ok=True)

def generate(check=False):
 spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
 raw=(v.SRC/'Cs1ScriptRunner.java').read_bytes().replace(b'\r\n',b'\n');s=v.clean(raw.decode());body=v.method(s[s.index('if (component.type != 1)'):],'if (component.type == 2)')
 (O/'Original.java').write_text('class Original {static void render(Component component,int componentX,int componentY,int clipLeft,int clipTop,int clipRight,int clipBottom){int temp1,temp2,temp3,objId,i=0,layer=-1,rectangle=0;Component[]components=null;'+body+'}}',encoding='utf-8')
 (O/'Probe.java').write_text(r'''
 import java.io.*;import java.util.*;
 class Component{int type=2,id,baseWidth,baseHeight,invMarginX,invMarginY,scrollY,scrollMaxV,height;int[]invOffsetX,invOffsetY,objTypes,objCounts,invSprite;boolean objDrawText;static boolean loadFailed;Sprite getInvSprite(int i){Probe.add(1,i);return i>=invSprite.length||invSprite[i]<0?null:new Sprite();}}
 class Sprite{void render(int x,int y){Probe.add(2,x,y,256);}void renderAlpha(int x,int y,int a){Probe.add(2,x,y,a);}}
 class Inv{static Sprite getObjectSprite(int outline,int id,boolean text,int count,int shadow){Probe.add(0,outline,id,text?1:0,count,shadow);return new Sprite();}}
 class Rasteriser{static boolean textureHasTransparency;}
 class InterfaceList{static Component clickedInventoryComponent;static int clickedInventoryComponentX,clickedInventoryComponentY,clickedInventoryComponentCycle,mouseOverInventoryObjectIndex;static boolean[]rectangleDirty=new boolean[1];static void redraw(Component c){}}
 class MiniMenu{static int itemTargetMode,selectedObjSlot,pressedSlotIndex;static Component pressedInventoryComponent;}
 class MiniMap{static int selectedComponentId;}
 class Mouse{static int lastMouseX,lastMouseY;}
 class Protocol{static int sceneDelta;}
 class GlRenderer{static boolean enabled;}
 class GlRaster{static int clipTop,clipBottom;}
 class SoftwareRaster{static int clipTop,clipBottom;}
 class PluginRepository{static void ComponentDraw(int i,Component c,int x,int y){}}
 public class Probe{
 static List<String> log=new ArrayList<>();static void add(int...a){log.add(Arrays.toString(a));}
 static int[]array(String s){return s.equals("~")?null:s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]x=line.split(";",-1);int[]a=array(x[0]);Component c=new Component();c.baseWidth=a[0];c.baseHeight=a[1];c.invMarginX=a[2];c.invMarginY=a[3];c.id=a[14];c.objDrawText=a[13]!=0;c.invOffsetX=array(x[1]);c.invOffsetY=array(x[2]);c.objTypes=array(x[3]);c.objCounts=array(x[4]);c.invSprite=array(x[5]);InterfaceList.clickedInventoryComponent=a[10]>=0?c:null;InterfaceList.mouseOverInventoryObjectIndex=a[10];MiniMenu.itemTargetMode=a[11]>=0?1:0;MiniMenu.selectedObjSlot=a[11];MiniMap.selectedComponentId=a[15];MiniMenu.pressedInventoryComponent=a[12]>=0?c:null;MiniMenu.pressedSlotIndex=a[12];log.clear();String error="null";try{Original.render(c,a[4],a[5],a[6],a[7],a[8],a[9]);}catch(Exception e){error="\""+e.getClass().getSimpleName()+"\"";}System.out.println("[["+String.join(",",log)+"],"+error+"]");}}
 }
 ''',encoding='utf-8')
 p=subprocess.run(['javac','-d',str(O),str(O/'Original.java'),str(O/'Probe.java')],capture_output=True,text=True);(O/'compile.log').write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr
 rng=random.Random(5302320);cases=[];ext=[-2147483648,-2147483647,-33,-32,-1,0,1,31,32,2147483646,2147483647]
 for i in range(2048):
  w=rng.choice([-1,0,1,2,4,5,6]);h=rng.choice([-1,0,1,2,4,6]);n=max(0,w)*max(0,h);cid=rng.choice([0,149<<16,746<<16|21,-2147483648])
  a=[w,h,rng.choice([-33,-32,-31,0,4,8]),rng.choice([-33,-32,-31,0,4,8]),rng.randrange(-100,200),rng.randrange(-100,200),0,0,rng.choice([0,1,31,32,64,200]),rng.choice([0,1,31,32,64,200]),rng.choice([-1,-1,0,max(0,n-1)]),rng.choice([-1,-1,0,max(0,n-1)]),rng.choice([-1,-1,0,max(0,n-1)]),i%2,cid,rng.choice([cid,cid,cid^1])]
  arrays=[[rng.randrange(-5,6)for _ in range(20)],[rng.randrange(-5,6)for _ in range(20)],[rng.choice([-1,0,996,1352,4152])for _ in range(n)],[rng.choice([-2147483648,-1,0,1,1000,2147483647])for _ in range(n)],[rng.choice([-1,0,5])for _ in range(20)]]
  if i%3==0:
   for j in range(2,10):
    if rng.randrange(4)==0:a[j]=rng.choice(ext)
  if i%4==0:
   j=rng.randrange(5);arrays[j]=None if rng.randrange(2)==0 else arrays[j][:rng.randrange(max(1,len(arrays[j])+1))]
  cases.append(dict(arguments=a,arrays=arrays))
 def enc(a):return '~'if a is None else ','.join(map(str,a))
 p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(';'.join([enc(c['arguments']),*[enc(x)for x in c['arrays']]])for c in cases)+'\n',capture_output=True,text=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
 rows=[json.loads(x)for x in p.stdout.splitlines()];assert len(rows)==len(cases)
 for c,row in zip(cases,rows):c['expected']=row
 out=dict(scope='Complete unchanged original type2 body; layer=-1 excludes already separately verified selected edge scroll. Recording available object sprites, background lookup services, no dirty/plugin/framebuffer/native OS lifecycle claim. Compares slot iteration, offsets, clip requests, selected/pressed opacity, actual item sprite arguments and lazy array failure prefixes.',sources={'Cs1ScriptRunner.java':hashlib.sha256(raw).hexdigest()},cases=cases)
 target=R/'fixtures/ui/inventory-render.json';content=json.dumps(out,separators=(',',':'))+'\n'
 if check:assert target.read_text(encoding='utf-8')==content,'Inventory renderer fixture differs'
 else:target.write_text(content,encoding='utf-8')
 print('Original type2 render requests',len(cases),'cases')

if __name__=='__main__':
 import sys
 generate('--check'in sys.argv)
