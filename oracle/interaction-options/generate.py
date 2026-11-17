"""Original complete InteractPlugin/Option classes, retained option table/default tests."""
import argparse, importlib.util, itertools, os, pathlib, sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('legacy',ROOT/'oracle/legacy-interaction/generate.py')
legacy=importlib.util.module_from_spec(spec);spec.loader.exec_module(legacy)
FOLDER='interaction-options'
SCOPE='Complete unchanged pinned InteractPlugin, Option and OptionHandler classes; constructor, init, setDefault, set/remove/get/options/initialized/sendOption and Option.defaultHandler. Original immutable names/indices, canonical static Option identity, exposed arrays, live loop bounds, reentrant callbacks and partial failures. Runtime node classes, definition/handler registry, default ROOT locale and packet delivery are explicit recording services. Other approach/arrival methods are not executed by this corpus. Native owning-world/default-registry and live action bindings remain separate.'

def op(operation,**kw):return dict(op=operation,**kw)
def requests():
 rows=[]
 def add(actions=None,**kw):rows.append(dict(actions=actions or [op('default'),op('default')],**kw))
 for kind,located in itertools.product(['Player','Npc','Scenery','Item','Other','Null'],[False,True]):
  for names in [None,[],[None,'null','Take','', 'NULL','İΣ😀']]:
   add(kind=kind,located=located,names=names)
 for kind in ['Player','Npc','Scenery','Item','Other','Null']:
  add([op('init',names=['Open',None,'null','Take']),op('get',index=3),op('default'),op('setInitialized',value=False),op('default')],kind=kind)
  for index in [-2147483648,-1,0,2,3,6,7,8,2147483647]:
   for name in [None,'null','', 'Take']:
    add([op('new',name=name,index=index),op('set',option=8),op('get',index=index),op('remove',option=8),op('removeIndex',index=index)],kind=kind)
  for name in [None,'Take','İΣ😀','NULL','']:
   add([op('handler',name=name,id=995)],kind=kind)
 # Exposed old arrays and names remain independent of a subsequent init.
 add([op('capture'),op('init',names=['A','B']),op('capture'),op('arraySet',array=0,index=3,option=1),op('arraySet',array=1,index=0,option=2),op('init',names=None),op('get',index=0)])
 add([op('default'),op('set',table=1,option=1),op('handlerSet',option=1,handler=2),op('default',table=1)],kind='Player')
 add([op('new',name='Follow',index=2),op('set',option=1),op('remove',option=8),op('remove',option=1)],kind='Player')
 add([op('set',option=None),op('remove',option=None),op('handler',name=None)])
 # Callback failures and reentrant replacement exercise live table length and
 # retained original names. One-shot hooks keep recursive calls bounded.
 mutations=[op('fail'),op('init',names=[]),op('init',names=['R']),op('init',names=['R']*9),
  op('setInitialized',value=True),op('setInitialized',value=False),op('namesSet',index=1,value='Changed'),
  op('setNode',id=99,located=False,definition=None),op('default',table=1)]
 for kind,event,mutation,at in itertools.product(['Player','Npc','Scenery','Item'],['id','location','definition','names','lookup','send'],mutations,[1,2]):
  add([op('capture'),op('default'),op('init',names=['A','B']),op('default')],kind=kind,
   names=['A','B'],hooks=[dict(event=event,at=at,actions=[mutation])])
 for kind in ['Npc','Scenery','Item']:
  add(kind=kind,definition=None)
  add([op('init',names=['A','B']),op('capture'),op('default')],kind=kind,
   hooks=[dict(event='lookup',actions=[op('arraySet',array=0,index=0,option=2)])])
 return rows

def transform(name,text):
 if name=='core/game/node/Node.java':
  text=text.replace('public int key,size=1;','public int key,id,size=1;')
  text=text.replace('public int getId(){return key;}','public int getId(){oracle.Support.emit("id",key);return id;}public String toString(){return "node:"+key;}')
 elif name.startswith('core/cache/def/impl/'):
  cls=name.rsplit('/',1)[-1][:-5];kind={'ItemDefinition':'Item','NPCDefinition':'Npc','SceneryDefinition':'Scenery'}[cls]
  text='''package core.cache.def.impl;public class %s extends oracle.Support.Definition{
  public static core.game.interaction.OptionHandler getOptionHandler(int id,String name){return oracle.Support.lookup("%s",id,name);}
  public static %s forId(int id){return new %s();}}'''%(cls,kind,cls,cls)
 elif name in ['core/game/node/entity/npc/NPC.java','core/game/node/scenery/Scenery.java','core/game/node/item/Item.java']:
  start=text.index('public core.cache.def.impl.')
  cls=text[start:].split()[1]
  text=text[:start]+'''public %s getDefinition(){oracle.Support.emit("definition",key);return(%s)oracle.Support.definitions[oracle.Support.definitionIds[key]];}}'''%(cls,cls)
 elif name=='core/net/packet/context/InteractionOptionContext.java':
  text='''package core.net.packet.context;public class InteractionOptionContext{public int player,index;public String name;public InteractionOptionContext(core.game.node.entity.player.Player p,int i,String s){player=p.key;index=i;name=s;}}'''
 elif name=='core/net/packet/PacketRepository.java':
  text='''package core.net.packet;public class PacketRepository{public static void send(Class<?>c,Object context){var v=(core.net.packet.context.InteractionOptionContext)context;oracle.Support.emit("send",v.player,v.index,v.name);}}'''
 return text
def register(adapter,read,c):read(pathlib.Path(__file__))
def generate(check):
 legacy.RUN=ROOT/'runs/interaction-options-oracle';legacy.requests=requests
 legacy.generate(check,sys.modules[__name__])
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args()
 legacy.c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
