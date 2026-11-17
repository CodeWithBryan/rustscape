"""Unchanged NPC metadata decoder, menu construction, and packet write statements."""
import hashlib,json,pathlib,random,re,subprocess
def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/npc';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    npc=source('NpcType');head=npc[:npc.index('public final NpcType getMultiNpc()')];fields=re.findall(r'(?:public|private|protected) (?:static )?(?:final )?(?:int|boolean|byte|short|JagString|HashTable)(?:\[\])* \w+(?:\s*=\s*[^;]+)?;',head)
    (run/'NpcType.java').write_text('class NpcType {'+'\n'.join(f.replace('private ','public ').replace('protected ','public ')for f in fields)+method(npc,'public final void decode(')+method(npc,'private void decode(')+'NpcType getMultiNpc(){return this;}int getParam(int key,int value){return value;}}',encoding='utf-8')
    mini=source('MiniMenu');constants=''.join(re.findall(r'public static final JagString \w+ = [^;]+;',mini))
    names=['NPC_ACTION_1','NPC_ACTION_2','NPC_ACTION_3','NPC_ACTION_4','NPC_ACTION_5','NPC_EXAMINE','OBJ_OBJ_ACTION','OBJ_NPC_ACTION','OBJ_LOC_ACTION']
    numbers=dict((n,int(v))for n,v in re.findall(r'public static final int (\w+) = (\d+);',mini))
    writes=''
    for name in names:
        signature='if (actionCode == '+name+(' && PathFinder.findPathToLoc(key, menuArg2, menuArg1))'if name=='OBJ_LOC_ACTION'else ')')
        block=method(mini,signature);lines='\n'.join(line for line in block.splitlines()if 'Protocol.outboundBuffer.'in line)
        writes+=f'if(actionCode=={numbers[name]}){{'+lines+'}'
    (run/'MenuOracle.java').write_text('class MenuOracle {'+constants+'''static int size,itemTargetMode,targetParamId=-1,targetMask,targetCursorId;static boolean isTargeting=false,DEBUG_OPS=false;static JagString selectedObjText,targetOpBase,targetVerb;static int selectedObjSlot,selectedObjId;static JagString[] annotateOps(JagString[] ops){return ops;}
static java.util.ArrayList<String> entries=new java.util.ArrayList<>();static String hex(JagString s){return s==null?"-":java.util.HexFormat.of().formatHex(s.value.getBytes(java.nio.charset.StandardCharsets.ISO_8859_1));}
static void add(int cursor,long key,JagString target,int a,short action,JagString option,int b){entries.add(action+":"+key+":"+a+":"+b+":"+cursor+":"+hex(option)+":"+hex(target));size++;}
'''+method(mini,'public static void addNpcEntries(')+method(mini,'public static JagString getCombatLevelColor(')+'''static byte[] wire(int actionCode,int keyInt,int menuArg1,int menuArg2,NpcType npcType){Protocol.outboundBuffer=new Buffer(new byte[1000]);long key=(long)keyInt<<32;'''+writes+'''return java.util.Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset);}}
class MiniMap{static int useCursor=-1,examineCursor=-1,selectedComponentId;}class client{static int game=0;}class PlayerList{static Player self=new Player();}class Player{int combatLevel;}class Camera{static int originX,originY;}class ParamType{int defaultInt;}class ParamTypeList{static ParamType get(int id){return null;}}class Protocol{static Buffer outboundBuffer;}
''',encoding='utf-8')
    buffer=source('Buffer');(run/'Buffer.java').write_text('class Buffer {byte[] data;int offset;Buffer(byte[] data){this.data=data;}void p1isaac(int n){p1(n);}'+''.join(method(buffer,s)for s in ['public final int g1()', 'public final byte g1b()', 'public final int g2()', 'public final int g3()', 'public final int g4()', 'public final JagString gjstr()', 'public final void p1(', 'public final void p2(', 'public final void p2add(', 'public final void ip2(', 'public final void ip2add(', 'public final void p4(', 'public final void ip4(', 'public final void mp4('])+'}',encoding='utf-8')
    (run/'Support.java').write_text('''class JagString{String value;static final JagString EMPTY=new JagString("");JagString(String value){this.value=value;}static JagString parse(String s){String out="";for(int i=0;i<s.length();i++){int c=s.charAt(i);if(c>=40&&c<=45&&i+1<s.length()){int next=s.charAt(++i);c=(c-40)*43+next-48;}if(c!=0)out+=(char)c;}return new JagString(out);}static JagString decodeString(byte[] b,int n,int o){return new JagString(new String(b,o,n,java.nio.charset.StandardCharsets.ISO_8859_1));}boolean equalsIgnoreCase(JagString b){return value.equalsIgnoreCase(b.value);}static JagString concatenate(JagString[] parts){String s="";for(JagString p:parts)s+=p.value;return new JagString(s);}static JagString parseInt(int n){return new JagString(""+n);}}
class LocalizedText {static JagString HIDDEN=new JagString("Hidden"),ATTACK=new JagString("Attack"),USE=new JagString("Use"),EXAMINE=new JagString("Examine"),LEVEL=new JagString("level: "),RATING=new JagString("rating: ");}class Node{}class HashTable{HashTable(int n){}void put(Node n,long key){}}class IntNode extends Node{IntNode(int n){}}class StringNode extends Node{StringNode(JagString s){}}class IntUtils{static int clp2(int n){return n;}}
''',encoding='utf-8')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
public class Probe {public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[] x=line.split(";",-1);NpcType d=new NpcType();d.id=Integer.parseInt(x[0]);d.decode(new Buffer(HexFormat.of().parseHex(x[1])));String meta=d.combatLevel+":"+(d.interactive?1:0)+":"+d.cursor1Op+":"+d.cursor1+":"+d.cursor2Op+":"+d.cursor2+":"+d.attackCursor;for(JagString op:d.ops)meta+=":"+MenuOracle.hex(op);
int[] a=Arrays.stream(x[2].split(",")).mapToInt(Integer::parseInt).toArray();PlayerList.self.combatLevel=a[0];if(a[1]!=-2)d.combatLevel=a[1];if(a[2]!=-1)d.interactive=a[2]==1;MenuOracle.size=0;MenuOracle.entries.clear();MenuOracle.itemTargetMode=a[3];MenuOracle.selectedObjText=new JagString("<col=ff9040>Seed<col=ffffff>");MenuOracle.addNpcEntries(d,a[4],a[4],0);String out=meta+";"+String.join(",",MenuOracle.entries);for(int action:new int[]{17,16,4,19,2,1007,40,26,14}){MiniMap.selectedComponentId=a[5];MenuOracle.selectedObjId=a[6];MenuOracle.selectedObjSlot=a[7];out+=";"+HexFormat.of().formatHex(MenuOracle.wire(action,a[4],a[7],a[5],d));}System.out.println(out);}}}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[]
    for _ in range(512):
        id=rng.choice([0,1,4,9,41,47,81,100]);raw=(root/f'fixtures/visual/npc_{id}.bin').read_bytes();a=[rng.randrange(3,127),rng.choice([-2,0,1,3,126,65535]),rng.choice([-1,0,1]),rng.randrange(2),rng.randrange(32768),rng.randrange(-2147483648,2147483648),rng.randrange(65536),rng.randrange(65536)];cases.append(dict(id=id,raw=raw.hex(),args=a))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(str(c['id'])+';'+c['raw']+';'+','.join(map(str,c['args']))for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for case,line in zip(cases,result):
        metadata,entries,*wires=line.split(';');case.update(metadata=metadata,entries=entries.split(',')if entries else[],wires=wires)
    p=root/'fixtures/ui/npc.json';content=json.dumps(dict(sources=hashes,bindings='Eight unmorphed cached NPC types; English game0; no spell/param targeting; actor index and local combat supplied explicitly',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if p.read_bytes()!=content:raise SystemExit('NPC interaction corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 NPC menus, metadata and action writes: {len(cases)} scenarios')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
