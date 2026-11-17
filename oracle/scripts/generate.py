"""Compare complete original Script/ScriptProcessor and script API wrappers."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/scripts-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler);read,run=compiler.read,compiler.run
def requests():
    rng=random.Random(530);cases=[]
    base={'a':dict(execution='f0',strength='WEAK'),'b':dict(execution='f1',strength='NORMAL'),'c':dict(execution='f2',strength='SOFT',persist=True),'u':dict(execution='f3',form='queuedUse',strength='STRONG'),'i':dict(execution='f0',form='interaction',distance=2),'o':dict(execution='f1',form='interaction',distance=-1),'w':dict(execution='f2',form='use',distance=3),'x':dict(form='basic',execution=None),'nil':None}
    def op(operation,**kw):return dict(op=operation,**kw)
    def add(actions,**kw):cases.append(dict(scripts=json.loads(json.dumps(kw.pop('scripts',base))),actions=actions,**kw))
    for kind in ['player','ai','other']:
        for strength in ['WEAK','NORMAL','STRONG','SOFT']:
            for persist in [True,False]:
                ts=json.loads(json.dumps(base));ts['a']['strength']=strength;ts['a']['persist']=persist
                add([op('add',script='a',strength=strength),op('add',script='b'),op('queue'),op('tick',value=1),op('queue'),op('delayScript',value=4),op('pre'),op('post',value=False),op('active'),op('tick',value=10),op('queue'),op('stopExecuting'),op('weak'),op('normal'),op('queue')],scripts=ts,kind=kind,executors={'f0':{'result':True}})
    for form in ['queued','queuedUse','basic','interaction','use']:
        for persist in [True,False]:
            for tick in [-2**31,-1,0,1,2**31-1]:
                add([op('add',script='a',strength='STRONG'),op('next',script='a',value=tick),op('queue'),op('execute',script='a'),op('active'),op('restartScript'),op('stopExecuting')],scripts={'a':dict(form=form,persist=persist,state=2**31-1,execution='f0')},tick=tick,executors={'f0':{'result':True}})
    for nodekind in ['other','npc','ground','scenery']:
        for distance in [-1,0,1,2,10]:
            add([op('setInteraction',node=1,script='i'),op('pre'),op('post',value=False),op('tick',value=1),op('pre'),op('post',value=True),op('range',value=20),op('pre'),op('post',value=False)],scripts={'i':dict(form='interaction',distance=distance,execution='f0')},nodes={'1':dict(kind=nodekind,location=[1,0,2],face=[9,8,0],center=[4,5,0])})
    for ctor in ['interaction','use','basic']:
        for kind in ['player','ai','other']:
            add([op('execute',script='i'),op('setInteraction',node=1,script='i'),op('interact',script='i'),op('tick',value=1),op('interact',script='i'),op('nodeActive',node=1,value=False),op('interact',script='i'),op('active')],scripts={'i':dict(form=ctor,execution='f0')},kind=kind)
    for who,event in [('exec:f0','invoke'),('entity','close'),('entity','animatorReset'),('entity','face'),('entity','location'),('node:1','active'),('path','find'),('los','test'),('exec:f1','equals'),('log','Error')]:
        callbacks=[op('fail'),op('reset'),op('normal'),op('weak'),op('add',script='b'),op('delayScript',value=3),op('keepRunning'),op('range',value=20),op('setInteraction',node=2,script='o'),op('destination',value=[1,0,0])]
        for cb in callbacks:
            add([op('setInteraction',node=1,script='i'),op('setInteraction',node=1,script='o'),op('tick',value=1),op('pre'),op('post',value=False),op('add',script='a'),op('add',script='b'),op('add',script='c',strength='STRONG'),op('add',script='x'),op('tick',value=4),op('queue')],hooks=[dict(who=who,event=event,actions=[cb])],nodes={'1':dict(kind='scenery',location=[1,0,0])})
    for key in ['modal','opened','chatbox','stun','running','sight']:
        for kind in ['player','ai','other']:
            add([op('setInteraction',node=1,script='i'),op('add',script='a'),op('tick',value=1),op(key,value=True),op('pre'),op('post',value=False),op(key,value=False),op('pre'),op('post',value=True)],kind=kind)
    for points,near in [([],False),([],True),([[4,5]],False),([[4,5]],True)]:
        add([op('setInteraction',node=1,script='i'),op('pre'),op('post',value=False)],nodes={'1':dict(kind='scenery',location=[1,0,0],points=points,near=near)})
    add([op('setInteraction',node=1,script='i'),op('setInteraction',node=2,script='o'),op('setInteraction',node=2,script='x'),op('setInteraction',node=1,script='x'),op('setInteraction',node=2,script='nil')],executors={'f0':{'group':'shared'},'f1':{'group':'shared'}})
    add([op('add',script='a'),op('add',script='a'),op('tick',value=1),op('queue'),op('active'),op('apiQueue',execution='f0',delay=-5,strength='STRONG'),op('tick',value=3),op('pre'),op('post',value=False)],executors={'f0':{'result':True}})
    add([op('add',script='a'),op('add',script='b'),op('tick',value=1),op('queue')],hooks=[dict(who='exec:f0',event='invoke',actions=[op('normal')])])
    add([op('setInteraction',node=1,script='i'),op('mutateLocation',name='target',value=[3,0,0]),op('approach',script='i'),op('mutateLocation',name='target',value=[1,0,0]),op('operable')],locations={'target':[1,0,0]},nodes={'1':dict(location='target')})
    add([op('setInteraction',node=1,script='i'),op('tick',value=1),op('pre')],hooks=[dict(who='los',event='test',actions=[op('reset'),op('destination',value=[1,0,0]),op('sight',value=False)]),dict(who='los',event='test',at=2,actions=[op('sight',value=True)])])
    add([op('setInteraction',node=1,script='i'),op('tick',value=1),op('pre')],hooks=[dict(who='entity',event='face',actions=[op('reset')])])
    add([op('setInteraction',node=1,script='i'),op('setInteraction',node=2,script='w')],hooks=[dict(who='exec:f2',event='equals',actions=[op('setInteraction',node=1,script='o')])])
    for x,y,z in [(16383,-16384,-1),(-16384,16383,4),(2**31-1,-2**31,2**31-1)]:
        add([op('setInteraction',node=1,script='i'),op('approach',script='i'),op('operable')],own=[x,y,z],nodes={'1':dict(location=[x+1,y,z])})
    for _ in range(60):
        actions=[]
        for _ in range(20):
            name=rng.choice(['add','queue','execute','interact','setInteraction','pre','post','reset','weak','normal','active','has','approach','operable','state','next','tick','delay','delayScript','delayEntity','range','restartScript','stopExecuting','keepRunning','apiQueue'])
            actions.append(op(name,script=rng.choice(list(base)),execution=rng.choice(['f0','f1']),node=rng.choice([1,2,3,4]),strength=rng.choice(['WEAK','NORMAL','STRONG','SOFT']),value=bool(rng.randrange(2))if name=='post'else rng.choice([-2,-1,0,1,3,100])))
        add(actions,kind=rng.choice(['player','ai','other']),executors={'f0':{'result':bool(rng.randrange(2))},'f1':{'result':bool(rng.randrange(2))}})
    for kind in ['player','ai','other']:
        add([op('aliasProcessor',name='alias'),op('delay',value=11,processor='alias'),op('newProcessor',name='fresh'),op('delay',value=22,processor='fresh'),op('sameProcessor',other='alias'),op('sameProcessor',processor='fresh',other='main'),op('readProcessor',processor='alias'),op('readProcessor',processor='fresh'),op('processorEntity',processor='fresh')],kind=kind)
        for outer in ['alias','fresh']:
            for phase in ['resetAnimator','ticks','invoke','equals']:
                who='exec:f0'if phase in ['invoke','equals']else'world'if phase=='ticks'else'entity'
                add([op('aliasProcessor',name='alias'),op('newProcessor',name='fresh'),op('setInteraction',processor=outer,node=1,script='i'),op('execute',processor=outer,script='a'),op('readProcessor',processor=outer),op('readProcessor')],kind=kind,hooks=[dict(who=who,event=phase,actions=[op('delay',processor='alias',value=7),op('range',processor=outer,value=6),op('reset',processor='alias')])])
        for outer in ['alias','fresh']:
            add([op('aliasProcessor',name='alias'),op('newProcessor',name='fresh'),op('add',processor=outer,script='a'),op('add',processor=outer,script='b'),op('tick',value=2),op('queue',processor=outer),op('readProcessor',processor=outer),op('readProcessor')],kind=kind,executors={'f0':{'result':True},'f1':{'result':True}},hooks=[dict(who='exec:f0',event='invoke',actions=[op('weak',processor='alias'),op('delay',processor='fresh',value=9)])])
    return cases
def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        p=ROOT/'reference'/part
        if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',p,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);java=[];kt=[];jars=compiler.artifacts();jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    def adapter(name,text):
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(p)
    node='package core.game.node;import core.game.world.map.Location;public class Node{public final int id;public Node(int id){this.id=id;}public Location getLocation(){return oracle.Model.nodeLocation(id);}public Location getCenterLocation(){return oracle.Model.center(id);}public Location getFaceLocation(Location l){return oracle.Model.faceLocation(id,l);}public boolean isActive(){return oracle.Model.active(id);}public String toString(){return oracle.Model.describe(id);}}'
    adapter('core/game/node/Node.java',node)
    adapter('core/game/node/entity/Entity.java','package core.game.node.entity;import core.game.interaction.ScriptProcessor;import core.game.world.map.Location;public class Entity extends core.game.node.Node{public final ScriptProcessor scripts=new ScriptProcessor(this);public final int[] clocks=new int[8];public Entity(int id){super(id);}public boolean delayed(){return oracle.Model.delayed(this);}public Location getLocation(){return oracle.Model.ownLocation();}public Walking getWalkingQueue(){return new Walking();}public static class Walking{public boolean isRunning(){return oracle.Model.flag("running");}}}')
    adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player;public class Player extends core.game.node.entity.Entity{public Player(){super(0);}public boolean hasModalOpen(){return oracle.Model.flag("modal");}public Iface getInterfaceManager(){return new Iface();}public static class Iface{public boolean isOpened(){return oracle.Model.flag("opened");}public boolean hasChatbox(){return oracle.Model.flag("chatbox");}}}')
    for path,parent in [('core/game/bots/AIPlayer','core.game.node.entity.player.Player'),('core/game/node/entity/npc/NPC','core.game.node.entity.Entity'),('core/game/node/scenery/Scenery','core.game.node.Node'),('core/game/node/item/GroundItem','core.game.node.Node')]:
        package,name=path.rsplit('/',1);constructor='public AIPlayer(){super();}'if name=='AIPlayer'else'public '+name+'(int id){super(id);}'
        adapter(path+'.java','package '+package.replace('/','.')+';public class '+name+' extends '+parent+'{'+constructor+'}')
    adapter('core/game/interaction/DestinationFlag.java','package core.game.interaction;import core.game.node.Node;import core.game.node.entity.Entity;import core.game.world.map.Location;public class DestinationFlag{private final String flag;private DestinationFlag(String s){flag=s;}public static final DestinationFlag ENTITY=new DestinationFlag("Entity"),ITEM=new DestinationFlag("Item");public Location getDestination(Entity e,Node n){return oracle.Model.dest(flag,n.id);}}')
    adapter('core/game/world/map/path/Pathfinder.java','package core.game.world.map.path;import core.game.node.Node;import core.game.node.entity.Entity;public class Pathfinder{public static Path find(Entity e,Node n){return oracle.Model.path(n==null?null:n.id);}}')
    adapter('core/game/world/map/path/Path.kt','package core.game.world.map.path\ndata class Point(val x:Int,val y:Int)\nclass Path(val points:List<Point>,val isMoveNear:Boolean)')
    location=read(SERVER/'core/game/world/map/Location.kt');nested=compiler.method(location,'value class LocationHash(');dist=compiler.method(location,'fun getDistance(first: Location, second: Location)');card=compiler.method(location,'val cardinalTiles: ArrayList<Location> get()')
    adapter('core/game/world/map/Location.kt','package core.game.world.map\nimport kotlin.math.sqrt\nclass Location(x:Int,y:Int,z:Int=0):core.game.node.Node(-1){var hash=LocationHash(x,y,(z%4+4)%4);var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,this.z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,this.z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};override fun getLocation()=this;fun getDistance(other:Location)=getDistance(this,other);fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);'+card+'\n'+compiler.method(location,'override fun equals(other: Any?)')+'\noverride fun hashCode():Int=hash.hash;\ncompanion object{fun create(x:Int,y:Int,z:Int)=Location(x,y,z);'+dist+'}\n@JvmInline '+nested+'}')
    adapter('core/tools/Log.kt','package core.tools\nenum class Log{ERR,FINE}')
    adapter('core/game/world/GameWorld.kt','package core.game.world\nobject GameWorld{@JvmStatic val ticks:Int get()=oracle.Model.ticks()}')
    content=read(SERVER/'core/api/ContentAPI.kt');methods=['fun finishedMoving(entity: Entity)','fun delayScript(entity: Entity','fun delayEntity(entity: Entity','fun apRange(entity: Entity','fun clearScripts(entity: Entity','fun restartScript(entity: Entity','fun keepRunning(entity: Entity','fun stopExecuting(entity: Entity','fun queueScript(entity: Entity']
    adapter('core/api/API.kt','package core.api\nimport core.game.node.Node\nimport core.game.node.entity.Entity\nimport core.game.node.entity.player.Player\nimport core.game.world.GameWorld\nimport core.game.world.map.Location\nimport core.game.interaction.*\nimport core.tools.Log\nfun getWorldTicks()=GameWorld.ticks\nfun log(owner:Any,l:Log,s:String)=oracle.Model.log(owner,if(l==Log.ERR)"Error" else "Fine",s)\nfun resetAnimator(e:Player)=oracle.Model.simple("animatorReset")\nfun closeAllInterfaces(e:Player)=oracle.Model.simple("close")\nfun isStunned(e:Entity)=oracle.Model.flag("stun")\nfun sendMessage(e:Player,s:String)=oracle.Model.message(s)\nfun face(e:Entity,l:Location)=oracle.Model.face(l)\nfun hasLineOfSight(e:Entity,n:Node)=oracle.Model.los(n)\nfun lock(e:Entity,t:Int)=oracle.Model.lock(t)\n'+'\n'.join(compiler.method(content,m)for m in methods))
    for name in ['Script.kt','ScriptProcessor.kt','Clocks.kt']:
        p=SERVER/'core/game/interaction'/name;read(p);kt.append(p)
    read(HERE/'Support.kt');kt.append(HERE/'Support.kt')
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    file=RUN/'input.jsonl';cases=requests();file.write_bytes(''.join(json.dumps(c,sort_keys=True)+'\n'for c in cases).encode());exe=compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if compiler.JAVA_HOME else'java'
    with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.SupportKt'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in p.stdout.splitlines()if line]
    if len(output)!=len(cases):raise RuntimeError('Missing script cases')
    data=''.join(json.dumps(c,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for c in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    initial=json.loads(read(HERE/'initial.lock.json'));assert hashlib.sha256(b''.join(data.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256']
    provenance=dict(format=1,initialBaseline=initial,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged Script.kt, ScriptProcessor.kt and Clocks.kt; verbatim ContentAPI script wrappers. Explicit entity/node/interface/animator/lock/pathfinding/LOS/clock/log/executor bindings, including function equality and deterministic callback exception text. Original LocationHash, distance, equality and cardinal-tile algorithms in a location adapter. No full nodes, pathfinding, combat, interfaces, event dispatch, concrete script content, or world-entry claim.')
    folder=ROOT/'fixtures/scripts'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Script fixtures/provenance differ')
        print(f'{len(output)} original script processor/API scenarios reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original script scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();compiler.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
