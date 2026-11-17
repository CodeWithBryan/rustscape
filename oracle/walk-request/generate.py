"""Verbatim processWalkPacket and anonymous arrival body with mutable services."""
import argparse, hashlib, importlib.util, itertools, json, os, pathlib, subprocess, urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]; HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/walk-request-oracle'; SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(name,**kw):return dict(op=name,**kw)
def requests():
    rows=[]
    def add(actions,**kw):rows.append(dict(actions=actions,**kw))
    for kind in ['world','interact','minimap']:
        for running in [False,True]:
            for locked,opened,open_walkable,chatbox,chat_walkable,close,dialogue in itertools.product([False,True],repeat=7):
                add([op('process',kind=kind,running=running),op('arrival')],locked=locked,opened=opened,openWalkable=open_walkable,chatbox=chatbox,chatWalkable=chat_walkable,close=close,dialogue=dialogue)
    for kind in ['world','interact','minimap','other']:
        for plane in range(4):
            for x,y in [(0,0),(1,0),(24,0),(25,0),(26,0),(15,20),(-15,-20),(-40,40)]:
                add([op('process',kind=kind,x=3207+x,y=3216+y,running=True),op('arrival')],location=[3207,3216,plane])
    for maximum in [-2147483648,-1,0,1,14,25,2147483647]:
        for target in [[3207,3216],[3232,3216],[3233,3216],[-16384,16383]]:
            add([op('process',x=target[0],y=target[1],running=True),op('arrival')],maxDistance=maximum,location=[3207,3216,3])
    for null in ['player','location','openWalkable','chatWalkable']:
        add([op('process'),op('arrival')],**{null:None},opened=True,chatbox=True)
    for event in ['locked','closeInterface','closeChatbox','dialogue','debug','minimap','setRunning','face','faceLocation','scripts','construct','run']:
        for action in [op('fail'),op('set',location=[3220,3221,2],chatbox=False),op('set',dialogue=False),op('set',maxDistance=1)]:
            add([op('process',running=True),op('arrival'),op('process',kind='interact',running=False)],opened=True,chatbox=True,hooks=[dict(event=event,actions=[action])])
    add([op('process',running=True),op('process',x=3210,y=3217,running=False),op('arrival'),op('arrival',index=0)])
    add([op('process',running=True),op('arrival')],hooks=[dict(event='construct',actions=[op('process',kind='interact',running=False)])])
    return rows
def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    p=ROOT/'reference/server'
    if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
    run(['git','-C',p,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java=[];kt=[]
    def adapter(name,text):
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
    source=read(SERVER/'core/net/packet/PacketProcessor.kt')
    body=c.method(source,'private fun processWalkPacket(pkt: Packet)')
    adapter('core/net/packet/PacketProcessor.kt','''package core.net.packet
import core.game.node.entity.player.Player
import core.game.world.map.Location
import core.api.utils.Vector
import core.game.interaction.MovementPulse
import core.ServerConstants
import core.net.packet.`in`.Packet
object PacketProcessor {
@JvmStatic fun handle(pkt:Packet){processWalkPacket(pkt)}
fun sendClearMinimap(player:Player){oracle.Support.emit("minimap")}
'''+body+'\n}')
    location=read(SERVER/'core/game/world/map/Location.kt')
    adapter('core/game/world/map/Location.kt','''package core.game.world.map
import core.api.utils.Vector
import kotlin.math.floor
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;
'''+next(line.strip()for line in location.splitlines()if'fun transform(vector: Vector)'in line)+'''
companion object{@JvmStatic @JvmOverloads fun create(x:Int,y:Int,z:Int=0)=Location(x,y,z)};@JvmInline '''+c.method(location,'value class LocationHash(')+'}')
    p=SERVER/'core/api/utils/Vector.kt';read(p);kt.append(p)
    adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST,NORTH,NORTH_EAST,WEST,EAST,SOUTH_WEST,SOUTH,SOUTH_EAST;}')
    adapter('core/ServerConstants.java','package core;public class ServerConstants{public static int MAX_PATHFIND_DISTANCE=25;}');read(SERVER/'core/ServerConstants.kt')
    adapter('core/net/packet/in/Packet.java','''package core.net.packet.in;import core.game.node.entity.player.Player;public class Packet{public static class WorldspaceWalk extends Packet{public Player player;public int destX,destY;public boolean isRun;}public static class InteractWalk extends Packet{public Player player;public int destX,destY;public boolean isRun;}public static class MinimapWalk extends Packet{public Player player;public int destX,destY;public boolean isRun;}}''')
    adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;import core.game.world.map.Location;import core.game.interaction.MovementPulse;
public class Player{public Location location;public boolean locked,opened,chatbox,running;public Boolean openWalkable=false,chatWalkable=false;public Scripts scripts=new Scripts();public Location getLocation(){return location;}public Locks getLocks(){return new Locks();}public Interfaces getInterfaceManager(){return new Interfaces();}public Dialogue getDialogueInterpreter(){return new Dialogue();}public Walking getWalkingQueue(){return new Walking();}public Manager getPulseManager(){return new Manager();}public void face(Object e){oracle.Support.emit("face");}public void faceLocation(Location l){oracle.Support.emit("faceLocation");}public void debug(String s){oracle.Support.emit("debug",s);}
public class Locks{public boolean isMovementLocked(){oracle.Support.emit("locked");return locked;}}
public class Interfaces{public boolean isOpened(){return opened;}public UI getOpened(){return new UI(openWalkable);}public boolean hasChatbox(){return chatbox;}public UI getChatbox(){return new UI(chatWalkable);}public boolean close(){oracle.Support.emit("closeInterface");return oracle.Support.answer("close");}public void closeChatbox(){oracle.Support.emit("closeChatbox");}}
public static class UI{Boolean walkable;UI(Boolean w){walkable=w;}public Definition getDefinition(){return walkable==null?null:new Definition(walkable);}}public static class Definition{boolean walkable;Definition(boolean w){walkable=w;}public boolean isWalkable(){return walkable;}}
public class Dialogue{public boolean close(){oracle.Support.emit("dialogue");return oracle.Support.answer("dialogue");}}
public class Walking{public boolean isRunning(){return running;}public void setRunning(boolean r){running=r;oracle.Support.emit("setRunning",r);}}
public static class Scripts{public void reset(){oracle.Support.emit("scripts");}}
public static class Manager{public void run(MovementPulse p){oracle.Support.emit("run");oracle.Support.pulses.add(p);}}
}''')
    adapter('core/game/interaction/MovementPulse.java','''package core.game.interaction;import core.game.node.entity.player.Player;import core.game.world.map.Location;public abstract class MovementPulse{public Player player;public Location location;public boolean force;public MovementPulse(Player p,Location l,boolean r){player=p;location=l;force=r;oracle.Support.emit("construct",oracle.Support.coord(l),r);}public abstract boolean pulse();}''')
    read(HERE/'Support.java');java.append(HERE/'Support.java')
    cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8')
    exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases)
    data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim PacketProcessor.processWalkPacket including anonymous arrival pulse, unchanged Vector and original packed Location/transform methods. Explicit mutable lock/interface/dialogue/script/debug/minimap/queue-running services and recording movement constructor/manager. Compares callback order, cancellation, strict distance truncation, plane-zero transformation, captured run flags, reentrancy, nulls and failure prefixes. Full packet queue, real MovementPulse/manager/search/walking/world composition and live handling remain separate.')
    folder=ROOT/'fixtures/walk-request'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Walk request fixtures/provenance differ')
        print(f'{len(rows)} original walk-request scenarios reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original walk-request scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
