"""Join original InteractPlugin defaults to SceneryDefinition handler lookup."""
import argparse,importlib.util,itertools,os,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('legacy',ROOT/'oracle/legacy-interaction/generate.py')
legacy=importlib.util.module_from_spec(spec);spec.loader.exec_module(legacy)
FOLDER='scenery-options'
SCOPE=('Complete unchanged InteractPlugin, Option and OptionHandler; original SceneryDefinition '
       'forId/getOptionHandler/setOptionHandler/getOptions and Definition.getConfiguration; original '
       'DoorManagingPlugin.newInstance registration. Cached definitions versus live IDs, retained '
       'arrays/options/handlers, ROOT locale, partial failures, and inherited walk/delay defaults. '
       'Scenery construction/metadata and other content methods are adapters. Native ownership is '
       'tested separately; these observations do not establish full content/startup or live doors.')
def op(operation,**kw):return dict(op=operation,**kw)
def requests():
    rows=[]
    for names,local,glob in itertools.product([None,[],[None]*5,['Open',None,'Close','null',''],['OPEN','open','Shut','go-through','\u0130\u03a3\U0001f600']], [None,0,1,'wrong'],[None,0,1]):
        rows.append(dict(names=names,local=local,globalHandler=glob,actions=[op('default'),op('capture'),op('register',name='open',value=1),op('default'),op('initialized',value=False),op('default')]))
    for change in [op('id',value=10327),op('id',value=-1),op('cached',value=1),op('cached',value=None),op('names',value=None),op('names',value=['Open','Close']),op('config',value='wrong'),op('config',value=1),op('register',name='open',value=None)]:
        rows.append(dict(actions=[op('default'),op('capture'),change,op('default'),op('initialized',value=False),op('default')]))
    for name in [None,'','Open','open','close','shut','go-through','i\u0307\u03c2\U0001f600']:
        rows.append(dict(names=['Open','Close','Shut','go-through','\u0130\u03a3\U0001f600'],actions=[op('default'),op('capture'),op('register',name=name,value=1),op('initialized',value=False),op('default')]))
    return rows
def transform(name,text):
    c=legacy.c
    if name=='core/cache/def/impl/SceneryDefinition.java':
        source=legacy.read(legacy.SERVER/name)
        return '''package core.cache.def.impl;import java.util.*;import core.game.interaction.OptionHandler;
public class SceneryDefinition extends oracle.Support.Definition{
public int id;public static final Map<Integer,SceneryDefinition>DEFINITIONS=new HashMap<>();
public static final Map<String,OptionHandler>OPTION_HANDLERS=new HashMap<>();
public final Map<String,Object>handlers=new HashMap<>();
'''+''.join(c.method(source,s)for s in ['public static SceneryDefinition forId(int objectId)','public static OptionHandler getOptionHandler(int nodeId, String name)','public static boolean setOptionHandler(String name, OptionHandler handler)','public String[] getOptions()'])+c.method(legacy.read(legacy.SERVER/'core/cache/def/Definition.java'),'public <V> V getConfiguration(String key)')+'}'
    if name=='core/game/node/scenery/Scenery.java':
        return '''package core.game.node.scenery;public class Scenery extends core.game.node.Node{
public core.cache.def.impl.SceneryDefinition definition;public core.cache.def.impl.SceneryDefinition getDefinition(){return definition;}}'''
    return text
def register(adapter,read,c):
    read(pathlib.Path(__file__))
    source=read(legacy.SERVER/'content/global/handlers/scenery/DoorManagingPlugin.java')
    adapter('content/global/handlers/scenery/DoorManagingPlugin.java','''package content.global.handlers.scenery;
import core.cache.def.impl.SceneryDefinition;import core.game.interaction.OptionHandler;import core.plugin.Plugin;
public class DoorManagingPlugin extends OptionHandler{
public boolean handle(core.game.node.entity.player.Player p,core.game.node.Node n,String s){throw new UnsupportedOperationException();}
'''+c.method(source,'public Plugin<Object> newInstance(Object arg) throws Throwable')+'}')
def generate(check):
    legacy.RUN=ROOT/'runs/scenery-options-oracle';legacy.requests=requests;legacy.generate(check,sys.modules[__name__])
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args()
    legacy.c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
