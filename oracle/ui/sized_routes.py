"""Execute pinned PathFinder size dispatch, BFS, reach checks and move writes."""
import argparse,hashlib,importlib.util,json,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
RUN=ROOT/'runs/client-sized-routing-oracle'
FIX=ROOT/'fixtures/ui/sized-routes.json'
spec=importlib.util.spec_from_file_location('sized_visual',ROOT/'oracle/visual/generate.py')
v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)

def generate(check=False):
    reach_spec=importlib.util.spec_from_file_location('sized_reach_transcription',ROOT/'tools/port_client_reach.py')
    reach=importlib.util.module_from_spec(reach_spec);reach_spec.loader.exec_module(reach)
    if (ROOT/'crates/client/src/route_reach.rs').read_text()!=reach.generate():raise SystemExit('Sized reach source differs from pinned transcription')
    v.RUN=RUN;sources=v.generate(build_only=True)
    raw=(v.SRC/'PathFinder.java').read_bytes().replace(b'\r\n',b'\n');sources['PathFinder.java']=hashlib.sha256(raw).hexdigest();source=v.clean(raw.decode())
    fields=source[source.index('public class PathFinder'):source.index('public static boolean findPath(')]
    (RUN/'PathFinder.java').write_text(fields+''.join(v.method(source,s)for s in ['public static boolean findPath(','public static boolean findPath2(','public static boolean findPathN(','public static boolean findPath1('])+'}')
    (RUN/'SizedRouteOracle.java').write_text(DRIVER)
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*map(str,RUN.glob('*.java'))],check=True)
    rng=random.Random(20091021);cases=[];requests=[]
    def add(c):
        values=[c['size'],*c['source'],*c['destination'],*c.get('target_size',[0,0]),c.get('shape',0),c.get('angle',0),c.get('blocked',0),int(c.get('approximate',False)),c.get('move_type',0),int(c.get('run',False)),*c.get('origin',[3152,3168])]
        cells=';'.join(','.join(map(str,p))for p in c['cells'])or'-'
        requests.append(c['kind']+'\t'+','.join(map(str,values))+'\t'+cells);cases.append(c)
    masks=[1,2,4,8,16,32,64,128,256,0x40000,0x200000,0x1000000]
    directions=[[-1,0],[1,0],[0,-1],[0,1],[-1,-1],[1,-1],[-1,1],[1,1]]
    # First-generation neighbour costs isolate the original edge/corner masks.
    # A one-step route can still arrive by a detour; cost==1 distinguishes it.
    for size in [1,2,3,4,8]:
        for direction in directions:
            add(dict(kind='step',size=size,source=[40,40],destination=[40+direction[0],40+direction[1]],cells=[]))
            candidates={(x,y)for x in range(39,41+size)for y in range(39,41+size)
                        if x in [39,40,40+size-2,40+size-1,40+size]or y in [39,40,40+size-2,40+size-1,40+size]}
            for x,y in sorted(candidates):
                for mask in masks:
                    add(dict(kind='step',size=size,source=[40,40],destination=[40+direction[0],40+direction[1]],cells=[[x,y,mask]]))
    for size in [1,2,3,4,8]:
        for _ in range(32):
            destination=[rng.randrange(38,51),rng.randrange(38,51)]
            cells=[[destination[0],destination[1],256]]
            cells.extend([rng.randrange(34,56),rng.randrange(34,56),rng.choice(masks)]for _ in range(8))
            add(dict(kind='ground',size=size,source=[40,40],destination=destination,cells=cells,move_type=2,run=bool(rng.randrange(2))))
    # Valid loaded maps retain the original blocked outer five tiles. Sparse
    # authored masks exercise asymmetric wall/decor flags and blocked rectangles.
    for _ in range(1536):
        size=rng.choice([1,2,3,4,8,16]);source=[rng.randrange(12,75),rng.randrange(12,75)]
        destination=[max(2,min(95,source[0]+rng.randrange(-14,15))),max(2,min(95,source[1]+rng.randrange(-14,15)))]
        cells=[[rng.randrange(5,94),rng.randrange(5,94),rng.choice(masks)]for _ in range(rng.randrange(0,50))]
        add(dict(kind='reach',size=size,source=source,destination=destination,target_size=[rng.randrange(0,9),rng.randrange(0,9)],shape=rng.randrange(12),angle=rng.randrange(4),blocked=rng.randrange(16),cells=cells))
    # Explicitly retain the size>1 original early wall/decor condition: its last
    # clause uses target Y twice and does not test the actor's Y coordinate.
    for size in [2,3,8]:
        for shape in range(1,11):
            for y in [5,30,80,95]:
                add(dict(kind='reach',size=size,source=[40,50],destination=[40+size-1,y],shape=shape,angle=shape%4,target_size=[1,1],cells=[]))
    for size in [2,3,8,16]:
        for width in [1,2,4]:
            for destination,dims in [([39,41],[1,width]),([40+size,41],[1,width]),([41,39],[width,1]),([41,40+size],[width,1])]:
                for blocked in [0,1,2,4,8,15]:
                    add(dict(kind='reach',size=size,source=[40,40],destination=destination,target_size=dims,blocked=blocked,cells=[]))
    for size in [1,2,3,4,8,16,64]:
        for _ in range(80):
            source=[rng.randrange(5,max(6,85-size)),rng.randrange(5,max(6,85-size))]
            destination=[rng.randrange(3,96),rng.randrange(3,96)]
            cells=[[rng.randrange(1,99),rng.randrange(1,99),rng.choice(masks)]for _ in range(rng.randrange(0,100))]
            add(dict(kind='route',size=size,source=source,destination=destination,target_size=rng.choice([[0,0],[1,1],[2,3],[4,2]]),shape=rng.choice([0,0,0,1,2,3,4,7,8,9,10,11]),angle=rng.randrange(4),blocked=rng.randrange(16),approximate=bool(rng.randrange(2)),move_type=rng.choice([0,2]),run=bool(rng.randrange(2)),origin=[rng.choice([0,3152,3200,65535]),rng.choice([0,3168,3200,65535])],cells=cells))
    for size in [1,2,3,8]:
        cells=[]
        for i,x in enumerate(range(12,86,size+3)):
            for y in range(1,99):
                if y < 5+size+1 if i%2 else y >= 90-size-1:continue
                cells.append([x,y,256])
        add(dict(kind='route',size=size,source=[5,50],destination=[91-size,50],cells=cells,approximate=True,move_type=2,run=True))
    (RUN/'requests.tsv').write_text('\n'.join(requests)+'\n')
    result=subprocess.run(['java','-Xmx1g','-cp',str(RUN),'SizedRouteOracle'],input='\n'.join(requests)+'\n',text=True,capture_output=True)
    (RUN/'stdout.log').write_text(result.stdout);(RUN/'stderr.log').write_text(result.stderr)
    if result.returncode:raise SystemExit(result.stderr)
    lines=result.stdout.splitlines();assert len(lines)==len(cases),(len(lines),len(cases))
    for c,line in zip(cases,lines):c['expected']=json.loads(line)
    fixture=dict(sources=sources,adapterSha256=hashlib.sha256(DRIVER.encode()).hexdigest(),bindings='Complete unchanged PathFinder.findPath dispatcher and findPathN/2/1, complete original CollisionMap reach methods and ClientProt.sendMovePacket. Loaded104tile maps with original blocked outer five tiles; zero map offset, explicit player size/CTRL/origin/ISAAC-free output services. First-generation costs isolate masks. Malformed out-of-map probes, full scene construction, picking, actor scheduling and accepted server actions remain separate.',cases=cases)
    if check:
        if json.loads(FIX.read_text())!=fixture:raise SystemExit('Sized-route fixture differs')
    else:FIX.write_text(json.dumps(fixture,separators=(',',':'))+'\n')
    print('Original sized routing:',len(cases),'cases',flush=True)

