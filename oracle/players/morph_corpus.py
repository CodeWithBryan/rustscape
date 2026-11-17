"""Unchanged NpcType variable selection and validity, with explicit varp services."""
import hashlib,json,random,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4/NpcType.java';raw=src.read_bytes().replace(b'\r\n',b'\n');code=clean(raw.decode());run=root/'runs/players-oracle/morph';run.mkdir(parents=True,exist_ok=True)
    methods=method(code,'public final NpcType getMultiNpc(')+method(code,'public final boolean isMultiNpcValid(')
    support=r'''import java.io.*;import java.util.*;
class NpcType {int id,multiNpcVarbit=-1,multiNpcVarp=-1;int[]multiNpcs; METHODS }
class NpcTypeList {static NpcType get(int id){NpcType n=new NpcType();n.id=id;return n;}}
class VarpDomain {static int[]activeVarps=new int[32];static int value;static List<Integer>queries=new ArrayList<>();static int getVarbit(int id){queries.add(id);return value;}}
public class Probe {public static void main(String[]args)throws Exception {BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]a=line.split(";",-1);NpcType n=new NpcType();n.multiNpcVarbit=Integer.parseInt(a[0]);n.multiNpcVarp=Integer.parseInt(a[1]);VarpDomain.value=Integer.parseInt(a[2]);Arrays.fill(VarpDomain.activeVarps,Integer.parseInt(a[3]));VarpDomain.queries.clear();if(!a[4].equals("null"))n.multiNpcs=Arrays.stream(a[4].split(",")).mapToInt(Integer::parseInt).toArray();NpcType selected=n.multiNpcs==null?null:n.getMultiNpc();boolean valid=n.isMultiNpcValid();System.out.println("["+(selected==null?"null":selected.id)+","+valid+","+VarpDomain.queries+"]");}}}
'''
    (run/'Probe.java').write_text(support.replace('METHODS',methods),encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(5303384);cases=[];lines=[]
    for i in range(1024):
        children=None if i%11==0 else [rng.choice([-1,0,1,3385,7969])for _ in range(rng.randrange(2,12))]
        b=rng.choice([-1,0,7,31]);v=rng.choice([-1,0,3,31]);bv=rng.choice([-2147483648,-1,0,1,4,8,11,2147483647]);vv=rng.choice([-1,0,1,4,10,2147483647])
        cases.append(dict(varbit=b,varp=v,bit_value=bv,varp_value=vv,children=children));lines.append(';'.join(map(str,[b,v,bv,vv,'null'if children is None else','.join(map(str,children))])))
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(lines)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(cases)
    for c,line in zip(cases,output):c['expected']=json.loads(line)
    content=json.dumps(dict(sources={'NpcType':hashlib.sha256(raw).hexdigest()},bindings='Unchanged getMultiNpc/isMultiNpcValid; original varbit callback observations and explicit active-varp array values. Does not establish model assembly or recursive asset availability.',cases=cases),separators=(',',':')).encode()+b'\n';path=root/'fixtures/players/morph.json'
    if check:
        if path.read_bytes()!=content:raise SystemExit('NPC morph corpus differs')
    else:path.write_bytes(content)
    print(f'Original NPC morph selection/validity: {len(cases)} cases')
