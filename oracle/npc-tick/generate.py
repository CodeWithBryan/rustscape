"""Verbatim NPC tick/wander/respawn bodies with seeded original random methods."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/npc-tick-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
    cases=[]
    def a(op,**kw):return dict(op=op,**kw)
    def add(actions=None,fields=None,**kw):cases.append(dict(actions=actions or[a('tick'),a('advance',ticks=1),a('tick'),a('arrival')],fields=fields or{},**kw))
    for seed in [0,1,2,3,530]:
        for radius in [-1,0,1,11,20,21,64]:
            for z in [0,1,3]:add([a('tick'),a('destination'),a('advance',ticks=1),a('tick')],dict(walks=True,radius=radius,location=[3207,3216,z],spawn=[3207,3216,z],forceTalk='hello'),seed=seed)
    for offset in [0,2,16,17,30,100]:
        for pulse in [None,0]:add(fields=dict(location=[3207+offset,3216,0],spawn=[3207,3216,0],walks=True,handler=True,aggressive=True,returnPulse=pulse),answers={'ReturnRunning':False})
    for name in ['Behavior','InteractionLocked','MovementLocked','PulseRunning','Attacking','InCombat','SelectTarget','WaterNpc','RegionActive','DialogueActive','DialogueChatbox','CanMove']:
        for value in [False,True]:add(fields=dict(walks=True,handler=True,aggressive=True,dialogue=1,forceTalk='hello'),answers={name:value},seed=2)
    for ticks in [-2147483648,-1,0,1,98,99,100,101,2147483647]:
        for respawn in [-1,100,2147483647]:add([a('tick'),a('advance',ticks=1),a('tick')],dict(respawn=respawn,respawning=True,forceTalk='hello'),ticks=ticks)
    phases=['Ticks','RegionActive','Inactivity','BehaviorRespawn','Respawn','Restore','EntityTick','Behavior','InteractionLocked','MovementLocked','PulseRunning','Attacking','InCombat','SelectTarget','Bool','Int','CanMove','WaterNpc','Find','Walk','PauseAggression','LockMovement','UnlockMovement','ImpactDisabled','Attribute','Construct','SubmitReturn','StopCombat','Chat']
    for phase in phases:
        for at in [1,2]:add(fields=dict(location=[3240,3216,0],spawn=[3207,3216,0],walks=True,aggressive=True,handler=True,forceTalk='hello'),failures={phase:at})
    for phase in ['Ticks','Behavior','MovementLocked','Bool','Int','CanMove','PauseAggression','LockMovement','Attribute','Construct','SubmitReturn','Chat']:
        for fields in [dict(radius=0),dict(location=[3220,3220,2]),dict(forceTalk=None,walks=False),dict(returnPulse=None,returning=False)]:add(fields=dict(location=[3240,3216,0],walks=True,handler=True,forceTalk='hello'),hooks=[dict(event=phase,actions=[a('set',fields=fields)])])
    for path in [None,[],[[3208,3217,0]],[[3208,3217,0],None,[3210,3219,1]]]:
        for index in [-1,0,1,2147483647]:add([a('destination'),a('destination'),a('destination')],dict(pathBound=True,path=path,pathIndex=index))
    add([a('actions'),a('arrival')],dict(location=[3250,3216,0],handler=True),hooks=[dict(event='ReturnRunning',actions=[a('set',fields=dict(radius=0))])])
    for text in ['',None,'HÃ©llo ä¸–ç•Œ']:add([a('actions'),a('advance',ticks=30),a('actions')],dict(forceTalk=text))
    # Centered NPCs enter the wander/RNG branch, unlike the far-away return cases.
    # Seeds4096/4097 begin with nextBoolean=false on the pinned JVM.
    for seed in [0,4096,4097]:
        for phase in ['Bool','Int','CanMove','WaterNpc','Find','Walk','Chat']:
            for at in [1,2]:add([a('tick'),a('advance',ticks=20),a('tick')],dict(walks=True,forceTalk='hello'),seed=seed,failures={phase:at})
    for phase in ['Bool','Int','CanMove','WaterNpc','Find','Walk','Chat']:
        for fields in [dict(radius=0),dict(location=[3220,3220,2]),dict(walks=False,forceTalk=None)]:add([a('tick'),a('advance',ticks=20),a('tick')],dict(walks=True,forceTalk='hello'),seed=4096,hooks=[dict(event=phase,actions=[a('set',fields=fields)])])
    return cases
def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
    if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
    run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java=[];kt=[]
    def adapter(name,text):
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
    npc=read(SERVER/'core/game/node/entity/npc/NPC.java')
    methods='\n'.join(c.method(npc,s)for s in ['public void tick()','public void handleTickActions()','private boolean walkToNextDest()','public void setNextWalk()','protected Location getMovementDestination()'])
    template=read(HERE/'NPC.java');adapter('core/game/node/entity/npc/NPC.java',template.replace('/* ORIGINAL_METHODS */',methods))
    location=read(SERVER/'core/game/world/map/Location.kt')
    transforms='\n'.join(line.strip()for line in location.splitlines()if'fun transform(diffX: Int, diffY: Int, z: Int)'in line or'fun transform(vector: Vector)'in line)
    adapter('core/game/world/map/Location.kt','''package core.game.world.map
import core.api.utils.Vector
import kotlin.math.*
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};val region:Region get()=Region(this);fun withinDistance(other:Location,dist:Int)=other.z==z&&getDistance(this,other)<=dist;
'''+transforms+'''
companion object{@JvmStatic fun create(x:Int,y:Int,z:Int=0)=Location(x,y,z);
'''+'@JvmStatic '+c.method(location,'fun getDistance(first: Location, second: Location)')+'};@JvmInline '+c.method(location,'value class LocationHash(')+'}\nclass Region(val location:Location){val isActive:Boolean get()=oracle.Support.query("RegionActive",true,oracle.Support.coord(location))}')
    # Original Vector's toDirection is independent of these selected methods.
    vector=read(SERVER/'core/api/utils/Vector.kt');adapter('core/api/utils/Vector.kt','package core.api.utils\nimport core.game.world.map.Location\nimport kotlin.math.*\nclass Vector(val x:Double,val y:Double){'+c.method(vector,'fun normalized()')+c.method(vector,'fun magnitude()')+c.method(vector,'operator fun times (other: Int)')+c.method(vector,'fun toLocation (plane: Int = 0)')+'companion object{'+c.method(vector,'@JvmStatic fun betweenLocs')+'}}')
    random=read(SERVER/'core/tools/RandomFunction.java')
    methods='\n'.join(c.method(random,s)for s in ['public static final int random(int a, int b)','public static final int random(int maxValue)','public static int randomize(int value)','public static boolean nextBool()'])
    adapter('core/tools/RandomFunction.java','package core.tools;public class RandomFunction{public static java.util.Random RANDOM;'+methods+'}')
    adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;import core.game.world.map.Location;public class Entity{public Location location=Location.create(3207,3216,0);public Location getLocation(){return location;}public void tick(){oracle.Support.emit("EntityTick");}public void teleport(Location l){oracle.Support.pendingTeleport=l;oracle.Support.emit("Teleport",oracle.Support.coord(l));}public void fullRestore(){oracle.Support.emit("Restore");}}''')
    adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;public class Player{public int id;public Player(int id){this.id=id;}public boolean isActive(){return oracle.Support.query("DialogueActive",true,id);}public Player getInterfaceManager(){return this;}public boolean hasChatbox(){return oracle.Support.query("DialogueChatbox",true,id);}}''')
    adapter('core/game/world/GameWorld.java','''package core.game.world;public class GameWorld{public static int getTicks(){return oracle.Support.ticks();}public static class Pulser{public void submit(core.game.interaction.MovementPulse p){oracle.Support.emit("SubmitReturn",p.id);}}public static Pulser getPulser(){return new Pulser();}}''')
    adapter('core/game/interaction/MovementPulse.java','''package core.game.interaction;import core.game.node.entity.npc.NPC;import core.game.world.map.Location;public class MovementPulse{public int id;public MovementPulse(NPC n,Location l,core.game.world.map.path.Pathfinder p){id=++oracle.Support.nextPulse;oracle.Support.emit("Construct",id,oracle.Support.coord(l));}public MovementPulse(int id){this.id=id;}public boolean isRunning(){return oracle.Support.query("ReturnRunning",true,id);}public boolean pulse(){return true;}}''')
    adapter('core/game/world/map/path/Pathfinder.java','''package core.game.world.map.path;import core.game.node.entity.npc.NPC;import core.game.world.map.Location;public class Pathfinder{public static Pathfinder SMART=new Pathfinder(),DUMB=new Pathfinder();public static Path find(NPC n,Location l,boolean near,Pathfinder f){oracle.Support.emit("Find",oracle.Support.coord(l),false);return new Path(l,false);}public static Path findWater(NPC n,Location l,boolean near,Pathfinder f){oracle.Support.emit("Find",oracle.Support.coord(l),true);return new Path(l,true);}}''')
    adapter('core/game/world/map/path/Path.java','''package core.game.world.map.path;import core.game.node.entity.npc.NPC;import core.game.world.map.Location;public class Path{Location l;boolean water;Path(Location l,boolean w){this.l=l;water=w;}public void walk(NPC n){oracle.Support.emit("Walk",oracle.Support.coord(l),water);}}''')
    read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=60)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    rows=[json.loads(l)for l in result.stdout.splitlines()if l];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim original NPC.tick/handleTickActions/walkToNextDest/setNextWalk/getMovementDestination including anonymous return-arrival body, original Vector/Location arithmetic and selected RandomFunction methods using seeded java.util.Random. Explicit region/locks/combat/dialogue/behavior/entity/attributes/pulse/path services. Compares state, destinations, seeded RNG draws, query/event order and partial failures/reentry. Full NPC class, concrete combat/content/definition/configuration, route/queue/world bindings remain separate.')
    folder=ROOT/'fixtures/npc-tick'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('NPC-tick fixtures/provenance differ')
        print(f'{len(rows)} original NPC tick/wander scenarios reproduced.')
    else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original NPC tick/wander scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
