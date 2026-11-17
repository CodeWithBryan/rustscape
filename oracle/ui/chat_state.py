"""Original Chat.add history and InterfaceList chat notification branch."""
import hashlib,json,pathlib,random,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/chat-state';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    chat=source('Chat');interface=source('InterfaceList')
    (run/'Chat.java').write_text(chat,encoding='utf-8')
    branch=method(interface,'if (Chat.transmitAt > component.lastTransmitTimer && component.onMsg != null)')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class JagString{String hex;JagString(String s){hex=s;}}
class Component{int lastTransmitTimer=-1;Object[] onMsg=new Object[]{1};}class HookRequest{Component source;Object[] arguments;}
class Requests{int count;void addTail(Object o){count++;}}
class InterfaceList{static int transmitTimer=1;static Requests lowPriorityRequests=new Requests();static void notifyChat(Component component){HookRequest request;'''+branch+'''} }
public class Probe{static String text(JagString s){return s==null?"":s.hex;}public static void main(String[] args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[] x=line.split(";",-1);if(x[0].equals("R")){Arrays.fill(Chat.types,0);Arrays.fill(Chat.phraseIds,0);Arrays.fill(Chat.names,null);Arrays.fill(Chat.messages,null);Arrays.fill(Chat.clans,null);Chat.size=Integer.parseInt(x[1]);Chat.transmitAt=0;InterfaceList.transmitTimer=Integer.parseInt(x[2]);component=new Component();continue;}if(x[0].equals("A"))Chat.add(Integer.parseInt(x[1]),Integer.parseInt(x[2]),new JagString(x[3]),new JagString(x[4]),new JagString(x[5]));if(!x[0].equals("D")){InterfaceList.lowPriorityRequests.count=0;InterfaceList.notifyChat(component);component.lastTransmitTimer=InterfaceList.transmitTimer;InterfaceList.transmitTimer++;System.out.println(InterfaceList.lowPriorityRequests.count);continue;}String out=Chat.size+";"+Chat.transmitAt+";"+InterfaceList.transmitTimer+";"+component.lastTransmitTimer;for(int i=0;i<100;i++)out+="|"+Chat.types[i]+","+Chat.phraseIds[i]+","+text(Chat.messages[i])+","+text(Chat.clans[i])+","+text(Chat.names[i]);System.out.println(out);}}static Component component;}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[];inputs=[]
    for _ in range(64):
        size=rng.choice([0,2147483640,-10]);timer=rng.choice([1,2147483640,-2147483648]);steps=[];inputs.append(f'R;{size};{timer}')
        for _ in range(rng.randrange(120,200)):
            if rng.randrange(5):
                c=dict(kind=rng.randrange(16),phrase=rng.randrange(-1,10000),text=bytes(rng.randrange(256)for _ in range(rng.randrange(30))).hex(),clan=bytes(rng.randrange(1,256)for _ in range(rng.randrange(10))).hex(),name=bytes(rng.randrange(1,256)for _ in range(rng.randrange(10))).hex());steps.append(c);inputs.append(';'.join(map(str,['A',c['phrase'],c['kind'],c['text'],c['clan'],c['name']])))
            else:steps.append(None);inputs.append('N')
        cases.append(dict(size=size,timer=timer,steps=steps));inputs.append('D')
    result=iter(subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines())
    for c in cases:c['hooks']=[int(next(result))for _ in c['steps']];c['expected']=next(result)
    assert next(result,None)is None
    content=json.dumps(dict(sources=hashes,bindings='Normalized nullable text reads; controlled transmit clocks, message inputs and one onMsg component; queue ordering against other event sources remains unproven',cases=cases),separators=(',',':')).encode()+b'\n';p=root/'fixtures/ui/chat-state.json'
    if check:
        if p.read_bytes()!=content:raise SystemExit('UI chat history corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 chat history/notification: {len(cases)} scenarios')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
