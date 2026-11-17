"""Original GroundSpawn subtype joined to GroundItemManager and PulseRunner."""
import argparse, importlib.util, os, pathlib, sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('manager',ROOT/'oracle/ground-manager/generate.py')
manager=importlib.util.module_from_spec(spec);spec.loader.exec_module(manager)
SCOPE='Complete unchanged Kotlin GroundSpawn nested class, Java GroundItemManager/GroundItem/Pulse and Kotlin PulseRunner (deterministic clock import only). Existing chunk/packet/tradeability adapters joined. Player population/max-player reads, world runner receiver, definitions/plugins/regions/packets and no-op telemetry are explicit dependencies. Configuration file parsing, full world entry, authoritative Take and complete content remain separate.'


def transform(name,text,source,c):
    text=manager.transform(name,text,source,c)
    if name=='oracle/Support.java':
        text=text.replace('ManagerSupport.reset();','ManagerSupport.reset();SpawnSupport.reset();')
        text=text.replace('static Object act(Map a){','static Object act(Map a){SpawnSupport.remember();')
        text=text.replace('ManagerSupport.snapshot(s);','ManagerSupport.snapshot(s);if(Boolean.TRUE.equals(input.get("spawns")))SpawnSupport.snapshot(s);')
    elif name=='oracle/ManagerSupport.java':
        text=text.replace('default:throw new IllegalArgumentException((String)a.get("op"));','default:return SpawnSupport.act(a);')
    elif name=='core/game/world/GameWorld.java':
        text='package core.game.world;class GroundSpawnGameClockAdapter{}'
    elif name=='core/game/node/item/Item.java':
        text=text[:-2]+'public String getName(){return getDefinition().getName();}}\n'
    return text


def register(adapter,source,read,c):
    manager.register(adapter,source,read,c)
    _,loader=source('core/game/system/config/GroundSpawnLoader.kt')
    imports='\n'.join(line for line in loader.splitlines() if line.startswith('import ') and not any(n in line for n in ['json.simple','java.io','core.api.log','core.tools.Log']))
    adapter('core/game/system/config/GroundSpawnLoader.kt','package core.game.system.config\n'+imports+'\nclass GroundSpawnLoader{'+c.method(loader,'class GroundSpawn\n')+'}')
    _,pulse=source('core/game/system/task/Pulse.java');adapter('core/game/system/task/Pulse.java',pulse)
    _,runner=source('core/game/world/PulseRunner.kt')
    adapter('core/game/world/PulseRunner.kt',runner.replace('package core.game.world','package core.game.world\nimport core.game.world.PulseClock as System',1))
    adapter('core/game/world/PulseClock.kt','package core.game.world\nobject PulseClock{fun currentTimeMillis()=0L}')
    adapter('core/game/node/Node.java','package core.game.node;public class Node{public boolean isActive(){return true;}}')
    adapter('core/ServerConstants.kt','package core\nobject ServerConstants{var GRAFANA_LOGGING=false;val MAX_PLAYERS:Int get()=oracle.SpawnSupport.maximum()}')
    adapter('core/game/world/repository/Repository.kt','package core.game.world.repository\nobject Repository{val players:Players get()=Players();class Players{val size:Int get()=oracle.SpawnSupport.population()}}')
    adapter('core/game/world/GameWorld.kt','package core.game.world\nobject GameWorld{@JvmStatic fun getTicks():Int{val n=oracle.Support.tick;oracle.Support.emit("ticks",n);return n};val Pulser:PulseRunner get()=oracle.SpawnSupport.pulser()}')
    adapter('core/game/bots/GeneralBotCreator.java','package core.game.bots;public class GeneralBotCreator{public static class BotScriptPulse extends core.game.system.task.Pulse{public Object botScript;public boolean pulse(){return true;}}}')
    adapter('core/tools/Log.java','package core.tools;public class Log{public static final int ERR=1,WARN=2;}')
    adapter('core/tools/SystemLogger.java','package core.tools;public class SystemLogger{}')
    adapter('core/api/API.kt','package core.api\nfun log(owner:Any,level:Int,message:String){oracle.Support.emit("log",level,message)}')
    adapter('core/integrations/grafana/Grafana.kt','package core.integrations.grafana\nobject Grafana{fun addPulseLength(n:String,v:Int){};fun countPulse(n:String){};var botPulseTime=0;var otherPulseTime=0}')
    adapter('oracle/SpawnSupport.java',read(HERE/'SpawnSupport.java'))
    read(HERE/'generate.py')


def requests():
    cases=[]
    def a(op,**kw):return dict(op=op,**kw)
    def add(actions,**kw):cases.append(dict(manager=True,spawns=True,actions=actions,**kw))
    def new(rate=3,**kw):return a('spawnNew',rate=rate,location=[3201,3202,0],**kw)
    for rate in [0,1,2,65535,65536,65537,131075,2147483647,-2147483648,-1]:
        add([new(rate),a('spawnDuration'),a('spawnInit'),a('spawnDuration'),a('itemPrivate',g=0),a('itemActive',g=0),a('managerPulse'),a('sync'),a('managerDestroy',g=0),*[a('spawnWorld')for _ in range(4)]])
    for minimum,maximum in [(0,0),(1,2),(5,10),(65535,1),(-1,5),(65536,65535),(-2147483648,2147483647)]:
        for count in [0,1,1000,2000,4000,2147483647]:
            add([new(),a('spawnRate',minimum=minimum,maximum=maximum),a('spawnConfig',population=count),a('spawnDuration'),a('spawnInit'),a('spawnDuration')])
    for maximum in [0,-1,1,2000,2147483647,-2147483648]:
        for population in [0,-1,1,2000,2147483647]:
            add([new(131073),a('spawnConfig',maximum=maximum,population=population),a('spawnDuration'),a('spawnRespawn'),a('spawnWorld')])
    for event in ['maximum','population','pulser','trade','lookup','dirty','item','default','ticks']:
        add([new(),a('spawnInit'),a('managerDestroy',g=0),*[a('spawnWorld')for _ in range(4)]],failures={event:1})
    for event in ['maximum','population','pulser']:
        for change in [a('spawnRate',minimum=10,maximum=20),a('spawnConfig',maximum=0,population=0),a('spawnRunner',index=1)]:
            add([new(131073),a('spawnRespawn'),a('spawnWorld',index=0),a('spawnWorld',index=1)],hooks=[dict(event=event,actions=[change])])
    for delay in [0,1,2,3]:
        add([new(delay),a('spawnInit'),a('managerDestroy',g=0),a('managerDestroy',g=0),*[a('spawnWorld')for _ in range(5)],a('sync'),a('update'),a('resetAll')])
    add([new(1),a('spawnInit'),a('managerDestroy',g=0),a('spawnWorld'),a('spawnWorld')],hooks=[dict(event='trade',at=2,actions=[a('spawnRespawn')])])
    add([new(1),a('spawnInit'),a('managerDestroy',g=0),a('spawnWorld'),a('spawnWorld')],hooks=[dict(event='lookup',at=3,actions=[a('managerDestroy',g=0)])])
    add([new(1),a('spawnInit'),a('setGround',g=0,removed=True,remainPrivate=True,forceVisible=False),a('tick',value=2147483647),a('itemPrivate',g=0),a('itemActive',g=0),a('managerPulse'),a('sync')])
    add([new(1),a('spawnInit'),a('managerDestroy',g=0),a('spawnWorld'),a('spawnPulse',pulse=0),a('spawnPulse',pulse=0),a('managerPulse')])
    return cases


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args()
    manager.base.c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    manager.base.generate(args.check,sys.modules[__name__],'ground-spawn')
