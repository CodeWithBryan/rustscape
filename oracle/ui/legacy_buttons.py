"""Original legacy8/51/59buttons, special205, magic-varp refresh and timer tick."""
import hashlib,importlib.util,json,pathlib,random,subprocess

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-legacy-buttons-oracle';O.mkdir(exist_ok=True)
    spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    raws={n:(v.SRC/(n+'.java')).read_bytes().replace(b'\r\n',b'\n')for n in['MiniMenu','VarpDomain','Buffer','Protocol']};texts={n:v.clean(b.decode())for n,b in raws.items()}
    branches=''.join(v.method(texts['MiniMenu'],'if (actionCode == '+s+')')for s in['LOGOUT_ACTION','LOGOUT_ACTION_2','UNKNOWN_8'])
    (O/'Original.java').write_text('class Original{static final int LOGOUT_ACTION=51,LOGOUT_ACTION_2=59,UNKNOWN_8=8;static void run(int actionCode,int menuArg2){Component component;int varpIndex;'+branches+'}'+v.method(texts['MiniMenu'],'public static boolean handleSpecialButtonAction(')+'}',encoding='utf-8')
    (O/'VarpDomain.java').write_text('class VarpDomain{static int[]activeVarps;static int mouseButtons,chatEffectsDisabled,inserting;'+v.method(texts['VarpDomain'],'public static void refreshMagicVarp(')+'}',encoding='utf-8')
    (O/'Buffer.java').write_text('class Buffer{byte[]data=new byte[100];int offset;void p1isaac(int n){p1(n);}'+''.join(v.method(texts['Buffer'],s)for s in['public final void p1(', 'public final void p4('])+'}',encoding='utf-8')
    timer=v.method(texts['Protocol'],'if (logoutOnDisconnectTimer > 0)')
    (O/'Probe.java').write_text(r'''
    import java.io.*;import java.util.*;
    class Component{int clientCode;int[][]cs1Scripts;int[]cs1ComparisonOperands;}
    class Protocol{static Buffer outboundBuffer;static int logoutOnDisconnectTimer;static void tick(){TIMER_BODY}}
    class InterfaceList{static Component getComponent(int id){Probe.log.add("[0,"+id+"]");if(Probe.args[2]==-2)throw new IllegalStateException();return Probe.args[2]==-1?null:Probe.component;}static void redrawActiveInterfaces(){Probe.log.add("[1]");if(Probe.args[8]!=0)throw new IllegalStateException();}}
    class AreaSoundManager{static void updateMulti(){Probe.log.add("[2]");if(Probe.args[9]!=0)throw new IllegalStateException();}}
    class VarpType{int clientCode;VarpType(int code){clientCode=code;}}
    class VarpTypeList{static VarpType get(int id){Probe.log.add("[3,"+id+"]");int code=Probe.args[10];if(code==-2)throw new IllegalStateException();return code==-1?null:new VarpType(code);}}
    public class Probe{static int[]args;static Component component;static List<String>log=new ArrayList<>();static int[]array(String s){return s.equals("~")?null:s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
    public static void main(String[]unused)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){if(line.startsWith("T;")){int[]a=array(line.substring(2));Protocol.logoutOnDisconnectTimer=a[0];for(int j=0;j<a[1];j++)Protocol.tick();System.out.println(Protocol.logoutOnDisconnectTimer);continue;}String[]f=line.split(";",-1);args=array(f[0]);component=new Component();component.clientCode=args[3];if(f[1].equals("~")){component.cs1Scripts=null;}else{int colon=f[1].indexOf(":");int count=Integer.parseInt(f[1].substring(0,colon));String[]rows=f[1].substring(colon+1).split("/",-1);component.cs1Scripts=new int[count][];for(int j=0;j<count;j++)component.cs1Scripts[j]=array(rows[j]);}component.cs1ComparisonOperands=array(f[2]);VarpDomain.activeVarps=array(f[3]);Protocol.logoutOnDisconnectTimer=args[4];VarpDomain.mouseButtons=args[5];VarpDomain.chatEffectsDisabled=args[6];VarpDomain.inserting=args[7];Protocol.outboundBuffer=new Buffer();log.clear();String error="null";try{Original.run(args[0],args[1]);}catch(Exception e){error="\""+e.getClass().getSimpleName()+"\"";}System.out.println("[\""+HexFormat.of().formatHex(Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset))+"\","+Arrays.toString(VarpDomain.activeVarps)+","+Protocol.logoutOnDisconnectTimer+","+VarpDomain.mouseButtons+","+VarpDomain.chatEffectsDisabled+","+VarpDomain.inserting+",["+String.join(",",log)+"],"+error+"]");}}}
    '''.replace('TIMER_BODY',timer),encoding='utf-8')
    p=subprocess.run(['javac','-d',str(O),*[str(O/n)for n in['Original.java','VarpDomain.java','Buffer.java','Probe.java']]],text=True,capture_output=True);(O/'compile.log').write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr
    rng=random.Random(5305159);cases=[];ints=[-2147483648,-1,0,1,2,205,2147483647]
    for i in range(2048):
     a=[rng.choice([8,51,59]),rng.choice([0,182<<16|6,-1,-2147483648,2147483647]),rng.choice([-2,-1,0,0,0,0]),rng.choice([-1,0,1,205,206]),*[rng.choice(ints)for _ in range(4)],int(rng.randrange(10)==0),int(rng.randrange(10)==0),rng.choice([-2,-1,0,1,5,6,9])]
     scripts=rng.choice([None,[],[None],[[]],[[4]],[[5]],[[5,-1]],[[5,0]],[[5,1]],[[5,2]],[[5,3]]]);operands=rng.choice([None,[],[0],[1],[-2147483648],[2147483647]]);values=rng.choice([None,[],[0],[1],[-2147483648,2147483647],[0,1,2]])
     if i<96:
      a[0]=[8,51,59][i%3];a[2]=0;a[8]=a[9]=0;scripts=[[5,0]];operands=[ints[i%len(ints)]];values=[ints[(i//3)%len(ints)]];a[10]=[0,5,6,9][(i//3)%4]
     cases.append(dict(arguments=a,scripts=scripts,operands=operands,values=values))
    def arr(a):return '~'if a is None else ','.join(map(str,a))
    def arrays(a):return '~'if a is None else str(len(a))+':'+('/'.join(arr(x)for x in a))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(';'.join([arr(c['arguments']),arrays(c['scripts']),arr(c['operands']),arr(c['values'])])for c in cases)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(x)for x in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    timer_cases=[dict(initial=rng.choice([-2147483648,-1,0,1,2,249,250,2147483647]),ticks=rng.randrange(400))for _ in range(512)]
    p=subprocess.run(['java','-cp',str(O),'Probe'],input=''.join('T;'+str(c['initial'])+','+str(c['ticks'])+'\n'for c in timer_cases),text=True,capture_output=True);(O/'timer-stdout.txt').write_text(p.stdout);(O/'timer-stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(x)for x in p.stdout.splitlines()];assert len(rows)==len(timer_cases)
    for c,value in zip(timer_cases,rows):c['expected']=value
    out=dict(scope='Complete legacy button branches: complete unchanged MiniMenu8/51/59branches and special205handler plus full original refreshMagicVarp with recording component lookup/redraw/ambient/varptype services. Original five-byte no-ISAAC10writer; dynamic array/null/failure boundaries. No original full redraw/ambient implementation, cipher/capacity/wholeMiniMenu/network/logout runtime claim.',sources={n+'.java':hashlib.sha256(b).hexdigest()for n,b in raws.items()},cases=cases,timerCases=timer_cases)
    content=json.dumps(out,separators=(',',':'))+'\n';target=R/'fixtures/ui/legacy-buttons.json'
    if check:assert target.read_text(encoding='utf-8')==content,'Legacy button corpus differs'
    else:target.write_text(content,encoding='utf-8')
    print('Original legacy buttons',len(cases),'cases and',len(timer_cases),'timer sequences',flush=True)

if __name__=='__main__':
    import sys
    generate('--check'in sys.argv)
