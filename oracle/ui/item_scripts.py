"""Original CS2 item metadata branches over explicit resolved-item service values."""
import hashlib,json,pathlib,random,subprocess

def generate(root,clean,method,check=False):
    source=root/'reference/client/client/src/main/java/rt4/ScriptRunner.java';raw=source.read_bytes().replace(b'\r\n',b'\n');runner=clean(raw.decode());run=root/'runs/ui-item-scripts';run.mkdir(exist_ok=True)
    names=['getItemName','getItemGroundOption','getItemOption','getItemValue','itemIsStackable','getNotedItem','getRealItem','itemIsMembers']
    blocks=[method(runner,f'if (opcode == Cs2Opcodes.{name})').replace('continue;','return;')for name in names]
    code='''import java.io.*;import java.util.*;import java.nio.charset.StandardCharsets;
class JagString{String value;JagString(String s){value=s;}}
class ObjType{JagString name;JagString[] ops,iops;int cost,stackable,certtemplate,certlink;boolean members;}
class ObjTypeList{static ObjType item;static ObjType get(int id){return item;}}
public class Probe{static int[] intStack=new int[10];static JagString[] stringStack=new JagString[10];static int isp,ssp;static final JagString EMPTY_STRING=new JagString("");
static class Cs2Opcodes{'''+''.join(f'static final int {name}={4200+i};'for i,name in enumerate(names))+'''}
static void apply(int opcode){int int1,int3;ObjType itemType,certType;'''+''.join(blocks)+'''throw new IllegalArgumentException();}
static JagString text(String s){return s.equals("-")?null:new JagString(new String(HexFormat.of().parseHex(s),StandardCharsets.ISO_8859_1));}
static JagString[] options(String s){return Arrays.stream(s.split(",",-1)).map(Probe::text).toArray(JagString[]::new);}
public static void main(String[]args)throws Exception{BufferedReader input=new BufferedReader(new InputStreamReader(System.in));String line;while((line=input.readLine())!=null){String[]x=line.split(";",-1);ObjType t=new ObjType();t.name=text(x[2]);t.cost=Integer.parseInt(x[3]);t.stackable=Integer.parseInt(x[4]);t.certtemplate=Integer.parseInt(x[5]);t.certlink=Integer.parseInt(x[6]);t.members=Boolean.parseBoolean(x[7]);t.ops=options(x[8]);t.iops=options(x[9]);ObjTypeList.item=t;isp=ssp=0;intStack[isp++]=Integer.parseInt(x[1]);int op=Integer.parseInt(x[0]);if(op==4201||op==4202)intStack[isp++]=Integer.parseInt(x[10]);apply(op);System.out.println(ssp>0?"S:"+HexFormat.of().formatHex(stringStack[0].value.getBytes(StandardCharsets.UTF_8)):"I:"+intStack[0]);}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(530);cases=[];inputs=[]
    numbers=[-2147483648,-32768,-1,0,1,2,5,6,65535,2147483647];strings=[None,'','Take','Drop','Wield','Read','Caf\xe9','<col=ff0000>Use']
    for op in range(4200,4208):
        for i in range(128):
            metadata=dict(id=rng.choice(numbers),name=rng.choice(strings[1:]),cost=rng.choice(numbers),stackable=rng.choice(numbers),certtemplate=rng.choice(numbers),certlink=rng.choice(numbers),members=bool(i%2),ops=[rng.choice(strings)for _ in range(5)],iops=[rng.choice(strings)for _ in range(5)])
            args=[metadata['id']]+([rng.choice(numbers)]if op in [4201,4202]else[]);case=dict(opcode=op,metadata=metadata,args=args);cases.append(case)
            text=lambda s:'-'if s is None else s.encode('latin1').hex()
            fields=[op,metadata['id'],text(metadata['name']),metadata['cost'],metadata['stackable'],metadata['certtemplate'],metadata['certlink'],str(metadata['members']).lower(),','.join(map(text,metadata['ops'])),','.join(map(text,metadata['iops'])),args[-1]];inputs.append(';'.join(map(str,fields)))
    lines=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(lines)==len(cases)
    for c,line in zip(cases,lines):c['expected']=int(line[2:])if line.startswith('I:')else bytes.fromhex(line[2:]).decode('utf-8')
    data=json.dumps(dict(sources={'ScriptRunner':hashlib.sha256(raw).hexdigest()},bindings='Unchanged original4200..4207 branches over explicit resolved item-metadata service values. Raw item decoding/template generation/member-world filtering are separate contracts. Includes null/empty options, out-of-range option indices, signed field extremes and certificate predicates.',cases=cases),separators=(',',':')).encode()+b'\n'
    dest=root/'fixtures/ui/item-scripts.json'
    if check:
        if dest.read_bytes()!=data:raise SystemExit('CS2 item metadata corpus differs')
    else:dest.write_bytes(data)
    print(f'Original CS2 item metadata branches: {len(cases)} cases')

if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
