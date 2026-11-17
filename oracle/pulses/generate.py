"""Run unchanged original Pulse, PulseManager and PulseRunner with explicit world bindings."""
import argparse,hashlib,importlib.util,itertools,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/pulses-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler);read,run=compiler.read,compiler.run
def requests():
    rng=random.Random(530);cases=[]
    base=dict(a=dict(delay=2,checks=[0,1]),b=dict(delay=1,finish=True,**{'class':'movement'}),c=dict(delay=3,**{'class':'combat'}),d=dict(delay=0,**{'class':'bot'}),s=dict(delay=1,**{'class':'script'}),nil=None)
    def op(name,**kw):return dict(op=name,**kw)
    def add(actions,**kw):cases.append(dict(hashes=kw.pop('hashes',[1,2,3,4]),pulses=json.loads(json.dumps(kw.pop('pulses',base))),actions=actions,**kw))
    for delay in [-2**31,-1,0,1,2,2**31-1]:
        for ticks in [-2**31,-1,0,1,2**31-1]:
            ts={'a':dict(delay=delay,ticks=ticks,checks=[None,0,1]),'nil':None}
            add([op('update',pulse='a'),op('update',pulse='a'),op('stop',pulse='a'),op('update',pulse='a'),op('start',pulse='a'),op('restart',pulse='a'),op('runBase',pulse='a'),op('removeFor',pulse='a',text=None)],pulses=ts)
    for kind in ['default','delay',None]:add([op('getNode',pulse='a',index=0),op('setNode',pulse='a',index=-1,node=1),op('inactive',pulse='a'),op('update',pulse='a')],pulses={'a':dict(ctor=kind,checks=[None,2],delay=3)})
    for field in ['inactiveResult','runningResult','updateResult','startBase','stopBase','removeResult','finish']:
        for value in [True,False]:
            ts=json.loads(json.dumps(base));ts['a'][field]=value
            add([op('managerRun',pulse='a'),op('world'),op('world'),op('clear'),op('has'),op('moving'),op('managerRun',pulse='b'),op('world')],pulses=ts)
    for event in ['pulse','update','inactive','isRunning','start','stop']:
        callbacks=[op('fail'),op('stop',pulse='b'),op('submit',pulse='a'),op('managerRun',pulse='c',kind='CUSTOM_1'),op('clear'),op('ticks',pulse='a',value=2**31-1),op('delay',pulse='a',value=-1),op('running',pulse='a',value=False),op('replaceChecks',pulse='a',value=None)]
        for action in callbacks:
            add([op('managerRun',pulse='a'),op('world'),op('world'),op('clear'),op('world'),op('has')],scripts=[dict(who='a',event=event,actions=[action])])
    for event in ['weak','normal']:
        for action in [op('fail'),op('managerRun',pulse='c',kind='CUSTOM_1'),op('mapPut',pulse='b',kind='STANDARD'),op('clearKind',kind='STANDARD')]:
            add([op('managerRun',pulse='a'),op('managerRun',pulse='b',kind='STRONG'),op('clear'),op('world')],scripts=[dict(who='scripts',event=event,actions=[action])])
    for hashes in itertools.permutations([1,2,3,4]):
        add([op('managerRun',pulse='a'),op('managerRun',pulse='b',kind='CUSTOM_1'),op('managerRun',pulse='c',kind='CUSTOM_2'),op('clear'),op('managerRun',pulse='d',kind='STRONG'),op('current'),op('has'),op('moving'),op('managerRun',pulse='a'),op('current'),op('clearKind',kind='STRONG'),op('world')],hashes=list(hashes))
    for hashes in [[0,0,0,0],[-2**31,2**31-1,-1,0],[65536,1,65537,0]]:
        for callback in [op('mapPut',pulse='b',kind='CUSTOM_1'),op('clearKind',kind='STANDARD'),op('mapPut',pulse='a',kind='STANDARD'),op('managerRun',pulse='c',kind=None)]:
            add([op('mapPut',pulse='a',kind='STANDARD'),op('mapPut',pulse='b',kind='CUSTOM_2'),op('managerRun',pulse='c'),op('clear'),op('world')],hashes=hashes,scripts=[dict(who='a',event='isRunning',actions=[callback])])
    for elapsed in [-1,0,29,30,99,100,2**31,2**32+30,2**63-1]:
        add([op('submit',pulse=t)for t in ['a','d','s']]+[op('world'),op('world')],times=[0,elapsed,0,elapsed,0,elapsed],logging=True)
    for logging,db in itertools.product([True,False],repeat=2):
        for who,event in [('metrics','length'),('metrics','count'),('metrics','bots'),('metrics','others'),('log','Warn'),('log','Error'),('clock','sample')]:
            add([op('submit',pulse='a'),op('submit',pulse='b'),op('world'),op('world')],logging=logging,db=db,step=100,scripts=[dict(who=who,event=event,actions=[op('fail')])])
    add([op('managerRun',pulse='a'),op('unhandled',kind='STRONG'),op('current'),op('has'),op('world'),op('world')])
    add([op('unhandled'),op('node',node=0,value=False),op('world'),op('has'),op('cancelDeath'),op('dead',value=True),op('cancelDeath')])
    add([op('managerRun',pulse='nil'),op('mapPut',kind='CUSTOM_1',pulse='b'),op('moving'),op('managerRun',pulse='c',kind=None),op('clear'),op('world'),op('submit',pulse='nil')])
    add([op('submit',pulse='a'),op('world'),op('world')],scripts=[dict(who='a',event='pulse',actions=[op('world')])])
    add([op('update',pulse='a'),op('update',pulse='b'),op('setNode',pulse='b',index=1,node=2),op('inactive',pulse='a')],arrays={'x':[0,1]},pulses={'a':dict(array='x'),'b':dict(array='x')})
    for replacement in [None,[],[0],[0,2]]:
        add([op('submit',pulse='a'),op('world'),op('inactive',pulse='a')],scripts=[dict(who='node:0',event='active',actions=[op('replaceChecks',pulse='a',value=replacement)])])
    add([op('submit',pulse='d'),op('world')],pulses={'d':dict(**{'class':'bot'},scriptNull=True)},step=100)
    for _ in range(70):
        actions=[]
        for _ in range(25):
            name=rng.choice(['update','managerRun','world','clear','clearKind','current','has','moving','start','stop','ticks','delay','submit','unhandled','dead','cancelDeath'])
            actions.append(op(name,pulse=rng.choice(['a','b','c','d','s','nil']),kind=rng.choice(['STANDARD','STRONG','CUSTOM_1','CUSTOM_2',None]),value=bool(rng.randrange(2))if name=='dead'else rng.choice([-2**31,-1,0,1,2**31-1])))
        add(actions,hashes=rng.choices([-2**31,-1,0,1,17,65536,2**31-1],k=4),step=rng.choice([-1,0,30,100]))
    return cases

