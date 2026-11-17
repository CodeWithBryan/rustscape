"""Unchanged MiniMenu NPC/player overlap branches and picked-key dispatch.

Actor fields/getSize and menu callbacks are explicit snapshot/recording adapters.
This does not execute GPU picking, menu insertion caps or server interactions.
"""
import hashlib,json,pathlib,random,subprocess


def generate(root,clean,method,check=False):
    run=root/'runs/entity-menu-oracle';run.mkdir(parents=True,exist_ok=True)
    raw=(root/'reference/client/client/src/main/java/rt4/MiniMenu.java').read_bytes().replace(b'\r\n',b'\n')
    source=clean(raw.decode());body=method(source,'public static void addEntries(')
    dispatch=body[body.index('long lastKey = -1L;'):body.index('if (entityType == 2')]
    npc=method(body,'if (entityType == 1)');player=method(body,'if (entityType == 0)')
    declarations='int otherFineX,otherFineY,minFineX,k;Npc otherNpc;Player otherPlayer;'
    producer_raw=(root/'reference/client/client/src/main/java/rt4/ScriptRunner.java').read_bytes().replace(b'\r\n',b'\n')
    producer=clean(producer_raw.decode())
    npc_key=next(line.strip()for line in producer.splitlines()if 'long sceneKey = (long) NpcList.ids[i]'in line)
    player_key=next(line.strip()for line in producer.splitlines()if 'sceneKey = (long) PlayerList.ids[i]'in line)
    negative=method(producer,'if (!npc.type.interactive)')
    gl_raw=(root/'reference/client/client/src/main/java/rt4/GlModel.java').read_bytes().replace(b'\r\n',b'\n')
    pickable=next(line.strip()for line in clean(gl_raw.decode()).splitlines()if 'boolean miniMenuPick = key > 0L;'in line)
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class NpcType{int size;boolean interactive;}class Npc{NpcType type=new NpcType();int xFine,yFine;}
class Player{int xFine,yFine,size;int getSize(){return size;}}
class NpcList{static Npc[]npcs=new Npc[32768];static int[]ids;static int size;}
class PlayerList{static Player[]players=new Player[2048];static int[]ids;static int size;}
class Model{static long[]pickResults;}
public class Probe{
static int pickResultCount;static List<String>out=new ArrayList<>();
static void addNpcEntries(NpcType t,int x,int id,int y){out.add("1,"+id+","+x+","+y);}
static void addPlayerEntries(int id,int y,Player p,int x){out.add("0,"+id+","+x+","+y);}
static void dispatch(){int i,x;
'''+dispatch+declarations+npc+player+'''}}}
static long key(int kind,int index,int interactive){int i=0;NpcList.ids=new int[]{index};PlayerList.ids=new int[]{index};if(kind==1){Npc npc=new Npc();npc.type.interactive=interactive!=0;
'''+npc_key+negative+'''return sceneKey;}else{long sceneKey;
'''+player_key+'''return sceneKey;}}
static boolean pickable(long key){'''+pickable+'''return miniMenuPick;}
static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
public static void main(String[]args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));String line;while((line=reader.readLine())!=null){if(line.startsWith("K;")){int[]a=ints(line.substring(2));long key=key(a[0],a[1],a[2]);System.out.println(String.format("%016x,%d",key,pickable(key)?1:0));continue;}String[]columns=line.split(";",-1);out.clear();
List<Integer>ids=new ArrayList<>();if(!columns[0].isEmpty())for(String row:columns[0].split("/")){int[]a=ints(row);Npc n=NpcList.npcs[a[0]];if(!ids.contains(a[0])){n=new Npc();n.xFine=a[1];n.yFine=a[2];n.type.size=a[3];NpcList.npcs[a[0]]=n;}ids.add(a[0]);}NpcList.ids=ids.stream().mapToInt(Integer::intValue).toArray();NpcList.size=ids.size();
ids.clear();if(!columns[1].isEmpty())for(String row:columns[1].split("/")){int[]a=ints(row);Player p=PlayerList.players[a[0]];if(!ids.contains(a[0])){p=new Player();p.xFine=a[1];p.yFine=a[2];p.size=a[3];PlayerList.players[a[0]]=p;}ids.add(a[0]);}PlayerList.ids=ids.stream().mapToInt(Integer::intValue).toArray();PlayerList.size=ids.size();
Model.pickResults=columns[2].isEmpty()?new long[0]:Arrays.stream(columns[2].split(",")).mapToLong(s->Long.parseUnsignedLong(s,16)).toArray();pickResultCount=Model.pickResults.length;dispatch();System.out.println(String.join("/",out));
}}}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(530127);cases=[]
    def key(kind,index,x=31,y=47,sign=False):return ((index<<32)|(kind<<29)|(y<<7)|x|((1<<63)if sign else 0))&((1<<64)-1)
    def add(npcs,players,keys,label):cases.append(dict(npcs=npcs,players=players,keys=[f'{k:016x}'for k in keys],label=label))
    # Exhaust the alignment/size differences and fine-position floor boundaries.
    for kind in [0,1]:
        for size in [1,2,3,4,8]:
            for offset in [0,1,63,64,65,127]:
                for dx,dy in [(0,0),(1,0),(63,0),(64,0),(127,0),(128,0),(-1,0),(0,127),(127,127),(128,128),(-128,-128)]:
                    center=2048+offset;parent=[7,center,center,size]
                    lower=center+64-size*64
                    npcs=[[10,lower+dx+64,lower+dy+64,1],[11,center,center,size]]
                    players=[[12,lower+dx+64,lower+dy+64,1],[13,center,center,size]]
                    (npcs if kind else players).insert(0,parent)
                    add(npcs,players,[key(kind,7)],'alignment-and-footprint')
    # Ordered lists, duplicate slots, consecutive/nonconsecutive keys and flags.
    for i in range(768):
        size=rng.choice([1,2,3,4,8]);center=rng.choice([1024,1088,2048,2112]);parent=[7,center,center,size]
        npcs=[[n,center+rng.choice([-256,-128,-127,-64,-1,0,1,64,127,128,256]),center+rng.choice([-256,-128,-64,0,64,127,128,256]),rng.choice([1,2,3,4,8])]for n in range(10,18)]
        players=[[n,center+rng.choice([-128,-64,0,1,64,127,128]),center+rng.choice([-128,-64,0,1,64,127,128]),rng.choice([1,2,3,4])]for n in range(20,25)]
        kind=i%2;(npcs if kind else players).append(parent);rng.shuffle(npcs);rng.shuffle(players)
        if i%3==0:npcs.insert(rng.randrange(len(npcs)+1),npcs[0].copy())
        if i%5==0:players.insert(rng.randrange(len(players)+1),players[0].copy())
        keys=[key(kind,7,31,47),key(kind,7,31,47),key(1,10,32,47),key(kind,7,31,47),key(kind,7,31,47,True),key(0,20,33,48)]
        add(npcs,players,keys,'ordered-multiple-picks')
    # Java overflow in all coordinate/size arithmetic, including signed shifts.
    for i in range(256):
        kind=i%2;size=rng.choice([1,2,3,4,8,33554432,2147483647,-2147483648,-1,0]);center=rng.choice([-2147483648,-2147483584,-64,0,64,2147483520,2147483584])
        npcs=[[10,rng.choice([-2147483648,-64,0,64,2147483584]),center,rng.choice([1,2,3,2147483647])]];players=[[20,center,center,rng.choice([1,2,3,2147483647])]]
        (npcs if kind else players).append([7,center,center,size]);add(npcs,players,[key(kind,7,127,0,True)],'signed-overflow')
    add([[7,64,64,1]],[],[key(1,7),key(2,123),key(1,7)],'scenery-key-breaks-consecutive-repeat')
    add([],[],[],'empty')
    lines=[';'.join(['/'.join(','.join(map(str,a))for a in c['npcs']),'/'.join(','.join(map(str,a))for a in c['players']),','.join(c['keys'])])for c in cases]
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(lines)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(cases)
    for case,result in zip(cases,output):case['expected']=[list(map(int,row.split(',')))for row in result.split('/')if row]
    producer_keys=[dict(kind=kind,index=index,interactive=interactive)for kind in [0,1]for index in [0,1,127,128,255,256,1023,2047]for interactive in [False,True]]
    key_lines=['K;'+','.join(map(str,[c['kind'],c['index'],int(c['interactive'])]))for c in producer_keys]
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(key_lines)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(producer_keys)
    for case,result in zip(producer_keys,output):
        key,enabled=result.split(',');case['expected']=key;case['pickable']=enabled=='1'
    fixture=dict(sources={'MiniMenu.java':hashlib.sha256(raw).hexdigest(),'ScriptRunner.java':hashlib.sha256(producer_raw).hexdigest(),'GlModel.java':hashlib.sha256(gl_raw).hexdigest()},scope=__doc__,producerKeys=producer_keys,cases=cases)
    target=root/'fixtures/ui/entity-menu.json';encoded=json.dumps(fixture,separators=(',',':'))+'\n'
    if check:assert target.read_text(encoding='utf-8')==encoded,'entity-menu fixture differs'
    else:target.write_text(encoded,encoding='utf-8')
    print(f'Original actor-overlap observations: {len(cases)}; original actor scene keys: {len(producer_keys)}',flush=True)


if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2]
    spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    generate(root,module.clean,module.method,'--check'in sys.argv)
