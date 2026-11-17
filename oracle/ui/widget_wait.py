"""Original selected legacy widget input/lifecycle observations."""
import hashlib,importlib.util,json,pathlib,random,re,subprocess

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-widget-wait-oracle';O.mkdir(exist_ok=True)
    spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources={}
    def source(name):
     b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    mini=source('MiniMenu');branch=v.method(mini,'if (actionCode == UNKNOWN_41 && Cs1ScriptRunner.pleaseWaitComponent == null)');send=v.method(mini,'public static void sendComponentClickPacket(')
    client=source('ClientProt');close=v.method(client,'public static void closeWidget()')
    (O/'Constants.java').write_text('class ClientProt{static final int CLOSE_MODAL='+re.search(r'public static final int CLOSE_MODAL = (\d+);',client)[1]+';}',encoding='utf-8')
    get=v.method(source('InterfaceList'),'public static Component getComponent(int parentId, int childId)')
    reset=v.method(source('Protocol'),'if (Cs1ScriptRunner.pleaseWaitComponent != null)')
    reset=reset.replace('InterfaceList.redraw(Cs1ScriptRunner.pleaseWaitComponent);','InterfaceList.redraw(Cs1ScriptRunner.pleaseWaitComponent);')
    (O/'Original.java').write_text('class Original{static final int UNKNOWN_41=41;static void action(int actionCode,int menuArg1,int menuArg2){'+branch+'}'+send+close+'static void reset(){'+reset+'}}',encoding='utf-8')
    s=source('Buffer');(O/'Buffer.java').write_text('class Buffer{byte[]data=new byte[4096];int offset;void p1isaac(int n){p1(n);}'+''.join(v.method(s,m)for m in ['public final void p1(','public final void imp4(','public final void ip2('])+'}',encoding='utf-8')
    (O/'InterfaceList.java').write_text('class InterfaceList{static Component parent;static OpenTable openInterfaces=new OpenTable();static Component getComponent(int id){return parent;}'+get+'static void redraw(Component c){}static void closeInterface(boolean unload,ComponentPointer p){openInterfaces.ptr=null;}}',encoding='utf-8')
    (O/'Support.java').write_text(r'''
    import java.io.*;import java.util.*;
    class Component{int color;Component[]createdComponents;}
    class ComponentPointer{int type;}
    class OpenTable{ComponentPointer ptr;ComponentPointer head(){return ptr;}ComponentPointer next(){return null;}}
    class Protocol{static Buffer outboundBuffer;}
    class Cs1ScriptRunner{static Component pleaseWaitComponent;}
    class Probe{
    static Component create(int color){Component c=new Component();c.color=color;return c;}
    public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){String[]p=line.split(";");int pane=Integer.parseInt(p[0]);Protocol.outboundBuffer=new Buffer();Cs1ScriptRunner.pleaseWaitComponent=null;InterfaceList.parent=create(100);InterfaceList.parent.createdComponents=new Component[]{create(101),create(102),create(103)};InterfaceList.openInterfaces.ptr=null;if(pane>=0){InterfaceList.openInterfaces.ptr=new ComponentPointer();InterfaceList.openInterfaces.ptr.type=pane;}List<String>out=new ArrayList<>();for(int i=1;i<p.length;i++){int[]a=Arrays.stream(p[i].split(",")).mapToInt(Integer::parseInt).toArray();int op=a[0],slot=a[1];if(op==0)Original.action(41,slot,65536);if(op==1){if(slot<0){Component n=create(200+i);n.createdComponents=InterfaceList.parent.createdComponents;InterfaceList.parent=n;}else InterfaceList.parent.createdComponents[slot]=create(200+i);}if(op==2)InterfaceList.parent.createdComponents[slot]=null;if(op==3)Original.reset();if(op==4)Original.closeWidget();String hex=HexFormat.of().formatHex(Protocol.outboundBuffer.data,0,Protocol.outboundBuffer.offset);out.add("["+(Cs1ScriptRunner.pleaseWaitComponent==null?-1:Cs1ScriptRunner.pleaseWaitComponent.color)+",\""+hex+"\","+(InterfaceList.openInterfaces.ptr==null?0:1)+"]");}System.out.println("["+String.join(",",out)+"]");}}
    }
    ''',encoding='utf-8')
    with(O/'compile.log').open('w')as log:p=subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],stdout=log,stderr=subprocess.STDOUT)
    assert p.returncode==0,(O/'compile.log').read_text()
    rng=random.Random(53041);cases=[];commands=[]
    for i in range(256):
     steps=[[0,0],[1,0],[0,0],[3,0],[0,0],[2,0],[0,1],[4,0],[0,0],[0,1],[4,0],[0,-1],[1,-1],[0,-1],[3,0],[0,-1]]
     steps += [[rng.randrange(5),rng.choice([-1,0,1,2])]for _ in range(16)]
     for step in steps:
      if step[0]==2 and step[1]<0:step[1]=0
     pane=i%3-1;cases.append(dict(paneKind=pane,steps=steps));commands.append(';'.join([str(pane),*[','.join(map(str,step))for step in steps]]))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    content=json.dumps(dict(scope='Complete original MiniMenu Continue branch/send method, original InterfaceList child lookup, ClientProt.closeWidget and original Protocol waiting-reference reset branch. ISAAC disabled, component-provider and zero/one open-pane iteration/close/redraw are explicit services. Object replacement/removal retains old references; no full callback/profile/bank or UI close-handler claim.',sources=sources,cases=cases),indent=2)+'\n'
    fixture=R/'fixtures/ui/widget-wait.json'
    if check:assert fixture.read_text(encoding='utf-8')==content,'legacy widget fixture differs'
    else:fixture.write_text(content,encoding='utf-8')
    print('PASS original Continue/wait/close',len(cases),'scenarios',sum(len(c['expected'])for c in cases),'snapshots',flush=True)

if __name__=="__main__":
    import argparse
    p=argparse.ArgumentParser();p.add_argument("--check",action="store_true");generate(p.parse_args().check)
