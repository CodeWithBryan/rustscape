"""Join original ground-item lifetime/geometry to original walking and path dispatch."""
import argparse,importlib.util,itertools,os,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;BASE=ROOT/'oracle/player-walk';sys.path.insert(0,str(BASE))
spec=importlib.util.spec_from_file_location('walk',BASE/'generate.py');walk=importlib.util.module_from_spec(spec);spec.loader.exec_module(walk)
FIXTURE='ground-target';MAIN='oracle.Support'
SCOPE='Complete unchanged GroundItem joined with unchanged MovementPulse/Pulse/PulseManager/PulseRunner/SmartPathfinder/WalkingQueue and original Pathfinder ground-node dispatch/truncation/reach. Original GroundSpawn isActive/isPrivate/isAutoSpawn override bodies in a constructor-only subtype adapter. Native replay uses actual native ground/spawn construction, canonical retained targets, queues and scheduler. Definition/copy/default-content construction, player/region/clipping/teleport/handler destinations and approach arrivals are explicit services. Rust-only ownership tests cover exposed views and disconnected cyclic graphs; JVM GC timing is not compared. No full ground action/Take/inventory/content/world/visual or performance claim.'
def op(operation,**kw):return dict(op=operation,**kw)
def requests():
 rows=[]
 def add(actions,**kw):rows.append(dict(ground=kw.pop('ground',{}),actions=actions,**kw))
 for spawn,permitted,bfs,running,distance in itertools.product([False,True],[False,True],[False,True],[False,True],[0,1,7,25,26,40]):
  add([op('approach')]+[op('tick')for _ in range(20)],ground=dict(spawn=spawn,location=[3207+distance,3216,0]),permitted=permitted,bfs=bfs,energy=100 if running else 0)
 for spawn,removed,decay,size in itertools.product([False,True],[False,True],[0,1,3,200],[-1,1,2]):
  add([op('approach')]+[op('tick')for _ in range(10)],ground=dict(spawn=spawn,removed=removed,decay=decay,size=size))
 changes=[op('groundSet',location=[3213,3218,0]),op('groundSet',location=None),op('groundSet',size=3),op('groundSet',removed=True,decay=0),op('groundReplace',location=[3215,3217,0],spawn=True),op('optionHandler',handler=2),op('optionReplace',name='Use',index=3,handler=2)]
 for change,phase in itertools.product(changes,[None,'destination','permitted','interactionArrival']):
  if phase is None:add([op('approach'),op('tick'),change]+[op('tick')for _ in range(10)])
  else:add([op('approach')]+[op('tick')for _ in range(12)],hooks=[dict(event=phase,actions=[change])],handler=None if phase=='permitted'else 1)
 return rows
def install(adapter,read,c,server,run,java,kt):
 read(pathlib.Path(__file__));source=read(BASE/'Support.java')
 source=source.replace('final Scenery target=scenery;','final Node target=GroundSupport.ground;')
 source=source.replace('default:throw new IllegalArgumentException();','default:GroundSupport.act(a);break;')
 source=source.replace('public static boolean permitted(Location l){emit("permitted",coord(l));return true;}','public static boolean permitted(Location l){emit("permitted",coord(l));return !Boolean.FALSE.equals(input.get("permitted"));}')
 source=source.replace('player.settings.energy=input.containsKey','GroundSupport.reset(input);player.settings.energy=input.containsKey')
 source=source.replace(' return o;}',' GroundSupport.snapshot(o);return o;}')
 source=source.replace('public static void main(String[]args)throws Exception{','public static void main(String[]args)throws Exception{GroundSupport.captureErrors();')
 java[:]=[p for p in java if p!=BASE/'Support.java']
 adapter('oracle/Support.java',source)
 adapter('core/game/node/item/Item.java','''package core.game.node.item;public class Item extends core.game.node.Node{protected int index=-1;protected Interact interactPlugin=new Interact();int id,amount,charge;
public Item(int i){this(i,1,1000);}public Item(int i,int a,int c){id=i;amount=a;charge=c;name="item";}public int getId(){return id;}public int getAmount(){return amount;}public int getCharge(){return charge;}
public static class Interact{public void setDefault(){}}}''')
 adapter('core/game/node/item/GroundItem.java',read(server/'core/game/node/item/GroundItem.java'))
 original=read(server/'core/game/system/config/GroundSpawnLoader.kt')
 adapter('oracle/GroundSpawnAdapter.kt','package oracle\nimport core.game.node.item.*\nimport core.game.world.map.Location\nclass GroundSpawnAdapter(item:Item,location:Location?):GroundItem(item,location){'+''.join(c.method(original,'override fun '+name+'()')for name in ['isActive','isPrivate','isAutoSpawn'])+'}')
 p=run/'core/game/node/entity/player/Player.java';text=p.read_text();adapter('core/game/node/entity/player/Player.java',text.rstrip()[:-1]+'''public Details getDetails(){return new Details();}public static class Details{public int getUid(){return 7;}}public String getUsername(){return name;}}''')
 adapter('oracle/GroundSupport.java',read(HERE/'GroundSupport.java'))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();walk.RUN=ROOT/'runs/ground-target-oracle'
 walk.c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);walk.generate(a.check,sys.modules[__name__])
