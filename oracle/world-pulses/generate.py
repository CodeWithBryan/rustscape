"""Original mixed entity/scenery pulses sharing one PulseRunner prefix."""
import argparse,importlib.util,os,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('interaction_compiler',ROOT/'oracle/interaction-runtime/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
FIXTURE='world-pulses';MAIN='oracle.WorldSupport'
SCOPE='Unchanged complete InteractPlugin/Option/OptionHandler/SceneryBuilder/Constructed with original MovementPulse, Pulse/PulseManager/PulseRunner, SmartPathfinder, WalkingQueue and Entity.tick. Actor and scenery tasks share one runner; callbacks submit work in both directions. Native scenery uses the existing real region/landscape/collision implementation and pinned definitions with synthetic land bytes; JVM landscape/flag writes and profile/zone/content APIs are explicit adapters. Compares global ordering, queue/check lifetime, delayed/error handling, stored object state and update records. Full live profile/registries/door fanout and capacity remain separate.'
def op(action,**kw):return dict(op=action,**kw)
def requests():
 out=[]
 def add(actions,**kw):out.append(dict(actions=actions,scenery={},**kw))
 for config in [dict(walk=True),dict(walk=False,delayed=True),dict(walk=False,delayed=False)]:
  for delay in [0,1,2,500]:
   for order in ['before','inside','after']:
    steps=[op('handle')]
    if order=='before':steps.insert(0,op('sceneReplace',ticks=delay))
    elif order=='after':steps.append(op('sceneReplace',ticks=delay))
    steps +=[op('tick')for _ in range(12)]
    if delay==500:steps +=[op('ticks',count=487),op('tick'),op('ticks',count=12)]
    add(steps,handlerConfig=config,sceneOnHandle=order=='inside',sceneDelay=delay)
 for mode in ['sceneClient','sceneExpire','sceneRespawn']:
  for delay in [0,1,3]:
   for ground in [False,True]:
    add([op(mode,ticks=delay),op('handle')]+[op('tick')for _ in range(10)],groundHandles=ground,handlerConfig=dict(walk=False,delayed=True))
 for mutation in ['sceneStop','sceneClose','sceneInactive']:
  add([op('sceneReplace',ticks=3),op('handle'),op('tick'),op(mutation)]+[op('tick')for _ in range(5)])
 for active in [False,True]:
  add([op('sceneExpire',ticks=2),op('sceneCheckActive',active=active),op('handle')]+[op('tick')for _ in range(6)],groundHandles=True,handlerConfig=dict(walk=False,delayed=True))
 for failure in ['groundFails','failChecks']:
  add([op('sceneExpire',ticks=1),op('handle')]+[op('tick')for _ in range(5)],**{failure:True},groundHandles=True,handlerConfig=dict(walk=False,delayed=True))
 return out
def install(adapter,read,c,server,run,java,kt):
 base.install(adapter,read,c,server,run,java,kt);read(pathlib.Path(__file__))
 read(server/'core/game/world/map/RegionChunk.java');read(server/'core/game/world/map/build/LandscapeParser.java')
 for name in ['SceneryBuilder','Constructed']:
  p=server/f'core/game/node/scenery/{name}.java';read(p);java.append(p)
 p=run/'oracle/Support.java';text=p.read_text();needle='new RuntimeSupport.Handler(v(a,"handler",1))';assert text.count(needle)==1
 adapter('oracle/Support.java',text.replace(needle,'new WorldSupport.Handler(v(a,"handler",1))'))
 p=run/'core/game/node/scenery/Scenery.java';text=p.read_text();source=read(server/'core/game/node/scenery/Scenery.java')
 more='''public int id=12348;public core.game.system.task.Pulse restore;public Scenery(){}public Scenery(int i,core.game.world.map.Location l,int t,int r){id=i;location=l;type=t;rotation=r;name="world";}public int getId(){return id;}public boolean isActive(){return oracle.WorldSupport.active(this);}public Scenery getWrapper(){return this;}public boolean isPermanent(){return true;}public core.game.system.task.Pulse getRestorePulse(){return restore;}public void setRestorePulse(core.game.system.task.Pulse p){restore=p;}'''
 more+=c.method(source,'public Constructed asConstructed()')+c.method(source,'public boolean equals(Object obj)')
 adapter('core/game/node/scenery/Scenery.java',text[:-1]+more+'}')
 p=run/'core/game/world/map/RegionManager.java';text=p.read_text()
 text=text.replace('public static Chunk getRegionChunk(Location l){return new Chunk();}','public static Chunk getRegionChunk(Location l){return new Chunk();}')
 text=text.replace('public static class Chunk{','public static class Chunk{public void flag(core.game.world.update.flag.chunk.ObjectUpdateFlag f){oracle.WorldSupport.flags.add(f);}')
 adapter('core/game/world/map/RegionManager.java',text)
 adapter('core/game/world/map/build/LandscapeParser.java','package core.game.world.map.build;public class LandscapeParser{public static core.game.node.scenery.Scenery removeScenery(core.game.node.scenery.Scenery s){return oracle.WorldSupport.remove(s);}public static void addScenery(core.game.node.scenery.Scenery s){oracle.WorldSupport.add(s);}}')
 adapter('core/game/world/update/flag/chunk/ObjectUpdateFlag.java','package core.game.world.update.flag.chunk;public class ObjectUpdateFlag{public core.game.node.scenery.Scenery object;public boolean remove;public ObjectUpdateFlag(core.game.node.scenery.Scenery s,boolean r){object=s;remove=r;}}')
 p=run/'core/game/node/item/GroundItem.java';text=p.read_text().replace('public class GroundItem extends core.game.node.Node{','public class GroundItem extends core.game.node.Node{public int id;public GroundItem(int i){id=i;}');adapter('core/game/node/item/GroundItem.java',text)
 adapter('core/game/node/item/GroundItemManager.java','package core.game.node.item;public class GroundItemManager{public static void create(GroundItem i){oracle.WorldSupport.ground(i);}}')
 read(HERE/'WorldSupport.java');java.append(HERE/'WorldSupport.java')
def generate(check):
 base.walk.RUN=ROOT/'runs/world-pulses-oracle';base.walk.generate(check,sys.modules[__name__])
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();base.walk.c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
