"""Unchanged original minimap movement prefix and click trailer on controlled routes."""
import argparse,hashlib,importlib.util,json,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]

def generate(check=False):
    spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    run=ROOT/'runs/minimap-wire-oracle';run.mkdir(parents=True,exist_ok=True);sources={}
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    client=source('ClientProt');menu=source('MiniMenu');buffer=source('Buffer')
    (run/'ClientProt.java').write_text('class ClientProt{static int MOVE_GAMECLICK=215,MOVE_MINIMAPCLICK=39;'+v.method(client,'public static void sendMovePacket(')+'}',encoding='utf-8')
    (run/'MiniMenu.java').write_text('class MiniMenu{static int WALK_HERE=60,clickOffsetY;static void setWalkDestination(int p,int x,int y){}static void action(int menuArg1,int menuArg2){int actionCode=60,keyInt=1;'+v.method(menu,'if (actionCode == WALK_HERE)')+'}}',encoding='utf-8')
    (run/'Buffer.java').write_text('class Buffer{byte[]data=new byte[2048];int offset;void p1isaac(int n){p1(n);}'+''.join(v.method(buffer,s)for s in ['public final void p1(', 'public final void p1add(', 'public final void p1sub(', 'public final void p2(', 'public final void p2add('])+'}',encoding='utf-8')
    code='''import java.io.*;import java.util.*;
class Protocol{static Buffer outboundBuffer;}
class Camera{static int originX,originY;static float yawTarget;}
class Keyboard{static final int KEY_CTRL=0,KEY_SHIFT=1;static boolean[]pressedKeys=new boolean[2];}
class LoginManager{static int mapFlagX,mapFlagY,staffModLevel;}
class MiniMap{static int compassAngleOffset,zoomOffset;}
class InterfaceList{static int clickOffsetX;}
class Player{static int plane;int xFine,yFine;int[]movementQueueX={0},movementQueueY={0};}class PlayerList{static Player self=new Player();}
class PathFinder{static int[]queueX,queueY;static int approximateDestination;static boolean findPath(int a,int b,int c,boolean d,int e,int f,int g,int h,int kind,int i,int j){ClientProt.sendMovePacket(queueX.length,kind);return true;}}
class API{static boolean IsRoofVisibilityActive(){return false;}static void ClearDestinationRoofTarget(){}static void SetDestinationRoofTarget(int x,int y){}}
class Cheat{static void teleport(int x,int y,int p){throw new AssertionError("staff cheat excluded");}}
public class Probe{public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";");int[]n=Arrays.stream(p[0].split(",")).mapToInt(Integer::parseInt).toArray();int[]turns=Arrays.stream(p[1].split(",")).mapToInt(Integer::parseInt).toArray();PathFinder.queueX=new int[turns.length/2];PathFinder.queueY=new int[turns.length/2];for(int i=0;i<turns.length/2;i++){PathFinder.queueX[i]=turns[i*2];PathFinder.queueY[i]=turns[i*2+1];}
Protocol.outboundBuffer=new Buffer();Keyboard.pressedKeys[0]=n[0]!=0;Camera.originX=n[1];Camera.originY=n[2];InterfaceList.clickOffsetX=n[3];MiniMenu.clickOffsetY=n[4];Camera.yawTarget=n[5];MiniMap.compassAngleOffset=n[6];MiniMap.zoomOffset=n[7];PlayerList.self.xFine=n[8];PlayerList.self.yFine=n[9];PathFinder.approximateDestination=n[10];MiniMenu.action(PathFinder.queueX[0],PathFinder.queueY[0]);System.out.println(HexFormat.of().formatHex(Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset)));}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[];inputs=[]
    for _ in range(1024):
        args=[rng.randrange(2),rng.randrange(16000),rng.randrange(16000),rng.randrange(256),rng.randrange(256),rng.randrange(2048),rng.randrange(-60,61),rng.randrange(-20,21),rng.randrange(13313),rng.randrange(13313),rng.randrange(2)]
        turns=[[rng.randrange(104),rng.randrange(104)]for _ in range(rng.randrange(1,41))];cases.append(dict(args=args,turns=turns));inputs.append(','.join(map(str,args))+';'+','.join(str(v)for p in turns for v in p))
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(cases)
    for c,line in zip(cases,output):c['expected']=line
    data=(json.dumps(dict(sources=sources,bindings='Unchanged ClientProt.sendMovePacket and full MiniMenu WALK_HERE branch. Successful pathfinder supplies controlled compressed turns, no-op roof plugin, staff0, original CTRL run input, ISAAC disabled as pinned profile. BFS/collision and real UI/world integration are separate.',cases=cases),separators=(',',':'))+'\n').encode();path=ROOT/'fixtures/minimap/wire.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original minimap wire corpus differs')
    else:path.write_bytes(data)
    print(f'Original minimap39 packets: {len(cases)} full prefix/trailer cases')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
