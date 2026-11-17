"""Complete original selected IF1 inventory-icon render/edge-scroll branch."""
import hashlib,importlib.util,json,pathlib,random,subprocess

def generate(check=False):
 R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-inventory-scroll-oracle';O.mkdir(exist_ok=True)
 spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
 raw=(v.SRC/'Cs1ScriptRunner.java').read_bytes().replace(b'\r\n',b'\n');s=v.clean(raw.decode())
 branch=v.method(s,'else if (InterfaceList.clickedInventoryComponent == component && temp1 == InterfaceList.mouseOverInventoryObjectIndex)').replace('else if','if',1)
 (O/'Original.java').write_text('class Original{static void render(Component component,int temp1,int x,int y,int layer,Component[]components,Sprite sprite){'+branch+'}}')
 (O/'Probe.java').write_text(r'''
import java.io.*;import java.util.*;
class Component{int scrollY,scrollMaxV,height;}
class Mouse{static int lastMouseX,lastMouseY;}
class Protocol{static int sceneDelta;}
class GlRenderer{static boolean enabled;}
class GlRaster{static int clipTop,clipBottom;}
class SoftwareRaster{static int clipTop,clipBottom;}
class InterfaceList{static Component clickedInventoryComponent;static int clickedInventoryComponentX,clickedInventoryComponentY,clickedInventoryComponentCycle,mouseOverInventoryObjectIndex;static void redraw(Component c){Probe.add(1,c.scrollY,clickedInventoryComponentY);}}
class Sprite{void renderAlpha(int x,int y,int a){Probe.add(0,x,y,a);}}
public class Probe{
static List<String>log=new ArrayList<>();static void add(int...a){log.add(Arrays.toString(a));}
public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){int[]a=Arrays.stream(line.split(",")).mapToInt(Integer::parseInt).toArray();Component c=new Component(),p=new Component();p.scrollY=a[0];p.scrollMaxV=a[1];p.height=a[2];InterfaceList.clickedInventoryComponentX=a[3];InterfaceList.clickedInventoryComponentY=a[4];Mouse.lastMouseX=a[5];Mouse.lastMouseY=a[6];InterfaceList.clickedInventoryComponentCycle=a[7];Protocol.sceneDelta=a[8];GlRaster.clipTop=a[9];GlRaster.clipBottom=a[10];SoftwareRaster.clipTop=a[11];SoftwareRaster.clipBottom=a[12];GlRenderer.enabled=a[13]!=0;InterfaceList.clickedInventoryComponent=a[14]!=0?c:null;InterfaceList.mouseOverInventoryObjectIndex=a[15];Component[]parents=a[16]==2?new Component[0]:new Component[]{a[16]==1?null:p};log.clear();String error="null";try{Original.render(c,a[17],a[18],a[19],a[16]==3?-1:65536,parents,new Sprite());}catch(Exception e){error="\""+e.getClass().getSimpleName()+"\"";}System.out.println("[["+String.join(",",log)+"],"+p.scrollY+","+InterfaceList.clickedInventoryComponentX+","+InterfaceList.clickedInventoryComponentY+","+error+"]");}}
}
''')
 p=subprocess.run(['javac','-d',str(O),str(O/'Original.java'),str(O/'Probe.java')],capture_output=True,text=True);(O/'compile.log').write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr
 rng=random.Random(53012832);cases=[]
 extremes=[-2147483648,-2147483647,-32,-5,-4,0,4,5,32,2147483646,2147483647]
 for i in range(4096):
  y=rng.randrange(-100,500);press=[rng.randrange(-100,500),rng.randrange(-100,500)]
  a=[rng.choice([0,1,10,100,500,2147483647,-2147483648]),rng.choice([100,500,1000,2147483647,-2147483648]),rng.choice([16,32,100,360,2147483647]),*press,press[0]+rng.choice([-6,-5,-4,0,4,5,6]),press[1]+rng.choice([-300,-32,-6,-5,-4,0,4,5,6,32,300]),rng.choice([0,1,4,5,6,100]),rng.choice([0,1,2,5,2147483647,-2147483648]),y,y+rng.choice([0,1,16,31,32,100,360]),y-10,y+rng.choice([10,20,32,500]),int(i%2==0),int(i%11!=0),0,rng.choice([0,0,0,1,2,3]),int(i%13==0),rng.randrange(-100,500),y+rng.choice([-300,-32,-16,0,16,32,300])]
  if i%4==0:
   for j in [0,1,2,3,4,5,6,7,8,9,10,11,12,18,19]:
    if rng.randrange(4)==0:a[j]=rng.choice(extremes)
  cases.append(dict(arguments=a))
 p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(','.join(map(str,c['arguments']))for c in cases)+'\n',capture_output=True,text=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
 rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==4096
 for c,row in zip(cases,rows):c['expected']=row
 text=json.dumps(dict(scope='Complete unchanged original selected-inventory icon render branch, including independent upper/lower edge scroll, per-axis deadzone, five-cycle presentation delay, parent array/null failures and completed draw/redraw prefixes. Sprite/raster/mouse/redraw are recording services. Does not establish sprite pixels, whole input lifecycle, IF3 scripting or native OS mouse behavior.',sources={'Cs1ScriptRunner.java':hashlib.sha256(raw).hexdigest()},cases=cases),separators=(',',':'))+'\n'
 target=R/'fixtures/ui/inventory-scroll.json'
 if check:assert target.read_text()==text,'Inventory scroll fixture differs'
 else:target.write_text(text)
 print('PASS original selected inventory draw and edge scroll',len(cases),'cases',flush=True)

if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');generate(p.parse_args().check)
