"""Original selected legacy widget input/lifecycle observations."""
import hashlib,importlib.util,json,pathlib,random,subprocess

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-legacy-widget-oracle';O.mkdir(exist_ok=True)
    spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources={}
    def source(name):
     b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    s=source('InterfaceList');method=v.method(s,'public static void processComponents(')
    for signature in ['if (component == Cs1ScriptRunner.draggedComponent)','if (component.noClickThrough && Mouse.lastMouseX','if (component.if3)']:
     block=v.method(method,signature);assert block in method;method=method.replace(block,'',1)
    draw=source('Cs1ScriptRunner');hidden=v.method(draw,'if (!component.if3 && InterfaceList.isHidden(component) && InterfaceList.hoveredComponent != component)')
    hidden=hidden.replace('continue;','return false;')
    code='class Original{static Component hoveredComponent,clickedInventoryComponent;static OpenTable openInterfaces=new OpenTable();static Properties getServerActiveProperties(Component c){return new Properties();}static boolean isHidden(Component c){return c.hidden;}static void handleScrollbar(int a,int b,Component c,int d,int e,int f,int g){}static void processSubInterface(int a,int b,int c,int d,int e,int f,int g){throw new AssertionError("subinterface service not included in this corpus");}'+method+'static boolean drawEligible(Component component){if(!component.if3 || !InterfaceList.isHidden(component)){if(component.type==0){'+hidden+'}return true;}return false;}}'
    (O/'Original.java').write_text(code,encoding='utf-8')
    (O/'Support.java').write_text(r'''
    import java.io.*;import java.util.*;
    class Component{int id,overlayer,type,x,y,width,height,clientCode,hoverOverlayer=-1,overColor,scrollMaxV,scrollX,scrollY;boolean if3,hidden,hasEventHandlers,noClickThrough;Component[]createdComponents;}
    class InterfaceList{static boolean isHidden(Component c){return c.hidden;}static Component hoveredComponent;}
    class Properties{int events;}
    class ComponentPointer{int interfaceId;}
    class OpenTable{ComponentPointer get(long key){return null;}}
    class Mouse{static int lastMouseX,lastMouseY;}
    class Protocol{static Component tooltipComponent;}
    class Cs1ScriptRunner{static Component draggedComponent,dragParentComponent;static boolean isMenuOpen;}
    class Probe{
    static List<Integer>visible=new ArrayList<>();static void visible(Component[]children,int parent){for(Component c:children)if(c!=null && c.overlayer==parent){InterfaceList.hoveredComponent=Original.hoveredComponent;if(Original.drawEligible(c)){visible.add(c.id);if(c.type==0)visible(children,c.id);}}}
    static Component copy(Component c){Component n=new Component();n.id=c.id;n.overlayer=c.overlayer;n.type=c.type;n.x=c.x;n.y=c.y;n.width=c.width;n.height=c.height;n.if3=c.if3;n.hidden=c.hidden;n.hoverOverlayer=c.hoverOverlayer;n.overColor=c.overColor;return n;}
    public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){String[]p=line.split(";");int[]h=Arrays.stream(p[0].split(",")).mapToInt(Integer::parseInt).toArray();Mouse.lastMouseX=h[0];Mouse.lastMouseY=h[1];Cs1ScriptRunner.draggedComponent=h[2]!=0?new Component():null;Original.clickedInventoryComponent=h[3]!=0?new Component():null;Cs1ScriptRunner.isMenuOpen=h[4]!=0;Component[]children=new Component[p.length-1];for(int i=1;i<p.length;i++){int[]a=Arrays.stream(p[i].split(",")).mapToInt(Integer::parseInt).toArray();Component c=new Component();c.id=65536+i-1;c.overlayer=a[0]<0?-1:65536+a[0];c.x=a[1];c.y=a[2];c.width=a[3];c.height=a[4];c.type=a[5];c.if3=a[6]!=0;c.hidden=a[7]!=0;c.hoverOverlayer=a[8];c.overColor=a[9];children[i-1]=c;}Original.hoveredComponent=null;InterfaceList.hoveredComponent=null;Original.processComponents(children,-1,0,0,400,300,0,0);if(h[5]>=0)children[h[5]]=copy(children[h[5]]);int current=-1;for(int i=0;i<children.length;i++)if(children[i]==Original.hoveredComponent)current=i;visible.clear();visible(children,-1);System.out.println("["+current+","+visible.toString()+"]");}}
    }
    ''',encoding='utf-8')
    # The original method refers to its own hoveredComponent; renderer's class alias
    # is updated immediately before each original draw-eligibility observation.
    with(O/'compile.log').open('w')as log:p=subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],stdout=log,stderr=subprocess.STDOUT)
    assert p.returncode==0,(O/'compile.log').read_text()
    rng=random.Random(5301044);cases=[];commands=[]
    for i in range(1024):
     point=[rng.choice([-1,0,5,10,20,30,39,40,50,60,70,100,399,400]),rng.choice([-1,0,5,10,20,30,39,40,50,60,70,100,299,300])]
     flags=[int(i%11==0),int(i%19==0),int(i%23==0)];replace=rng.choice([-1,-1,-1,1,2,3,4,5])
     nodes=[[-1,0,0,400,300,0,int(i%7!=0),int(i%31==0),-1,0],
     [0,10,10,40,30,rng.choice([2,3,4,5]),int(i%9==0),int(i%5==0),rng.choice([-1,2,3]),rng.choice([0,1,0xff0000])],
     [0,20,20,50,40,rng.choice([2,3,4,5]),int(i%13==0),int(i%3==0),rng.choice([-1,1,3]),rng.choice([0,0,1])],
     [0,10,10,100,100,0,int(i%5==0),int(i%3!=0),-1,0],
     [3,0,0,50,40,4,int(i%17==0),int(i%2==0),-1,rng.choice([0,0,1])],
     [3,10,20,40,30,4,int(i%19==0),int(i%3==0),-1,0]]
     case=dict(point=point,dragged=bool(flags[0]),inventoryClicked=bool(flags[1]),menuOpen=bool(flags[2]),replaceAfterInput=replace,nodes=nodes);cases.append(case);commands.append(';'.join([','.join(map(str,[*point,*flags,replace])),*[','.join(map(str,n))for n in nodes]]))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    content=json.dumps(dict(scope='Original static-interface processComponents traversal, geometry, hidden-container and legacy hover selection with unchanged original branches. IF3 event service and noClickThrough queue/drag service are excluded; all corpus noClickThrough=false. No subinterfaces, created children, scrollbar or tooltip behavior claim. Hidden draw eligibility uses original conditions; replacement checks original object identity.',sources=sources,cases=cases),indent=2)+'\n'
    fixture=R/'fixtures/ui/legacy-widgets.json'
    if check:assert fixture.read_text(encoding='utf-8')==content,'legacy widget fixture differs'
    else:fixture.write_text(content,encoding='utf-8')
    print('PASS original static legacy hover/hidden/object identity',len(cases),'draft scenarios',flush=True)

if __name__=="__main__":
    import argparse
    p=argparse.ArgumentParser();p.add_argument("--check",action="store_true");generate(p.parse_args().check)
