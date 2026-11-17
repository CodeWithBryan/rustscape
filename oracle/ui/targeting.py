"""Unchanged MiniMenu widget menus/action writes and ParamType decoder.

Synthetic component state and typed item/parameter tables are explicit bindings;
this does not run combat, content handlers or a complete original UI session.
"""
import hashlib,importlib.util,json,pathlib,random,re,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[2]

def generate(check=False):
    spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    spec=importlib.util.spec_from_file_location('fields',ROOT/'tools/port_component_fields.py');f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)
    run=ROOT/'runs/ui-oracle/targeting';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    mini=source('MiniMenu');numbers={name:int(n)for name,n in re.findall(r'public static final int (\w+) = (\d+);',mini)}
    declarations=''.join('public '+t+' '+n+(' = '+value if value else '')+';'for t,n,value in f.fields())
    (run/'Component.java').write_text('class Component {static final JagString EMPTY_STRING=JagString.EMPTY;static final ServerActiveProperties DEFAULT_SERVER_ACTIVE_PROPERTIES=new ServerActiveProperties(0,-1);'+declarations+'}',encoding='utf-8')
    (run/'ServerActiveProperties.java').write_text(source('ServerActiveProperties').replace(' extends Node',''),encoding='utf-8')
    (run/'ParamType.java').write_text(source('ParamType').replace(' extends SecondaryNode',''),encoding='utf-8')
    buffer=source('Buffer');(run/'Buffer.java').write_text('class Buffer {byte[] data;int offset;Buffer(byte[] d){data=d;}void p1isaac(int n){p1(n);}'+''.join(v.method(buffer,s)for s in ['public final int g1()', 'public final int g4()', 'public final JagString gjstr()', 'public final void p1(', 'public final void p2(', 'public final void p2add(', 'public final void ip2(', 'public final void ip2add(', 'public final void p4(', 'public final void ip4(', 'public final void mp4(', 'public final void imp4('])+'}',encoding='utf-8')
    (run/'MiniMap.java').write_text('class MiniMap {static int useCursor=-1,examineCursor=-1,selectedComponentId;static JagString HIDDEN_OPTION_TEXT=JagString.EMPTY;'+v.method(source('MiniMap'),'public static JagString getTargetVerb(')+'}',encoding='utf-8')
    (run/'InterfaceList.java').write_text('class InterfaceList {static JagString HIDDEN_OPTION_PREFIX=JagString.EMPTY;static Component mouseOverInventoryInterface;static ServerActiveProperties getServerActiveProperties(Component c){return c.properties;}'+v.method(source('InterfaceList'),'public static JagString getOp(')+'}',encoding='utf-8')
    constants=''.join(line for line in mini.splitlines()if 'public static final JagString 'in line or 'public static final int 'in line)
    writes=''
    for action in [3,11,12,15,38,39,45]:
        name=next(n for n,value in numbers.items()if value==action);block=v.method(mini,'if (actionCode == '+name)
        writes+=f'if(actionCode=={action}){{'+''.join(line for line in block.splitlines()if 'Protocol.outboundBuffer.'in line)+'}'
    menu='''class MiniMenu {static int size,itemTargetMode,selectedObjSlot,selectedObjId,targetParamId=-1,targetMask,targetCursorId,clickedInventoryIndex;static boolean isTargeting,DEBUG_OPS;static JagString selectedObjText,targetOpBase,targetVerb;static java.util.ArrayList<String> entries=new java.util.ArrayList<>();
static JagString[] annotateOps(JagString[] ops){return ops;}static String hex(JagString s){return s==null?"-":java.util.HexFormat.of().formatHex(s.value.getBytes(java.nio.charset.StandardCharsets.ISO_8859_1));}
static void add(int cursor,long key,JagString target,int a,short action,JagString option,int b){entries.add(action+":"+key+":"+a+":"+b+":"+cursor+":"+hex(option)+":"+hex(target));size++;}
'''+constants+v.method(mini,'public static void addComponentEntries(')+v.method(mini,'public static int getOpCursor(')+'''static byte[] wire(int actionCode,int keyInt,int menuArg1,int menuArg2,int targetInterfaceId,int targetChildId,long key){Protocol.outboundBuffer=new Buffer(new byte[1000]);'''+writes+'''return java.util.Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset);}}
'''
    (run/'MiniMenu.java').write_text(menu,encoding='utf-8')
    (run/'Support.java').write_text('''class JagString {String value;static final JagString EMPTY=new JagString("");JagString(String s){value=s;}static JagString parseInt(int n){return new JagString(""+n);}static JagString parse(String s){String out="";for(int i=0;i<s.length();i++){int c=s.charAt(i);if(c>=40&&c<=45&&i+1<s.length())c=(c-40)*43+s.charAt(++i)-48;if(c!=0)out+=(char)c;}return new JagString(out);}static JagString decodeString(byte[] d,int n,int o){return new JagString(new String(d,o,n,java.nio.charset.StandardCharsets.ISO_8859_1));}int length(){return value.length();}JagString trim(){return new JagString(value.trim());}static JagString concatenate(JagString[] parts){String s="";for(JagString p:parts)s+=p.value;return new JagString(s);}}
class Cheat{static boolean qaOpTest;}class Cs1ScriptRunner{static Component pleaseWaitComponent;}class Camera{static int originX,originY;}class Protocol{static Buffer outboundBuffer;}class LocalizedText{static JagString OK=new JagString("Ok"),CLOSE=new JagString("Close"),CONTINUE=new JagString("Continue"),USE=new JagString("Use"),EXAMINE=new JagString("Examine");}
class ObjType {int id=1;JagString name=new JagString("Seed");JagString[] iops=new JagString[5];int getParam(int def,int id){return def;}}class ObjTypeList{static ObjType get(int id){return new ObjType();}}class ParamTypeList{static ParamType get(int id){return new ParamType();}}
''',encoding='utf-8')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;public class Probe {static String unhex(String s){return new String(HexFormat.of().parseHex(s),java.nio.charset.StandardCharsets.ISO_8859_1);}public static void main(String[]args)throws Exception {BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){String[] x=line.split(";",-1);if(x[0].equals("W")){int[] a=Arrays.stream(x[1].split(",")).mapToInt(Integer::parseInt).toArray();Camera.originX=a[7];Camera.originY=a[8];System.out.println(HexFormat.of().formatHex(MiniMenu.wire(a[0],a[1],a[2],a[3],a[4],a[5],Integer.toUnsignedLong(a[6])<<32)));continue;}if(x[0].equals("P")){ParamType p=new ParamType();p.decode(new Buffer(HexFormat.of().parseHex(x[1])));System.out.println(p.defaultInt+":"+(p.isString()?1:0)+":"+MiniMenu.hex(p.defaultString));continue;}int[] a=Arrays.stream(x[1].split(",")).mapToInt(Integer::parseInt).toArray();Component c=new Component();c.id=a[0];c.createdComponentId=a[1];c.if3=a[2]!=0;c.type=5;c.buttonType=a[3];c.properties=new ServerActiveProperties(a[4],-1);c.optionCircumfix=new JagString(unhex(x[2]));c.optionSuffix=new JagString("Spell");c.optionBase=new JagString("Base");c.ops=new JagString[10];for(int i=0;i<10;i++)if((a[5]>>i&1)!=0)c.ops[i]=new JagString("Op"+i);if(a[6]!=0)c.onOptionClick=new Object[]{1};MiniMenu.isTargeting=a[7]!=0;MiniMenu.targetMask=a[8];MiniMenu.targetCursorId=37;MiniMenu.targetVerb=new JagString("Cast");MiniMenu.targetOpBase=new JagString("Selected");MiniMenu.entries.clear();MiniMenu.size=0;MiniMenu.addComponentEntries(0,0,c);System.out.println(String.join(",",MiniMenu.entries));}}}
''',encoding='utf-8')
    spec=importlib.util.spec_from_file_location('lifecycle',pathlib.Path(__file__).with_name('target_lifecycle.py'));lifecycle=importlib.util.module_from_spec(spec);spec.loader.exec_module(lifecycle)
    life=lifecycle.emit(run,source,v.method)
    spec=importlib.util.spec_from_file_location('filters',pathlib.Path(__file__).with_name('target_filter.py'));filters=importlib.util.module_from_spec(spec);spec.loader.exec_module(filters)
    eligibility=filters.emit(run,source,v.method)
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    lifecycle.observe(run,life)
    filters.observe(run,eligibility)
    rng=random.Random(53032);wires=[];menus=[];params=[];inputs=[]
    for action in [3,11,12,15,38,39,45]:
        for _ in range(128):
            a=[action,rng.choice([-1,0,1,32767,65535,65536]),rng.randrange(-65536,65536),rng.randrange(-2147483648,2147483648),rng.randrange(-2147483648,2147483648),rng.choice([-1,0,32768,65535]),rng.randrange(2147483648),rng.randrange(-32768,32768),rng.randrange(-32768,32768)];wires.append(dict(args=a));inputs.append('W;'+','.join(map(str,a)))
    for _ in range(512):
        if3=rng.randrange(2);a=[149<<16,rng.choice([-1,0,4])if if3 else -1,if3,0 if if3 else 2,rng.randrange(1<<23),rng.randrange(1<<10),rng.randrange(2),rng.randrange(2),rng.randrange(128)];verb=rng.choice(['Cast','   ','\tCast\t','\xa0']);menus.append(dict(args=a,verb=verb));inputs.append('M;'+','.join(map(str,a))+';'+verb.encode('latin1').hex())
    for kind in [0,1,115,255]:
        for default in [-2147483648,-1,0,1,2147483647]:
            raw=bytes([1,kind,2])+default.to_bytes(4,'big',signed=True)+b'\x05text\x00\x00';params.append(dict(raw=raw.hex()));inputs.append('P;'+raw.hex())
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(inputs)
    for case,value in zip(wires+menus+params,result):case['expected']=value
    content=json.dumps(dict(sources=hashes,bindings='English UI, QA/debug options disabled, explicit synthetic component/selection state and typed parameter table bindings; unchanged widget menu, target conditions, typed getParam methods and seven action write branches. Unchanged lifecycle/select bodies with explicit hook mutation/error adapters documented in target_lifecycle.py. No full CS2/world approach/content claim.',wires=wires,menus=menus,params=params,lifecycle=life,eligibility=eligibility),separators=(',',':')).encode()+b'\n';target=ROOT/'fixtures/ui/targeting.json'
    if check:assert target.read_bytes()==content,'targeting corpus differs'
    else:target.write_bytes(content)
    print(f'RT4 selected-component targeting: {len(wires)} wire / {len(menus)} widget menu / {len(params)} parameter / {len(life)} lifecycle / {len(eligibility)} typed target cases',flush=True)
if __name__=='__main__':generate('--check'in sys.argv)