def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        path=ROOT/'reference'/part
        if run(['git','-C',path,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',path,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=compiler.artifacts();jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    stdlib=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));cp=os.pathsep.join(map(str,[classes,stdlib,jar]));compile_cp=os.pathsep.join(map(str,[stdlib,jar]));java=[];kt=[]
    def adapter(name,text):
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(path)
    enum=read(SERVER/'core/game/node/entity/impl/PulseType.java');names=['STANDARD','STRONG','CUSTOM_1','CUSTOM_2']
    if not all(n in enum for n in names):raise RuntimeError('PulseType declarations differ')
    adapter('core/game/node/entity/impl/PulseType.java','package core.game.node.entity.impl;public final class PulseType{private final String name;private final int index;private PulseType(String n,int i){name=n;index=i;}'+''.join('public static final PulseType '+n+'=new PulseType("'+n+'",'+str(i)+');'for i,n in enumerate(names))+'public static PulseType[] values(){return new PulseType[]{STANDARD,STRONG,CUSTOM_1,CUSTOM_2};}public static PulseType valueOf(String s){for(PulseType p:values())if(p.name.equals(s))return p;throw new IllegalArgumentException(s);}public int hashCode(){return oracle.PulseOracle.enumHash(index);}public String toString(){return name;}}')
    adapters={
    'core/game/node/Node.java':'package core.game.node;public class Node{public final int id;public boolean active;public Node(int id,boolean active){this.id=id;this.active=active;}public boolean isActive(){return oracle.PulseOracle.nodeActive(this);}}',
    'core/game/node/entity/Entity.java':'package core.game.node.entity;import core.game.node.entity.impl.PulseManager;public class Entity extends core.game.node.Node{public final Scripts scripts=new Scripts();private final PulseManager manager=new PulseManager(this);public Entity(){super(0,true);}public PulseManager getPulseManager(){return manager;}public static class Scripts{public void removeWeakScripts(){oracle.PulseOracle.hook("scripts","weak");}public void removeNormalScripts(){oracle.PulseOracle.hook("scripts","normal");}}}',
    'core/game/node/entity/player/Player.java':'package core.game.node.entity.player;public class Player extends core.game.node.entity.Entity{public Dispatch getPacketDispatch(){return new Dispatch();}public class Dispatch{public void sendMessage(String s){oracle.PulseOracle.hook("player","message",s);}}}',
    'core/game/node/entity/combat/DeathTask.java':'package core.game.node.entity.combat;public class DeathTask{public static boolean isDead(core.game.node.entity.Entity e){oracle.PulseOracle.hook("entity","dead");return oracle.PulseOracle.dead;}}',
    'core/game/interaction/MovementPulse.java':'package core.game.interaction;public class MovementPulse extends oracle.TestPulse{public MovementPulse(int d,core.game.node.Node[] checks){super(d,checks);}}',
    'core/game/node/entity/combat/CombatPulse.java':'package core.game.node.entity.combat;public class CombatPulse extends oracle.TestPulse{public CombatPulse(int d,core.game.node.Node[] checks){super(d,checks);}}',
    'core/game/bots/GeneralBotCreator.java':'package core.game.bots;public class GeneralBotCreator{public static class Script{}public static class BotScriptPulse extends oracle.TestPulse{public Script botScript;public BotScriptPulse(int d,core.game.node.Node[] c,Script script){super(d,c);botScript=script;}}}',
    'oracle/ScriptAPIPulse.java':'package oracle;public class ScriptAPIPulse extends TestPulse{public ScriptAPIPulse(int d,core.game.node.Node[] c){super(d,c);}}',
    'core/game/world/GameWorld.java':'package core.game.world;public class GameWorld{public static PulseRunner getPulser(){return oracle.PulseOracle.runner;}}',
    'core/ServerConstants.java':'package core;public class ServerConstants{public static boolean GRAFANA_LOGGING;}',
    'core/tools/Log.java':'package core.tools;public class Log{public static final int WARN=1,ERR=2;}',
    'core/tools/SystemLogger.java':'package core.tools;public class SystemLogger{}',
    'core/api/API.kt':'package core.api\nfun log(owner:Any,level:Int,message:String)=oracle.PulseOracle.log(level,message)',
    'core/game/world/System.kt':'package core.game.world\nobject PulseClock{fun currentTimeMillis()=oracle.PulseOracle.clock()}',
    'core/integrations/grafana/Grafana.kt':'package core.integrations.grafana\nobject Grafana{fun addPulseLength(n:String,v:Int)=oracle.PulseOracle.length(n,v);fun countPulse(n:String)=oracle.PulseOracle.count(n);var botPulseTime:Int get()=oracle.PulseOracle.botTotal;set(v){oracle.PulseOracle.botTotal(v)};var otherPulseTime:Int get()=oracle.PulseOracle.otherTotal;set(v){oracle.PulseOracle.otherTotal(v)}}'
    }
    for name,text in adapters.items():adapter(name,text)
    for path in [SERVER/'core/game/system/task/Pulse.java',SERVER/'core/game/node/entity/impl/PulseManager.java',HERE/'TestPulse.java',HERE/'PulseOracle.java']:read(path);java.append(path)
    runner=SERVER/'core/game/world/PulseRunner.kt';runner_text=read(runner)
    adapter('core/game/world/PulseRunner.kt',runner_text.replace('package core.game.world','package core.game.world\nimport core.game.world.PulseClock as System',1))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java])
    run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    file=RUN/'input.jsonl';cases=requests();file.write_bytes(''.join(json.dumps(c,sort_keys=True)+'\n'for c in cases).encode())
    exe=compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if compiler.JAVA_HOME else'java'
    with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.PulseOracle'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in p.stdout.splitlines()if line]
    if len(output)!=len(cases):raise RuntimeError('Missing pulse cases')
    data=''.join(json.dumps(c,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for c in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unmodified Pulse.java/PulseManager.java; complete PulseRunner class with a single added System import alias for the controlled clock, otherwise verbatim source. Explicit node/entity/script/packet/death/telemetry/content-class adapters. PulseType values/identity equality preserved by a class adapter with controlled enum hash codes, making HashMap callback order reproducible. No actual content pulses, full Node, Grafana persistence, thread interruption, world lifecycle or live gameplay claim.')
    folder=ROOT/'fixtures/pulses'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Pulse fixture/provenance differs; review before regeneration')
        print(f'{len(output)} original pulse/manager/runner scenarios reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original pulse scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();compiler.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
