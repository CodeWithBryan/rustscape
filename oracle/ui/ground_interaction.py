"""Unchanged RT4 ground-menu branch and complete action serialization branches.

The shared ground reducer and its ordering oracle belong to protocol/game/ground.
These adapters only supply menu inputs, node traversal, routing and output buffers.
"""
import argparse, hashlib, importlib.util, json, pathlib, random, re, subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
RUN = ROOT / 'runs/ground-interaction-oracle'
FIX = ROOT / 'fixtures/ui/ground-interaction.json'


def generate(check=False):
    spec = importlib.util.spec_from_file_location('visual', ROOT / 'oracle/visual/generate.py')
    v = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(v)
    sources = {}

    def source(name):
        raw = (ROOT / f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n', b'\n')
        sources[name + '.java'] = hashlib.sha256(raw).hexdigest()
        return v.clean(raw.decode())

    mini = source('MiniMenu')
    buffer = source('Buffer')
    names = ['UNKNOWN_21', 'OBJSTACK_ACTION_1', 'OBJSTACK_ACTION_2', 'UNKNOWN_24', 'OBJ_EXAMINE', 'OBJ_OBJSTACK_ACTION']
    constants = ''.join(re.search(r'public static final int ' + n + r' = [^;]+;', mini).group(0) for n in names)
    branches = ''.join(v.method(mini, 'if (actionCode == ' + n + ')') for n in names)
    declarations = '''static int size,defaultCursorId=-1,itemTargetMode,selectedObjSlot,selectedObjId;
static boolean DEBUG_OPS=false,isTargeting=false;static int targetParamId=-1,targetMask,targetCursorId=-1;
static JagString selectedObjText,targetOpBase,targetVerb;
static final JagString COLOR_ITEM_ORANGE=new JagString("<col=ff9040>"),ARROW_ORANGE=new JagString(" -> <col=ff9040>");
static JagString[]ops=new JagString[500],opBases=new JagString[500];static short[]actions=new short[500];
static int[]intArgs1=new int[500],intArgs2=new int[500],cursors=new int[500];static long[]keys=new long[500];'''
    menu = v.method(mini, 'if (entityType == 3)')
    code = 'class MiniMenu {' + declarations + constants + v.method(mini, 'public static void add(') + v.method(mini, 'public static void sort()')
    code += 'static JagString[]annotateOps(JagString[]v){return v;}static void ground(int x,int y){int entityType=3,j,otherFineX;' + menu + '}'
    code += 'static void action(int actionCode,int keyInt,int menuArg1,int menuArg2){if(actionCode>=2000)actionCode-=2000;boolean pathFound;' + branches + '}}'
    code += 'class Buffer{byte[]data=new byte[100];int offset;' + ''.join(v.method(buffer, s) for s in ['public final void p1(', 'public final void p2(', 'public final void p2add(', 'public final void ip2(', 'public final void ip2add(', 'public final void mp4(']) + 'void p1isaac(int n){p1(n);}}'
    RUN.mkdir(parents=True, exist_ok=True)
    (RUN / 'GroundOracle.java').write_text(code + SUPPORT, encoding='utf-8')
    subprocess.run(['javac', '-encoding', 'UTF-8', '-d', str(RUN), str(RUN / 'GroundOracle.java')], check=True)
    rng = random.Random(53033)
    cases, requests = [], []
    for i in range(512):
        tile = [rng.randrange(104), rng.randrange(104)]
        items = []
        for j in range(rng.randrange(1, 6)):
            options = [f'Option {op}' if rng.randrange(3) == 0 else None for op in range(5)]
            if i % 7 == 0:
                options = [None, None, 'Take', None, None]
            items.append(dict(id=rng.randrange(65536), name=f'Item {i}-{j}', ops=options,
                              cursors=[rng.randrange(-1, 5), rng.randrange(-1, 100), rng.randrange(-1, 5), rng.randrange(-1, 100)]))
        selected = 'Selected<col=ffffff>' if i % 3 == 0 else None
        initial = [0, 1, 495, 499, 500][i % 5] if i % 11 == 0 else 0
        opened = i % 31 == 0
        c = dict(kind='menu', tile=tile, items=items, selected=selected, initial=initial, opened=opened)
        cases.append(c)
        words = ['M', str(tile[0]), str(tile[1]), str(initial), str(int(opened)), selected or '-', str(len(items))]
        for item in items:
            words += [str(item['id']), item['name'], *[s or '-' for s in item['ops']], *map(str, item['cursors'])]
        requests.append('\t'.join(words))
    for i in range(1024):
        action = [21, 18, 20, 24, 1002, 33][i % 6] + (2000 if i % 5 == 0 else 0)
        values = [rng.randrange(-2**31, 2**31) for _ in range(6)]
        c = dict(kind='action', action=action, id=values[0], tile=values[1:3], selected=dict(id=values[3], slot=values[4], component=values[5]))
        cases.append(c)
        requests.append('\t'.join(map(str, ['A', action, *values])))
    p = subprocess.run(['java', '-cp', str(RUN), 'GroundOracle'], input='\n'.join(requests) + '\n', text=True, capture_output=True, check=True)
    lines = p.stdout.splitlines()
    assert len(lines) == len(cases), p.stdout[-1000:]
    for c, expected in zip(cases, lines):
        c['expected'] = hashlib.sha256(bytes.fromhex(expected)).hexdigest() if c['kind'] == 'menu' else expected
    out = dict(sources=sources, bindings='Unchanged MiniMenu entityType3 branch, add/sort and six complete action branches. Explicit item definitions, linked-list traversal, default cursor -1, menu plugin no-op, English strings, pathfinder/player/cross adapters, ISAAC disabled. Covers normal/item-selected menus and action bytes; not spell targeting, original picking, ground reducer, live Take/content or full player option34 dispatch.', cases=cases)
    if check:
        assert json.loads(FIX.read_text()) == out, 'Ground interaction fixture differs'
    else:
        FIX.write_text(json.dumps(out, separators=(',', ':')) + '\n')
    print('Original ground menus/actions:', len(cases), 'cases')


SUPPORT = r'''
class JagString{String s;JagString(String s){this.s=s;}static JagString concatenate(JagString[]v){StringBuilder b=new StringBuilder();for(JagString j:v)b.append(j.s);return new JagString(b.toString());}}
class ObjType{JagString name;JagString[]ops=new JagString[5];int cursor1Op,cursor1,cursor2Op,cursor2;int getParam(int d,int k){return d;}}
class ObjTypeList{static java.util.Map<Integer,ObjType>types=new java.util.HashMap<>();static ObjType get(int id){return types.get(id);}}
class ObjStack{int type;ObjStack(int id){type=id;}}class ObjStackNode{ObjStack value;ObjStackNode(int id){value=new ObjStack(id);}}
class LinkedList{java.util.ArrayList<ObjStackNode>nodes=new java.util.ArrayList<>();int cursor;ObjStackNode tail(){cursor=nodes.size()-1;return prev();}ObjStackNode prev(){return cursor<0?null:nodes.get(cursor--);}}
class SceneGraph{static LinkedList[][][]objStacks=new LinkedList[4][104][104];}
class Player{static int plane;int[]movementQueueX={50},movementQueueY={50};}class PlayerList{static Player self=new Player();}
class MiniMap{static int useCursor=-1,examineCursor=-1,selectedComponentId;}
class LocalizedText{static JagString USE=new JagString("Use"),EXAMINE=new JagString("Examine");}
class ParamType{int defaultInt;}class ParamTypeList{static ParamType get(int id){return new ParamType();}}
class Cs1ScriptRunner{static boolean isMenuOpen;}class PluginRepository{static void DrawMiniMenu(MiniMenuEntry e){}}class MiniMenuEntry{MiniMenuEntry(int n){}}
class client{static int game;}class Camera{static int originX,originY;}class Cross{static int x,y,type,milliseconds;}class Mouse{static int clickX,clickY;}
class Protocol{static Buffer outboundBuffer;}class PathFinder{static boolean findPath(int...v){return false;}static boolean findPath(int a,int b,int c,boolean d,int e,int f,int g,int h,int i,int j,int k){return false;}}
public class GroundOracle{
static String hex(byte[]b){return java.util.HexFormat.of().formatHex(b);}
static int number(String[]w,int n){return Integer.parseInt(w[n]);}
static void text(java.io.DataOutputStream d,JagString s)throws Exception{byte[]b=s.s.getBytes(java.nio.charset.StandardCharsets.UTF_8);d.writeInt(b.length);d.write(b);}
public static void main(String[]args)throws Exception{
var r=new java.io.BufferedReader(new java.io.InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;
while((line=r.readLine())!=null){String[]w=line.split("\t");
if(w[0].equals("A")){Protocol.outboundBuffer=new Buffer();MiniMenu.selectedObjId=number(w,5);MiniMenu.selectedObjSlot=number(w,6);MiniMap.selectedComponentId=number(w,7);MiniMenu.action(number(w,1),number(w,2),number(w,3),number(w,4));System.out.println(hex(java.util.Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset)));continue;}
int x=number(w,1),y=number(w,2);MiniMenu.size=number(w,3);Cs1ScriptRunner.isMenuOpen=number(w,4)!=0;MiniMenu.itemTargetMode=w[5].equals("-")?0:1;MiniMenu.selectedObjText=new JagString(w[5]);
for(int i=0;i<MiniMenu.size;i++){MiniMenu.ops[i]=new JagString("Prefix");MiniMenu.opBases[i]=new JagString("");MiniMenu.actions[i]=1005;MiniMenu.keys[i]=0;MiniMenu.intArgs1[i]=MiniMenu.intArgs2[i]=0;MiniMenu.cursors[i]=-1;}
ObjTypeList.types.clear();LinkedList nodes=new LinkedList();int n=number(w,6),p=7;
for(int i=0;i<n;i++){int id=number(w,p++);ObjType t=new ObjType();t.name=new JagString(w[p++]);for(int j=0;j<5;j++){String s=w[p++];t.ops[j]=s.equals("-")?null:new JagString(s);}t.cursor1Op=number(w,p++);t.cursor1=number(w,p++);t.cursor2Op=number(w,p++);t.cursor2=number(w,p++);ObjTypeList.types.put(id,t);nodes.nodes.add(new ObjStackNode(id));}
SceneGraph.objStacks[0][x][y]=nodes;MiniMenu.ground(x,y);MiniMenu.sort();var b=new java.io.ByteArrayOutputStream();var d=new java.io.DataOutputStream(b);d.writeInt(MiniMenu.size);
for(int i=0;i<MiniMenu.size;i++){text(d,MiniMenu.ops[i]);text(d,MiniMenu.opBases[i]);d.writeInt(MiniMenu.actions[i]);d.writeLong(MiniMenu.keys[i]);d.writeInt(MiniMenu.intArgs1[i]);d.writeInt(MiniMenu.intArgs2[i]);d.writeInt(MiniMenu.cursors[i]);}System.out.println(hex(b.toByteArray()));
}}
}
'''

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    generate(parser.parse_args().check)
