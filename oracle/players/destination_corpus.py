"""Original clear153 branch, including retained mapFlagY and opcode reset."""
import hashlib,json,random,re,subprocess
def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/destination';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    p=source('Protocol');server=source('ServerProt');source('LoginManager')
    branch=method(p,'else if (opcode == ServerProt.CLEAR_MINIMAP_FLAG)')
    constant=re.search(r'public static final int CLEAR_MINIMAP_FLAG = \d+;',server).group(0)
    code='''import java.io.*;class LoginManager{static int mapFlagX,mapFlagY;}class ServerProt{ CONSTANT }public class Probe{static int opcode;static boolean read(){if(false){} BRANCH return false;}public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String l;while((l=r.readLine())!=null){String[]a=l.split(",");LoginManager.mapFlagX=Integer.parseInt(a[0]);LoginManager.mapFlagY=Integer.parseInt(a[1]);opcode=Integer.parseInt(a[2]);boolean ok=read();System.out.println("["+LoginManager.mapFlagX+","+LoginManager.mapFlagY+","+opcode+","+ok+"]");}}}'''.replace('CONSTANT',constant).replace('BRANCH',branch)
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(530153);cases=[dict(flag=[rng.randrange(-2**31,2**31),rng.randrange(-2**31,2**31)],opcode=153 if i%3 else 0)for i in range(128)]
    lines=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(','.join(map(str,[*c['flag'],c['opcode']]))for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines()
    for c,line in zip(cases,lines):c['expected']=json.loads(line)
    path=root/'fixtures/players/destination.json';content=json.dumps(dict(sources=hashes,bindings='Unchanged original Protocol CLEAR_MINIMAP_FLAG branch. Explicit initial flag/opcode assignments; retains mapFlagY. Malformed payloads are rejected by the bounded native frame adapter.',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Destination flag corpus differs')
    else:path.write_bytes(content)
    print('Original clear-minimap-flag reducer: 128 cases')
