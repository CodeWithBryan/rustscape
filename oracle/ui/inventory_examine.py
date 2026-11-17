"""Complete unchanged MiniMenu Examine, actual JagString helpers and Chat.add."""
import hashlib,importlib.util,json,pathlib,random,subprocess
R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-inventory-examine-oracle';O.mkdir(exist_ok=True)

def generate(check=False):
 spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
 raws={n:(v.SRC/(n+'.java')).read_bytes().replace(b'\r\n',b'\n')for n in['MiniMenu','Buffer','JagString','Chat']};texts={n:v.clean(b.decode())for n,b in raws.items()}
 branch=v.method(texts['MiniMenu'],'if (actionCode == OBJ_EXAMINE_IN_COMPONENT)')
 (O/'Original.java').write_text('class Original{static final int OBJ_EXAMINE_IN_COMPONENT=1006;static final JagString QUANTITY_SEPARATOR=new JagString("207820");static Component pressedInventoryComponent;static int itemInteractionTick,pressedSlotIndex;static void run(int keyInt,int menuArg1,int menuArg2){int actionCode=1006;Component component;'+branch+'}}',encoding='utf-8')
 (O/'Chat.java').write_text(texts['Chat'],encoding='utf-8')
 jag='''class JagString{byte[]chars;int length;static final JagString NULL_STRING=new JagString("6e756c6c"),EMPTY=new JagString("");JagString(){}JagString(String hex){chars=java.util.HexFormat.of().parseHex(hex);length=chars.length;}
 '''+''.join(v.method(texts['JagString'],s)for s in['public static JagString concatenate(', 'public static JagString concatenateRange(', 'public static void copy(', 'public static JagString parseInt(int value)', 'public static JagString parseInt(boolean showSign, int value)'])+'}'
 (O/'JagString.java').write_text(jag,encoding='utf-8')
 (O/'Buffer.java').write_text('class Buffer{byte[]data=new byte[100];int offset;void p1isaac(int n){p1(n);}'+''.join(v.method(texts['Buffer'],s)for s in['public final void p1(', 'public final void ip2add('])+'}',encoding='utf-8')
 (O/'Probe.java').write_text(r'''
 import java.io.*;import java.util.*;
 class Component{int object;int[]objCounts;Component(int o,int[]a){object=o;objCounts=a;}}
 class InterfaceList{static int calls,first,second,transmitTimer;static Component getComponent(int id){Probe.add(0,id);int n=calls++==0?first:second;if(n==-2)throw new IllegalStateException();return n<0?null:Probe.nodes[n];}}
 class Protocol{static Buffer outboundBuffer;}
 class ObjType{JagString name;ObjType(JagString n){name=n;}}
 class ObjTypeList{static boolean fail;static JagString name;static ObjType get(int id){Probe.add(1,id);if(fail)throw new IllegalStateException();return new ObjType(name);}}
 public class Probe{static Component[]nodes;static List<String>log=new ArrayList<>();static void add(int...a){log.add(Arrays.toString(a));}static int[]array(String s){return s.equals("~")?null:s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}static String text(JagString s){return s==null?"":HexFormat.of().formatHex(s.chars);}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]f=line.split(";",-1);int[]a=array(f[0]);nodes=new Component[]{new Component(0,array(f[1])),new Component(1,array(f[2]))};Original.pressedInventoryComponent=a[3]<0?null:nodes[a[3]];Original.pressedSlotIndex=a[4];Original.itemInteractionTick=a[5];InterfaceList.calls=0;InterfaceList.first=a[6];InterfaceList.second=a[7];ObjTypeList.fail=a[8]!=0;InterfaceList.transmitTimer=a[9];Chat.size=a[10];Chat.transmitAt=0;Arrays.fill(Chat.types,0);Arrays.fill(Chat.phraseIds,0);Arrays.fill(Chat.messages,null);Arrays.fill(Chat.names,null);Arrays.fill(Chat.clans,null);ObjTypeList.name=f[3].equals("~")?null:new JagString(f[3]);Protocol.outboundBuffer=new Buffer();log.clear();String error="null";try{Original.run(a[1],a[0],a[2]);}catch(Exception e){error="\""+e.getClass().getSimpleName()+"\"";}System.out.println("[["+String.join(",",log)+"],\""+HexFormat.of().formatHex(Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset))+"\","+(Original.pressedInventoryComponent==null?-1:Original.pressedInventoryComponent.object)+","+Original.pressedSlotIndex+","+Original.itemInteractionTick+","+Chat.size+","+Chat.transmitAt+","+(Chat.messages[0]==null?"null":"["+Chat.types[0]+","+Chat.phraseIds[0]+",\""+text(Chat.messages[0])+"\",\""+text(Chat.names[0])+"\",\""+text(Chat.clans[0])+"\"]")+","+error+"]");}}}
 ''',encoding='utf-8')
 p=subprocess.run(['javac','-d',str(O),*[str(O/n)for n in['Original.java','Chat.java','JagString.java','Buffer.java','Probe.java']]],capture_output=True,text=True);(O/'compile.log').write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr
 rng=random.Random(5301006);cases=[];values=[-2147483648,-1,0,1,99998,99999,100000,100001,2147483647]
 for i in range(2048):
  arrays=[[rng.choice(values)for _ in range(rng.randrange(3))]for _ in range(2)];a=[rng.choice([-1,0,1,2,65535]),rng.choice([995,1351,-1,65535,-2147483648,2147483647]),rng.choice([1<<16,149<<16,746<<16|21]),rng.choice([-1,0,1]),rng.choice([-1,0,1,65535]),rng.choice([-2147483648,0,7,14,2147483647]),rng.choice([-2,-1,0,1]),rng.choice([-2,-1,0,1]),int(rng.randrange(6)==0),rng.choice([-2147483648,-1,0,1,512,2147483647]),rng.choice([-2147483648,0,7,2147483647])]
  if i%2==0:a[0]=0;a[6]=0;arrays[0]=[values[(i//2)%len(values)]]
  if i%7==0:arrays[rng.randrange(2)]=None
  name=rng.choice([None,'','Coins','Bronze axe','Caf\xe9','<col=ff0000>Coins','null'])
  cases.append(dict(arguments=a,counts=arrays,name=None if name is None else name.encode('latin1').hex()))
 def arr(a):return '~'if a is None else ','.join(map(str,a))
 p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(';'.join([arr(c['arguments']),*[arr(a)for a in c['counts']],c['name']if c['name']is not None else'~'])for c in cases)+'\n',capture_output=True,text=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
 rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==2048
 for c,row in zip(cases,rows):c['expected']=row
 out=dict(scope='Complete unchanged inventory Examine branch, exact JagString decimal/concatenation/copy and complete original Chat class. Original no-ISAAC packet writer; scripted first/late component/definition services. Canonical100-entry chat arrays and normalized nullable text reads, as existing chat-state contract. Tests threshold, null/count bounds, definition/lookup failures, post-chat/packet reset and late capture. No cipher/buffer-capacity/whole Chat/MiniMenu/native OS or full-game claim.',sources={n+'.java':hashlib.sha256(b).hexdigest()for n,b in raws.items()},cases=cases)
 target=R/'fixtures/ui/inventory-examine.json';content=json.dumps(out,separators=(',',':'))+'\n'
 if check:assert target.read_text(encoding='utf-8')==content,'Inventory Examine corpus differs'
 else:target.write_text(content,encoding='utf-8')
 print('Original complete inventory Examine',len(cases),'cases')

if __name__=='__main__':
 import sys
 generate('--check'in sys.argv)
