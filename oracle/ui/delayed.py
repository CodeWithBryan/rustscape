"""Unchanged intrusive queues and varp timers with a controlled monotonic clock."""
import hashlib, json, pathlib, random, subprocess

def generate(root, clean, method, check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/delayed';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    for name in ['DelayedStateChange','Node','SecondaryNode','SecondaryLinkedList','HashTable','LongNode']:(run/(name+'.java')).write_text(source(name),encoding='utf-8')
    varps=source('VarpDomain')
    (run/'VarpDomain.java').write_text('class VarpDomain {static int[] activeVarps=new int[3000],varp=new int[3000];static HashTable pendingVarpTimers=new HashTable(16);'+''.join(method(varps,s)for s in ['public static void set(', 'public static void setVarp(', 'public static int poll('])+'}',encoding='utf-8')
    (run/'Probe.java').write_text('''import java.io.*;
class MonotonicClock {static long now;static long currentTimeMillis(){return now;}}
class PluginRepository {static void OnVarpUpdate(int id,int value){}}
class JagString {}
public class Probe {public static void main(String[] args)throws Exception {BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){DelayedStateChange.clear();VarpDomain.pendingVarpTimers=new HashTable(16);VarpDomain.activeVarps=new int[3000];VarpDomain.varp=new int[3000];MonotonicClock.now=1000;StringBuilder out=new StringBuilder();for(String command:line.split(";")){String[] c=command.split(",");int a=c.length>1?Integer.parseInt(c[1]):0,b=c.length>2?Integer.parseInt(c[2]):0;switch(c[0]){case "T":MonotonicClock.now+=a;break;case "C":DelayedStateChange.create(a,b).pushClient();break;case "S":DelayedStateChange d=DelayedStateChange.create(a,b);d.pushServer();d.intArg1=Integer.parseInt(c[3]);break;case "V":VarpDomain.set(b,a);break;case "L":VarpDomain.setVarp(a,b);break;case "P":out.append("[");for(DelayedStateChange x=DelayedStateChange.poll();x!=null;x=DelayedStateChange.poll())out.append(x.getType()+":"+x.getId()+":"+x.intArg1+",");out.append("|");for(int id=VarpDomain.poll(true);id!=-1;id=VarpDomain.poll(false))out.append(id+":"+VarpDomain.activeVarps[id]+",");out.append("]");break;}}for(int i=0;i<100;i++)out.append(VarpDomain.activeVarps[i]+":"+VarpDomain.varp[i]+",");System.out.println(out);}}}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[]
    for _ in range(256):
        commands=[]
        for _ in range(128):
            op=rng.choice(['T','C','S','V','L','P','P']);id=rng.randrange(0,80)
            if op=='T':c=[op,rng.choice([0,1,2,499,500,501,600])]
            elif op in ['C','S']:c=[op,rng.choice([1,2,3,7]),rng.choice([id,id,-1,-2147483648])]+([rng.randrange(-2147483648,2147483648)]if op=='S'else[])
            elif op in ['V','L']:c=[op,id,rng.randrange(-2147483648,2147483648)]
            else:c=[op]
            commands.append(c)
        commands += [['T',501],['P']];cases.append(dict(commands=commands))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(';'.join(','.join(map(str,c))for c in case['commands'])for case in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines()
    assert len(result)==len(cases)
    for case,line in zip(cases,result):case['expected']=line
    content=json.dumps(dict(sources=hashes,clock='Controlled monotonic milliseconds; no plugin callbacks',cases=cases),separators=(',',':')).encode()+b'\n';p=root/'fixtures/ui/delayed.json'
    if check:
        if p.read_bytes()!=content:raise SystemExit('delayed UI corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 delayed queues/varp timers: {len(cases)} scenarios')

if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    generate(root,v.clean,v.method,'--check'in sys.argv)
