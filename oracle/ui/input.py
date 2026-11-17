"""Original inventory reorder branch and IF3 drag-release serialization."""
import hashlib, json, pathlib, random, subprocess
def generate(root, clean, method, check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/input';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    buffer=source('Buffer');(run/'Buffer.java').write_text('class Buffer {byte[] data=new byte[1000];int offset;void p1isaac(int n){p1(n);}'+''.join(method(buffer,s)for s in ['public final void p1(', 'public final void p1sub(', 'public final void p2(', 'public final void p2add(', 'public final void ip2(', 'public final void p4(', 'public final void ip4(', 'public final void mp4('])+'}',encoding='utf-8')
    props=source('ServerActiveProperties').replace(' extends Node','');(run/'ServerActiveProperties.java').write_text(props,encoding='utf-8')
    reorder=method(source('Protocol'),'if (InterfaceList.clickedInventoryComponent == InterfaceList.mouseOverInventoryInterface &&')
    release=method(source('Cs1ScriptRunner'),'if (InterfaceList.dragTargetComponent != null && InterfaceList.getDragRenderParent(draggedComponent) != null)')
    swap=method(source('Component'),'public final void swapObjs(')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class Component {int id,createdComponentId,clientCode;int[] objTypes,objCounts;ServerActiveProperties properties;'''+swap+'''}
class InterfaceList {static Component clickedInventoryComponent,mouseOverInventoryInterface,dragTargetComponent;static int mouseOverInventoryObjectIndex;static boolean validParent;static ServerActiveProperties getServerActiveProperties(Component c){return c.properties;}static Component getDragRenderParent(Component c){return validParent?c:null;}}
class MiniMenu {static int clickedInventoryIndex;}class VarpDomain {static int inserting;}class Protocol {static Buffer outboundBuffer;}
public class Probe {static Buffer outboundBuffer;static Component draggedComponent;
 static void reorder(){Component component;'''+reorder+'''}static void release(){'''+release+'''}
 public static void main(String[]args)throws Exception {BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[] x=line.split(";");int[] a=Arrays.stream(x[0].split(",")).mapToInt(Integer::parseInt).toArray();outboundBuffer=Protocol.outboundBuffer=new Buffer();Component c=new Component();c.id=a[0];c.clientCode=a[1];c.properties=new ServerActiveProperties(a[2],-1);c.objTypes=Arrays.stream(x[1].split(",")).mapToInt(Integer::parseInt).toArray();c.objCounts=Arrays.stream(x[2].split(",")).mapToInt(Integer::parseInt).toArray();MiniMenu.clickedInventoryIndex=a[3];InterfaceList.mouseOverInventoryObjectIndex=a[4];VarpDomain.inserting=a[5];InterfaceList.clickedInventoryComponent=InterfaceList.mouseOverInventoryInterface=c;reorder();String out=Arrays.toString(c.objTypes)+";"+Arrays.toString(c.objCounts)+";"+HexFormat.of().formatHex(Arrays.copyOf(outboundBuffer.data,outboundBuffer.offset));Protocol.outboundBuffer=new Buffer();draggedComponent=c;c.createdComponentId=a[6];Component target=new Component();target.id=a[7];target.createdComponentId=a[8];InterfaceList.dragTargetComponent=target;InterfaceList.validParent=a[9]!=0;release();out+=";"+HexFormat.of().formatHex(Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset));System.out.println(out);}}
}''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[]
    for _ in range(512):
        n=rng.randrange(2,40);a,b=rng.sample(range(n),2)
        cases.append(dict(args=[rng.randrange(-2147483648,2147483648),rng.choice([0,206]),rng.choice([0,1<<28,1<<29,(3<<28)]),a,b,rng.choice([0,1,2]),rng.randrange(-1,10000),rng.randrange(-2147483648,2147483648),rng.randrange(-1,10000),rng.randrange(2)],ids=[rng.choice([-1,0,1,996,4152])for _ in range(n)],counts=[rng.randrange(-2147483648,2147483648)for _ in range(n)]))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(';'.join(','.join(map(str,c[k]))for k in ['args','ids','counts'])for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines()
    assert len(result)==len(cases)
    for case,line in zip(cases,result):
        ids,counts,reorder,release=line.split(';');case.update(expected_ids=json.loads(ids),expected_counts=json.loads(counts),reorder=reorder,release=release)
    p=root/'fixtures/ui/input.json';content=json.dumps(dict(sources=hashes,bindings='Controlled component identity, slots and drag-parent availability; no server container mutations',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if p.read_bytes()!=content:raise SystemExit('UI input corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 inventory reorder/IF3 drag wires: {len(cases)} cases')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