DRIVER=r'''
import java.io.*;import java.util.*;
class SizedRouteOracle {
static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
public static void main(String[]args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));String line;
while((line=reader.readLine())!=null){String[]p=line.split("\t");int[]a=ints(p[1]);int size=a[0],x=a[1],y=a[2],tx=a[3],ty=a[4],w=a[5],h=a[6],shape=a[7],angle=a[8],blocked=a[9];CollisionMap map=new CollisionMap(104,104);for(int i=1;i<99;i++)for(int j=1;j<99;j++)map.flags[i][j]=0;if(!p[2].equals("-"))for(String c:p[2].split(";")){int[]v=ints(c);map.flags[v[0]][v[1]]=v[2];}PathFinder.collisionMaps[0]=map;
if(p[0].equals("reach")){boolean reached=x==tx&&y==ty;if(!reached&&shape!=0){if((shape<5||shape==10)&&map.isAtWall(ty,x,y,tx,shape-1,size,angle))reached=true;if(!reached&&shape<10&&map.isAtWallDecor(ty,shape-1,tx,y,size,angle,x))reached=true;}if(!reached&&w!=0&&h!=0)reached=map.isInsideOrOutsideRect(tx,y,x,size,w,blocked,ty,h);System.out.println(reached);continue;}
PlayerList.self=new Player();PlayerList.self.oracleSize=size;Camera.originX=a[13];Camera.originY=a[14];Keyboard.pressedKeys[0]=a[12]!=0;Protocol.outboundBuffer=new OutBuffer();ClientProt.lastPathLength=0;boolean found=PathFinder.findPath(y,angle,h,a[10]!=0,blocked,tx,w,shape,a[11],ty,x);if(p[0].equals("ground")&&!found)found=PathFinder.findPath(y,0,1,false,0,tx,1,0,a[11],ty,x);
if(p[0].equals("step")){System.out.println(PathFinder.costs[tx][ty]==1);continue;}
if(!found){System.out.println("null");continue;}
StringBuilder turns=new StringBuilder("[");for(int i=0;i<ClientProt.lastPathLength;i++){if(i!=0)turns.append(',');turns.append('[').append(PathFinder.queueX[i]).append(',').append(PathFinder.queueY[i]).append(']');}turns.append(']');System.out.println("{\"turns\":"+turns+",\"approximate\":"+(PathFinder.approximateDestination!=0)+",\"packet\":\""+HexFormat.of().formatHex(Protocol.outboundBuffer.out.toByteArray())+"\"}");
}}}
'''
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');generate(parser.parse_args().check)
