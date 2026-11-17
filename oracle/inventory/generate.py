"""Compile unchanged Item/Container/Event classes with explicit world boundaries."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/inventory-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read,run=compiler.read,compiler.run
def requests():
    rng=random.Random(530);cases=[]
    base={str(i):dict(id=i,name='item'+str(i),value=i*11,stackable=i in(0,2,4),unnoted=i!=4,noteId=3 if i==4 else -1)for i in range(8)}
    base['3']['handlers']={'high_alchemy':17,'low_alchemy':7}
    def op(name,**args):return dict(op=name,**args)
    def item(id,amount=1,charge=1000):return dict(id=id,amount=amount,charge=charge)
    def add(actions,capacity=4,kind='DEFAULT',sort='ID',items=None,containers=None,definitions=None,**kwargs):
        cases.append(dict(kind='sequence',definitions=definitions or base,items=items or dict(a=item(1,3),b=item(2,3),c=item(2,2,500),d=item(4,2),empty=dict(empty=True),nil=None),containers=containers if containers is not None else [dict(id=0,capacity=capacity,type=kind,sort=sort,listeners=[0,1])],actions=actions,**kwargs))
    values=[-2**31,-1,0,1,65535,65536,2**31-1]
    for id in values:
        for charge in values:
            add([op('itemQuery',item='a'),op('itemCopy',item='a',as_='copy'),op('itemCharge',item='a',value=charge),op('itemQuery',item='a'),op('itemCopy',item='a',as_='changed'),op('itemId',item='a',value=charge),op('itemQuery',item='a'),op('itemUnnote',item='a',as_='plain'),op('itemDrop',item='a',as_='drop')],items={'a':item(id,rng.choice(values),charge)},containers=[],drop={str(id&65535):rng.choice(values)})
    for amount in values:
        add([op('itemAmount',item='a',value=amount),op('itemIndex',item='a',value=7),op('itemCopy',item='a',as_='copy'),op('itemDefinition',item='a',value=3),op('itemQuery',item='a'),op('itemDefinition',item='a',value=None),op('itemQuery',item='a')],containers=[])
    add([op('itemQuery',item='empty'),op('itemCopy',item='empty',as_='copy'),op('itemUnnote',item='empty',as_='u'),op('itemPlugin',item='empty'),op('itemOperate',item='empty'),op('itemQuery',item='copy')],containers=[])
    for value in values:
        for config in [None,0,1,2**31-1,'1',True,1.5]:
            defs=json.loads(json.dumps(base));defs['3']['value']=value;defs['3']['handlers']={'high_alchemy':config,'low_alchemy':config}
            add([op('itemQuery',item='a'),op('itemQuery',item='d'),op('itemAlchemy',item='d',high=False),op('itemUnnote',item='d',as_='plain')],items={'a':item(3,-2),'d':item(4,3,500)},definitions=defs,containers=[])
    for kind in ['DEFAULT','SHOP','ALWAYS_STACK','NEVER_STACK']:
        for sort in ['ID','HASH']:
            for capacity in [0,1,2,4,8]:
                add([op('add',item='b',fire=False),op('query',item='c'),op('add',item='c',fire=False),op('captureEvent'),op('captureArray'),op('get',slot=0,as_='held'),op('itemAmount',item='held',value=5),op('update'),op('remove',item='c',fire=False),op('query',item='b'),op('insert',from_=0,to=capacity-1,fire=False),op('refresh'),op('clear',fire=False),op('update',force=False),op('update'),op('addList',items=['a',None,'b','c']),op('shift'),op('format'),op('removeAll',ids=[1,2])],capacity=capacity,kind=kind,sort=sort)
            for amount in [-2**31,-3,0,1,2,2**31-1]:
                add([op('add',item='a',fire=False,preferred=2),op('add',item='b',fire=False),op('add',item='c'),op('replace',item='a',slot=1,fire=False,as_='old'),op('remove',item='a',slot=1,fire=False),op('query',item='a'),op('clear',fire=True)],kind=kind,sort=sort,items={'a':item(1,amount),'b':item(2,amount),'c':item(2,amount,1)})
    for preferred in [-2,-1,0,3,4,2**31-1]:
        add([op('add',item='b',preferred=preferred),op('add',item='a',preferred=preferred),op('addGround',item='c'),op('addAbsent',item='a')])
    for shape in [('DEFAULT','ID',2),('ALWAYS_STACK','HASH',5),('SHOP','HASH',1)]:
        for target_size in [0,1,4,8]:
            containers=[dict(id=0,capacity=shape[2],type=shape[0],sort=shape[1],listeners=[0]),dict(id=1,capacity=target_size,type='DEFAULT',sort='ID',listeners=[1])]
            add([op('addMany',items=['a','b','c']),op('add',item='d',container=1,fire=False),op('spaceContainer',source=1),op('copy',source=1),op('add',item='a',fire=False),op('copy',source=0),op('add',item='b'),op('addAll',source=0),op('spaceMany',items=['a','b','c']),op('addAll',source=1),op('format')],containers=containers)
    for value in [[],[None],[False],[dict(slot='0',id='1',amount='-3',charge='1000')],[dict(slot='-1',id='1',amount='1',charge='1')],[dict(slot='0',id='5614',amount='1',charge='1')],[dict(slot='4',id='1',amount='1',charge='1')]]:
        add([op('add',item='b',fire=False),op('parse',value=value),op('update'),op('format')])
    rows=[dict(slot=str(i),id=str(i),amount=str(i+1),charge='1000')for i in range(4)]
    for field,bad in [('slot','bad'),('id','bad'),('amount','bad'),('charge',None),('amount',True),('slot','-1')]:
        value=json.loads(json.dumps(rows));value[2][field]=bad;add([op('parse',value=value),op('query',item='b'),op('update')])
    callbacks=[op('replace',item='c',slot=0,fire=False),op('clear',fire=False),op('clear',fire=True),op('refresh'),op('update',force=True),op('register',value=2),op('removeListener',value=1),op('fail'),op('itemAmount',item='b',value=99),op('mutateSlot',slot=0,field='amount',value=17),op('eventClear',value=True)]
    for when in ['update','refresh']:
        for callback in callbacks:
            add([op('add',item='b',fire=False),op('captureEvent'),op('update')if when=='update'else op('refresh'),op('update'),op('clear')],scripts={'0':[dict(when=when,at=1,action=callback)]})
    add([op('register',value=None),op('add',item='b'),op('update'),op('clear')])
    helper_ops=[op('getNew',slot=-1),op('getNew',slot=0,as_='new'),op('getId',slot=-1),op('getId',slot=4),op('getAsId',slot=-1),op('getItem',item='nil',as_='none'),op('getByItem',item='nil'),op('getAll',item='nil'),op('contains',id=2,amount=0),op('containsItem',item='nil'),op('containsItems',items=[]),op('containsItems',items=['a',None]),op('containsOneItems',items=['a',None]),op('containsOneId',id=2),op('containsOneIds',ids=[1,2]),op('containsAll',ids=[1,2]),op('containItems',ids=[1,2]),op('containsAll',ids=[]),op('containItems',ids=[]),op('removeListener',value=8),op('refresh',listener=None),op('refresh',listener=5)]
    for populated in [False,True]:add(([op('addMany',items=['a','b'])]if populated else[])+helper_ops)
    for kind in ['DEFAULT','SHOP','ALWAYS_STACK','NEVER_STACK']:
        add([op('rawSet',slot=0,item='a'),op('rawSet',slot=1,item='b'),op('rawSet',slot=2,item='c'),op('query',item='a'),op('format'),op('contains',id=2,amount=1),op('containsOneId',id=2),op('containsOneIds',ids=[2]),op('remove',item='d',slot=0,fire=False),op('captureArray'),op('captureEvent'),op('clear',fire=False),op('update',force=True),op('retainedSet',array=0,slot=0,item='d'),op('update')],kind=kind,items={'a':item(2,2**31-1),'b':item(2,2**31-1,2),'c':item(2,1),'d':item(2,1,99)})
    add([op('add',item='a',fire=False),op('get',slot=0,as_='live'),op('captureArray'),op('captureEvent'),op('rawSet',slot=1,item='b'),op('eventSet',slot=3,item='b'),op('itemAmount',item='b',value=17),op('update'),op('remove',item='live',fire=False),op('nullAmount',value=99),op('captureEvent'),op('update',force=True),op('nullAmount',value=0),op('clear',fire=False),op('retainedSet',array=0,slot=0,item='d'),op('refresh')])
    for callback in [op('removeListener',value=0),op('removeListener',value=1),op('register',value=2)]:
        add([op('add',item='b',fire=False),op('update'),op('clear')],scripts={'0':[dict(when='update',at=1,action=callback)]})
    for fields in [{'wrapper':'plugin','option:operate':'handler'},{'wrapper':True,'option:operate':1},{'wrapper':None,'option:operate':None}]:
        defs=json.loads(json.dumps(base));defs['1']['handlers']=fields
        add([op('itemPlugin',item='a'),op('itemHasPlugin',item='a'),op('itemOperate',item='a'),op('itemDefinition',item='a',value=2),op('itemPlugin',item='a'),op('itemOperate',item='a'),op('itemCharge',item='a',value=-1),op('itemOperate',item='a')],containers=[],definitions=defs)
    for change in [op('itemId',item='a',value=3),op('itemAmount',item='a',value=50),op('fail')]:
        add([op('itemDrop',item='a',as_='drop')],containers=[],drop={'1':3},dropAction=change)
    for root in [None,False,1,{},[dict(slot='0',id='1',amount='100',charge='1000')]]:
        add([op('parse',value=root),op('remove',item='a'),op('format')])
    defs={str(i):{}for i in range(5615)}
    add([op('parse',value=[dict(slot='0',id='5614',amount='1',charge='1000'),dict(slot='1',id='5613',amount='-3',charge='65536')]),op('update')],definitions=defs)
    collisions={str(i):item(i*64,1)for i in range(40)}
    add([op('rawSet',slot=i,item=str(i))for i in range(40)]+[op('format')],capacity=40,items=collisions)
    defs=json.loads(json.dumps(base));defs['1']['handlers']={'high_alchemy':True}
    add([op('add',item='b',fire=False),op('parse',value=rows),op('captureEvent'),op('update')],definitions=defs)
    sparse={'0':dict(id=0),'3':dict(id=3),'99999':dict(id=99)}
    add([op('parse',value=[dict(slot='0',id='2',amount='3',charge='1000'),dict(slot='1',id='3',amount='1',charge='1000')]),op('format')],definitions=sparse)
    for getter in ['getNew','getId','getAsId']:
        add([op('addMany',items=['a','b']),*[op(getter,slot=slot,as_='held')for slot in [-2,-1,0,3,4]]])
    for listener in [None,0,1,10]:
        add([op('register',value=None),op('register',value=0),op('unregister',value=listener),op('unregister',value=listener),op('add',item='b'),op('unregister',value=None),op('update')])
    for listener in [0,1,None]:
        add([op('add',item='b',fire=False),op('update'),op('update')],scripts={'0':[dict(when='update',at=1,action=op('unregister',value=listener))]})
    for _ in range(100):
        kind=rng.choice(['DEFAULT','SHOP','ALWAYS_STACK','NEVER_STACK']);sort=rng.choice(['ID','HASH']);capacity=rng.randrange(0,8)
        actions=[]
        for n in range(25):
            name=rng.choice(['add','remove','replace','insert','clear','shift','update','refresh','get','query','addMany','removeMany','captureEvent','captureArray','format','spaceMany','addAbsent'])
            actions.append(op(name,item=rng.choice(['a','b','c','d','empty','nil']),slot=rng.randrange(-1,capacity+2),from_=rng.randrange(-1,capacity+1),to=rng.randrange(-1,capacity+1),fire=bool(rng.randrange(2)),preferred=rng.choice([-1,0,capacity]),items=rng.choices(['a','b','c','d',None],k=3),as_='returned'))
        add(actions,capacity=capacity,kind=kind,sort=sort)
    for keys in [[i*64 for i in range(40)],list(reversed([i*64 for i in range(40)])),[-2**31+i*64 for i in range(20)]+[i*64 for i in range(20)],[-2**31,2**31-1,-1,0,1],list(range(100)),[rng.randrange(-2**31,2**31)for _ in range(500)]]:
        cases.append(dict(kind='integerMap',keys=keys))
    return cases

def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['server','client']:
        path=ROOT/'reference'/part
        if run(['git','-C',path,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',path,'diff','--quiet','HEAD','--'])
    RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as response:data=response.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    classpath=os.pathsep.join(map(str,[classes,jar]));java=[]
    def adapter(name,text):
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');java.append(path)
    definition=read(SERVER/'core/cache/def/impl/ItemDefinition.java')
    # Retain fields/defaults and every definition method exercised by actual Item.
    body=definition[definition.index('private static final Map<Integer, ItemDefinition> DEFINITIONS'):definition.index('public ItemDefinition()')]
    methods=['public ItemDefinition()','public static ItemDefinition forId(','public static Map<Integer, ItemDefinition> getDefinitions()','public boolean isStackable()','public int getValue()','public int getAlchemyValue(','public int getNoteId()','public boolean isUnnoted()','public ItemPlugin getItemPlugin()','public static OptionHandler getOptionHandler(']
    body+='\n'.join(compiler.method(definition,m)for m in methods)
    adapter('core/cache/def/impl/ItemDefinition.java','package core.cache.def.impl;import java.util.*;import core.cache.def.Definition;import core.game.node.item.*;import core.game.interaction.OptionHandler;import core.game.system.config.ItemConfigParser;import core.tools.Log;import static core.api.ContentAPIKt.log;public class ItemDefinition extends Definition<Item>{'+body+'}')
    node=read(SERVER/'core/game/node/Node.java')
    node_methods=['public Node(','public String getName()','public int getIndex()','public void setIndex(','public DestinationFlag getDestinationFlag()']
    adapter('core/game/node/Node.java','package core.game.node;import core.game.interaction.*;import core.game.world.map.Location;public class Node {protected String name;protected int index;protected Location location;protected InteractPlugin interactPlugin;protected DestinationFlag destinationFlag;'+''.join(compiler.method(node,m)for m in node_methods)+'}')
    config=read(SERVER/'core/game/system/config/ItemConfigParser.kt')
    constants=''.join('public static String '+name+'='+re.search(r'const val '+name+r' = ("[^"]+")',config).group(1)+';'for name in ['HIGH_ALCHEMY','LOW_ALCHEMY'])
    adapter('core/game/system/config/ItemConfigParser.java','package core.game.system.config;public class ItemConfigParser {'+constants+'}')
    adapters={
      'core/game/world/map/Location.java':'package core.game.world.map;public class Location {}',
      'core/game/interaction/DestinationFlag.java':'package core.game.interaction;public class DestinationFlag {public static final DestinationFlag ITEM=new DestinationFlag();}',
      'core/game/interaction/InteractPlugin.java':'package core.game.interaction;public class InteractPlugin {public InteractPlugin(Object owner){}}',
      'core/game/interaction/OptionHandler.java':'package core.game.interaction;public class OptionHandler {public String name;public OptionHandler(String name){this.name=name;}}',
      'core/game/node/item/ItemPlugin.java':'package core.game.node.item;public class ItemPlugin {public String name;public ItemPlugin(String name){this.name=name;}}',
      'core/game/node/entity/player/Player.java':'package core.game.node.entity.player;public class Player {}',
      'core/game/node/entity/combat/equipment/DegradableEquipment.java':'package core.game.node.entity.combat.equipment;public class DegradableEquipment {public static int getDropReplacement(int id){return oracle.InventoryOracle.replacement(id);}}',
      'core/game/node/item/GroundItemManager.java':'package core.game.node.item;public class GroundItemManager {public static void create(Item item,core.game.node.entity.player.Player player){oracle.InventoryOracle.ground(item);}}',
      'core/tools/Log.java':'package core.tools;public class Log {public static int ERR=1;}',
      'core/api/ContentAPIKt.java':'package core.api;public class ContentAPIKt {public static void log(Object o,int level,String text){throw new AssertionError(text);}}',
      'org/rs09/consts/Items.java':'package org.rs09.consts;public class Items {public static int MAGIC_CARPET_5614=5614;}',
      'com/google/errorprone/annotations/CheckReturnValue.java':'package com.google.errorprone.annotations;public @interface CheckReturnValue {}',
      'core/tools/StringUtils.java':'package core.tools;public class StringUtils {public static boolean isPlusN(String name){throw new UnsupportedOperationException("Examine outside inventory oracle");}}',
    }
    for name,text in adapters.items():adapter(name,text)
    for name in ['core/cache/def/Definition.java','core/game/node/item/Item.java','core/game/container/Container.java','core/game/container/ContainerEvent.java','core/game/container/ContainerListener.java','core/game/container/ContainerType.java','core/game/container/SortType.java']:
        path=SERVER/name;read(path);java.append(path)
    read(HERE/'InventoryOracle.java');java.append(HERE/'InventoryOracle.java');run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,*java])
    command=['java','-Xmx2g','-Dfile.encoding=UTF-8','-cp',classpath,'oracle.InventoryOracle']
    if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java'))
    inputs=requests();input_file=RUN/'input.jsonl';input_file.write_bytes(''.join(json.dumps(case,sort_keys=True,ensure_ascii=True)+'\n'for case in inputs).encode())
    with input_file.open('rb')as stdin:result=subprocess.run(command,cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    output=[json.loads(line)for line in result.stdout.splitlines()if line]
    if len(output)!=len(inputs):raise RuntimeError('Missing inventory cases')
    data=''.join(json.dumps(case,sort_keys=True,ensure_ascii=True,separators=(',',':'))+'\n'for case in output).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged Item, Container, ContainerEvent, ContainerListener and enum classes; complete Definition and verbatim selected ItemDefinition/Node methods. Explicit controlled degradation/ground-item and opaque plugin/interaction/Node-world adapters. Isolated definitions/listeners and mutable NULL_ITEM reset per scenario. State, aliases, event/array retention, partial failures, synchronous callbacks, integer-map output and JSON container loading are compared. No complete Node, item content configuration/plugin/degradation, actual ground spawning, live inventory/bank/equipment profile or world integration claim.')
    folder=ROOT/'fixtures/inventory'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Inventory oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original inventory scenarios reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(output)} original inventory scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args();compiler.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(args.check)
