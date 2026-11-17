"""Complete original button callback/reference/wire order under explicit services."""
import hashlib,importlib.util,json,pathlib,random,subprocess

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-button-click-oracle';O.mkdir(exist_ok=True)
    spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources={}
    def source(name):
        b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    (O/'Original.java').write_text('class Original{'+v.method(source('ClientProt'),'public static void sendButtonClick(')+'}',encoding='utf-8')
    s=source('Buffer');(O/'Buffer.java').write_text('class Buffer{byte[]data=new byte[4096];int offset;void p1isaac(int n){p1(n);}'+''.join(v.method(s,m)for m in ['public final void p1(','public final void p4(','public final void p2('])+'}',encoding='utf-8')
    s=source('ServerActiveProperties');(O/'ServerActiveProperties.java').write_text('class ServerActiveProperties{int events;ServerActiveProperties(int n){events=n;}'+v.method(s,'public final boolean isButtonEnabled(')+'}',encoding='utf-8')
    get=v.method(source('InterfaceList'),'public static Component getComponent(int parentId, int childId)')
    (O/'InterfaceList.java').write_text('class InterfaceList{static Component root;static Integer override;static int slot;static Component getComponent(int id){return root;}'+get+'static ServerActiveProperties getServerActiveProperties(Component c){return override==null?c.properties:new ServerActiveProperties(override);}static void replace(Component c){if(slot<0)root=c;else root.createdComponents[slot]=c;}static Component current(){return getComponent(65536,slot);}}',encoding='utf-8')
    (O/'Support.java').write_text(r'''
import java.io.*;import java.util.*;
class Component{int color,clientCode;Object[]onOptionClick;ServerActiveProperties properties;Component[]createdComponents;}
class JagString{String value;JagString(String s){value=s;}}
class HookRequest{Object[]arguments;Component source;JagString opBase;int op;}
class Protocol{static Buffer outboundBuffer;}
class Services{static int[]a;static List<String>events=new ArrayList<>();static Component lastSource;static Component create(int color,int flags,int code){Component c=new Component();c.color=color;c.properties=new ServerActiveProperties(flags);c.clientCode=code;return c;}static String fields(Component c){return c==null?"null":"["+c.color+","+c.properties.events+","+c.clientCode+"]";}}
class ScriptRunner{static void run(HookRequest r){Services.events.add("\"hook:"+r.source.color+":"+r.op+":"+r.opBase.value+"\"");Services.lastSource=r.source;int[]a=Services.a;switch(a[7]){case 1:InterfaceList.replace(null);break;case 2:InterfaceList.replace(Services.create(22,a[8],a[9]));break;case 3:r.source.properties=new ServerActiveProperties(a[8]);break;case 4:r.source.clientCode=a[9];break;case 5:InterfaceList.override=a[10]!=0?a[11]:null;break;case 6:InterfaceList.replace(null);InterfaceList.override=null;Protocol.outboundBuffer.p1isaac(184);break;case 7:throw new RuntimeException("hook");case 8:InterfaceList.replace(Services.create(22,a[8],0));Original.sendButtonClick(new JagString("nested"),InterfaceList.slot,2,65536);break;}}}
class MiniMenu{static boolean handleSpecialButtonAction(Component c){Services.events.add("\"special:"+c.color+":"+c.clientCode+"\"");if(Services.a[12]==2)throw new RuntimeException("special");return Services.a[12]==0;}}
class Probe{public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){int[]a=Arrays.stream(line.split(",")).mapToInt(Integer::parseInt).toArray();Services.a=a;Services.events.clear();Services.lastSource=null;Protocol.outboundBuffer=new Buffer();InterfaceList.slot=a[13];Component c=a[0]!=0?Services.create(11,a[3],a[4]):null;if(c!=null&&a[1]!=0)c.onOptionClick=new Object[]{1};if(a[13]<0)InterfaceList.root=c;else{InterfaceList.root=Services.create(0,0,0);InterfaceList.root.createdComponents=new Component[]{c};}InterfaceList.override=a[5]!=0?a[6]:null;String error="null";try{Original.sendButtonClick(new JagString("outer"),a[13],a[2],65536);}catch(RuntimeException e){error="\""+e.getMessage()+"\"";}String hex=HexFormat.of().formatHex(Protocol.outboundBuffer.data,0,Protocol.outboundBuffer.offset);System.out.println("{\"error\":"+error+",\"wire\":\""+hex+"\",\"events\":["+String.join(",",Services.events)+"],\"current\":"+Services.fields(InterfaceList.current())+",\"source\":"+Services.fields(Services.lastSource)+",\"override\":"+InterfaceList.override+"}");}}}
''',encoding='utf-8')
    with(O/'compile.log').open('w')as log:p=subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],stdout=log,stderr=subprocess.STDOUT)
    assert p.returncode==0,(O/'compile.log').read_text()
    rng=random.Random(530155);cases=[];commands=[]
    for i in range(768):
        op=rng.choice([-1,0,1,2,3,4,5,6,7,8,9,10,11,32]);flags=lambda:rng.choice([0,-1,rng.randrange(-2147483648,2147483648),1<<(op&31)if(op&31)!=31 else -2147483648])
        a=[int(i%29!=0),int(i%5!=0),op,flags(),rng.choice([0,0,205]),int(i%3==0),flags(),i%9,flags(),rng.choice([0,205,324]),int(i%2==0),flags(),rng.randrange(3),-1 if i%2 else 0]
        cases.append(dict(arguments=a));commands.append(','.join(map(str,a)))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    content=json.dumps(dict(scope='Complete unchanged ClientProt.sendButtonClick including callback-before-gating, captured object references, reentry and wire order. Original child lookup, button mask and Buffer writes; ISAAC-disabled component/provider, hook-execution and special-button services are explicit adapters, including uncaught service failures. Does not port every special clientCode handler or full CS2 callback implementation.',sources=sources,cases=cases),indent=2)+'\n';fixture=R/'fixtures/ui/button-clicks.json'
    if check:assert fixture.read_text(encoding='utf-8')==content,'button click fixture differs'
    else:fixture.write_text(content,encoding='utf-8')
    print('PASS complete original button callback/reference/wire routing',len(cases),'cases',flush=True)

if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');generate(p.parse_args().check)
