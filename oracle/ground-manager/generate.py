"""Joined unchanged GroundItemManager, chunk add/remove and private packet sends."""
import argparse, importlib.util, os, pathlib, re
ROOT=pathlib.Path(__file__).resolve().parents[2]
HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('chunk',ROOT/'oracle/chunk-sync/generate.py')
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
SCOPE='Complete unchanged GroundItemManager and GroundItem; verbatim RegionChunk add/remove/sync/update and complete private item send bodies, original BuildItemContext, Location distance statements, ItemDefinition tradeability/hasAction bodies. Joined native canonical lists, item/flag state and packet bytes. Definition fields/typed configuration/plugin, actor location/playing/AI repository, GroundSpawn/content virtual dispatch, region lookup, disabled ISAAC and session are explicit adapters. Not complete GroundSpawn scheduling, inventory pickup, profile, live fanout or gameplay capacity.'

def transform(name,text,source,c):
    if name=='core/game/world/map/RegionChunk.java':
        _,original=source(name)
        text=text[:-1]+''.join(c.method(original,signature)for signature in ['public void add(GroundItem item)','public void remove(GroundItem item)'])+'}'
        text=text.replace('import java.util.*;','import java.util.*;import core.net.packet.context.BuildItemContext;import core.game.world.update.flag.chunk.ItemUpdateFlag;')
    elif name in ['core/net/packet/out/ConstructGroundItem.java','core/net/packet/out/ClearGroundItem.java','core/net/packet/out/UpdateGroundItemAmount.java']:
        _,original=source(name)
        text=text.replace('import core.net.packet.IoBuffer;','import core.net.packet.IoBuffer;import core.net.packet.context.BuildItemContext;import core.game.node.entity.player.Player;')
        text=text[:-1]+c.method(original,'public void send(BuildItemContext context)')+'}'
    elif name=='core/net/packet/out/UpdateAreaPosition.java':
        _,original=source(name)
        text=text[:-1]+c.method(original,'public static IoBuffer getBuffer(Player player, Location base)')+'}'
    elif name=='core/net/packet/IoBuffer.java':
        text=text[:-1]+'public IoBuffer cypherOpcode(Object disabled){return this;}}'
    elif name=='core/game/world/map/Location.kt':
        _,original=source(name)
        chunkbase=next(line.strip() for line in original.splitlines() if 'val chunkBase:' in line)
        extra='val regionX:Int get()=x shr 3;val regionY:Int get()=y shr 3;'+chunkbase+'\n'
        extra+=c.method(original,'@JvmOverloads fun withinDistance(')+'\n'
        extra+='companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z);@JvmStatic '+c.method(original,'fun getDistance(first: Location, second: Location)')+'}\n'
        text=text.replace('package core.game.world.map\n','package core.game.world.map\nimport kotlin.math.sqrt\n')
        text=text.replace('override fun equals',extra+'override fun equals')
    elif name=='core/game/node/item/Item.java':
        _,original=source('core/game/node/item/Item.java')
        text=text.replace('protected Location location;','public core.cache.def.impl.ItemDefinition definition;protected Location location;')
        text=text.replace('interactPlugin=new Interact(this);','interactPlugin=new Interact(this);definition=oracle.ManagerSupport.definition(getId());')
        text=text[:-2]+'public core.cache.def.impl.ItemDefinition getDefinition(){return definition;}'+c.method(original,'public boolean hasItemPlugin()')+c.method(original,'public ItemPlugin getPlugin()')+'}\n'
    elif name=='core/game/node/entity/player/Player.java':
        text=text.replace('public int id,uid;','public Location location;public boolean playing=true,artificial;public java.util.ArrayList<core.game.node.item.GroundItem>aiItems;public int id,uid;')
        text=text.replace('flags.anchor=new Location(3207,3216,0);','flags.anchor=new Location(3207,3216,0);location=flags.anchor;')
        text=text.replace('public Details getDetails()', 'public Location getLocation(){Location l=location;oracle.Support.emit("location",id);return l;}public boolean isPlaying(){boolean v=playing;oracle.Support.emit("playing",id);return v;}public boolean isArtificial(){boolean v=artificial;oracle.Support.emit("artificial",id);return v;}public Details getDetails()')
        text=text.replace('public int getUid()', 'public Session getSession(){return p.session;}public int getUid()')
        text=text.replace('public void write(IoBuffer b)', 'public Pair getIsaacPair(){return new Pair();}public static class Pair{public Object getOutput(){return null;}}public void write(IoBuffer b)')
    elif name=='core/game/world/map/RegionManager.java':
        text=text.rstrip()[:-1]+'public static RegionChunk getRegionChunk(Location l){if(l==null)throw new NullPointerException();RegionChunk c=oracle.ManagerSupport.chunk();oracle.Support.emit("lookup",java.util.Arrays.asList(l.getX(),l.getY(),l.getZ()));return c;}}'
    elif name=='oracle/Support.java':
        text=text.replace('new GroundItem(', 'new VirtualGround(')
        text=text.replace('static Object act(Map a)', 'static Object act(Map a)')
        text=text.replace('default:throw new IllegalArgumentException(op);','default:return ManagerSupport.act(a);')
        text=text.replace('s.put("itemLists",ls);return s;', 's.put("itemLists",ls);ManagerSupport.snapshot(s);return s;')
        text=text.replace('ChunkUpdateTracker.INSTANCE.resetAll();RegionChunk.dirtyListener', 'ManagerSupport.reset();ChunkUpdateTracker.INSTANCE.resetAll();RegionChunk.dirtyListener')
        text=text.replace('ChunkUpdateTracker.INSTANCE.onFlagged(c);emit("dirty"', 'ChunkUpdateTracker.INSTANCE.onFlagged(c);ManagerSupport.rememberFlags();emit("dirty"')
    return text

def register(adapter,source,read,c):
    for name in ['core/game/node/item/GroundItemManager.java','core/net/packet/context/BuildItemContext.java','core/net/packet/Context.java','core/game/world/map/MapDistance.java']:
        _,text=source(name);adapter(name,text)
    # Explicit dependency bodies; manager/chunk/packet/reference bodies above
    # are copied unchanged. Record both adapter and harness source hashes.
    _,item_definition=source('core/cache/def/impl/ItemDefinition.java')
    _,config=source('core/game/system/config/ItemConfigParser.kt')
    constant=re.search(r'const val TRADEABLE\s*=\s*("[^"]*")',config).group(1)
    adapter('core/game/system/config/ItemConfigParser.java','package core.game.system.config;public class ItemConfigParser{public static final String TRADEABLE='+constant+';}')
    trade=c.method(item_definition,'public boolean isTradeable()').replace('isTradeable()','tradeableBody()',1)
    adapter('core/cache/def/impl/ItemDefinition.java','''package core.cache.def.impl;
import core.game.node.item.ItemPlugin;import core.game.system.config.ItemConfigParser;
public class ItemDefinition{public int id;public Object tradeable=true;public ItemPlugin plugin;public String name="null";public String[]options={null,null,null,null,"drop"};
public ItemDefinition(int id){this.id=id;}
public String getName(){return name;}@SuppressWarnings("unchecked") public<V>V getConfiguration(String k,V fail){return(V)(tradeable==null?fail:tradeable);}
public boolean isTradeable(){boolean v=tradeableBody();oracle.Support.emit("trade",id,v);return v;}
public ItemPlugin getItemPlugin(){ItemPlugin v=plugin;oracle.Support.emit("plugin",id,v==null?null:v.id);return v;}'''+trade+c.method(item_definition,'public boolean hasAction(String optionName)')+c.method(item_definition,'public boolean hasDestroyAction()')+'}')
    adapter('core/game/node/item/ItemPlugin.java','''package core.game.node.item;
import core.game.node.entity.player.Player;
public class ItemPlugin{public static final int DROP=1;public int id;public ItemPlugin(int id){this.id=id;}public void remove(Player p,Item i,int option){oracle.Support.emit("pluginRemove",id,p==null?null:p.id,oracle.ManagerSupport.id((GroundItem)i),option);}}''')
    adapter('core/game/bots/AIRepository.java','''package core.game.bots;
import java.util.ArrayList;import core.game.node.entity.player.Player;import core.game.node.item.GroundItem;
public class AIRepository{public static ArrayList<GroundItem>getItems(Player p){ArrayList<GroundItem>v=p.aiItems;oracle.Support.emit("ai",p.id);return v;}}''')
    adapter('core/net/packet/PacketRepository.java','''package core.net.packet;
import core.net.packet.context.BuildItemContext;import core.net.packet.out.*;
public class PacketRepository{public static void send(Class<?>c,BuildItemContext b){String kind=c==ConstructGroundItem.class?"construct":c==ClearGroundItem.class?"clear":"amount";oracle.Support.emit("packet",kind,b.getPlayer()==null?null:b.getPlayer().id,oracle.ManagerSupport.id((core.game.node.item.GroundItem)b.getItem()),b.getOldAmount());if(c==ConstructGroundItem.class)new ConstructGroundItem().send(b);else if(c==ClearGroundItem.class)new ClearGroundItem().send(b);else new UpdateGroundItemAmount().send(b);}}''')
    adapter('oracle/ManagerSupport.java',read(HERE/'ManagerSupport.java'))
    read(HERE/'generate.py')

def requests():
    cases=[]
    def a(op,**kw):return dict(op=op,**kw)
    def add(actions,**kw):cases.append(dict(manager=True,actions=actions,**kw))
    def g(**kw):return a('newGround',location=[3201,3202,0],**kw)
    create=a('managerCreate',g=0);destroy=a('managerDestroy',g=0);pulse=a('managerPulse')
    for ctor in ['ground','owner','location','player','uid']:
        for owner in [None,0,1]:
            actions=[g(p=owner)]if ctor=='ground'else[]
            add([*actions,a('managerCreate',ctor=ctor,g=0,p=owner,uid=7,absolute=100,location=[3201,3202,0]),a('managerGet',id=1,p=0,location=[3201,3202,0]),a('tick',value=100),pulse,a('tick',value=200),pulse])
    for private in [False,True]:
        for plugin in [None,4]:
            add([a('definition',id=1,tradeable=not private,plugin=plugin),g(),create,a('managerCreate',g=0),destroy,destroy])
    for value in [99,100,101,199,200,201,-2147483648,2147483647]:
        for flags in [{},{'remainPrivate':True},{'forceVisible':True},{'removed':True}]:
            add([g(),create,a('setGround',g=0,**flags),a('tick',value=value),pulse,a('update',p=0),a('update',p=1),pulse])
    for playing in [False,True]:
        for location in [[3201,3202,0],[3216,3202,0],[3216,3203,0],[3201,3202,1],None,[-16384,-16384,0]]:
            add([g(),create,a('playerState',playing=playing,location=location),destroy])
    for owner in [None,0,1]:
        for amount in [-2147483648,-1,0,1,2147483647]:
            add([g(p=owner,amount=amount),create,a('managerIncrease',id=1,amount=2147483647,p=owner,location=[3201,3202,0])])
    for op in ['managerCreate','managerDestroy','chunkAdd','chunkRemove']:
        add([a(op,g=None),pulse])
    for ctor in ['owner','player','uid']:
        add([a('managerCreate',ctor=ctor,input=None,p=None,location=None)])
    for event in ['trade','plugin','pluginRemove','lookup','packet','scene','send','dirty','ticks','uid','playing','location','artificial','ai','auto','active','private','respawn']:
        for at in [1,2]:
            add([a('definition',id=1,plugin=4),g(),a('virtual',g=0,auto=False),create,a('playerState',artificial=True),a('setAI',items=[0,0]),a('captureList',list='ai'),a('tick',value=200),pulse,destroy],failures={event:at})
    for key,value in [('auto',True),('active',False),('private',False)]:
        add([g(),a('virtual',g=0,**{key:value}),create,pulse,destroy])
    add([g(),create,a('setAI',items=[0,0]),a('playerState',artificial=True),a('tick',value=200),pulse])
    add([g(),create,a('mutateList',list='manager',kind='add',g=0),a('tick',value=200),pulse])
    add([g(),create,a('mutateList',list='manager',kind='add',g=None),pulse])
    add([a('input',id=1),a('input',id=2),a('array',items=[0,None,1]),a('managerMany',array=0,p=0,location=[3201,3202,0])])
    add([a('managerMany',array=None,location=None)])
    # Reentrant list changes and callback failures retain their exact prefixes.
    changes=[a('mutateList',list='manager',kind='clear'),a('mutateList',list='manager',kind='add',g=0),a('mutateList',list='chunk',kind='clear'),a('setGround',g=0,removed=True,location=None,decay=500),a('dropper',g=0,p=1),a('definition',id=1,replace=True,tradeable=False,plugin=None),a('itemDefinition',g=0,id=None),a('setAI',items=[]),a('lookup',value=1),a('tick',value=100)]
    for event in ['trade','plugin','pluginRemove','lookup','packet','ticks','uid','artificial','ai','playing','location']:
        for action in changes:
            add([a('definition',id=1,plugin=4),g(),g(id=2),create,a('managerCreate',g=1),a('managerGet',id=1,p=0,location=[3201,3202,0]),a('playerState',artificial=True),a('setAI',items=[0,1]),a('captureList',list='ai'),a('tick',value=200),pulse,destroy],hooks=[dict(event=event,actions=[action])])
    # Trigger iterator modifications specifically during get, not construction.
    for kind in ['clear','add','remove','set']:
        add([g(),g(),a('setItems',items=[0,1]),a('managerGet',id=1,p=1,location=[3201,3202,0])],hooks=[dict(event='ticks',at=3,actions=[a('mutateList',list='chunk',kind=kind,g=0,slot=1)])])
    add([a('definition',id=1,plugin=1),g(),create],hooks=[dict(event='plugin',actions=[a('definition',id=1,plugin=None)])])
    add([g(),create,destroy],hooks=[dict(event='playing',actions=[a('dropper',g=0,p=None)])])
    add([g(),create,a('playerState',artificial=True),a('setAI',items=[0]),a('captureList',list='ai'),a('tick',value=200),pulse],hooks=[dict(event='ai',actions=[a('setAI',items=[])])])
    for kind in ['clear','add','remove','set']:
        add([g(),g(),a('virtual',g=0,private=False),a('setItems',items=[0,1]),a('managerGet',id=1,p=1,location=[3201,3202,0])],hooks=[dict(event='private',actions=[a('mutateList',list='chunk',kind=kind,g=0,slot=1)])])
    for changes in [
        [a('mutateList',list='manager',kind='clear')],
        [a('mutateList',list='manager',kind='remove',g=1)],
        [a('newGround',location=[3201,3202,0]),a('managerCreate',g=2)],
        [a('virtual',g=1,auto=True)],
        [a('setGround',g=1,removed=True)],
    ]:
        add([g(),g(),a('virtual',g=0,auto=False),a('virtual',g=1,auto=False),create,a('managerCreate',g=1),a('tick',value=200),pulse,pulse],hooks=[dict(event='auto',actions=changes)])
    add([g(),a('virtual',g=0,auto=True),create,destroy],hooks=[dict(event='respawn',actions=[create])])
    add([a('managerCreate',ctor='player',input=None,p=0)])
    add([g(),a('newGround',location=[3202,3202,0],p=1),a('setItems',items=[0,1]),a('managerGet',id=1,p=1,locationOf=0)],hooks=[dict(event='ticks',at=3,actions=[a('moveLocation',g=0,value=[3202,3202,0])])])
    add([g(),create,a('input',id=1,amount=1),a('managerIncrease',input=0,p=0,location=[3201,3202,0])],hooks=[dict(event='uid',at=2,actions=[a('inputSet',input=0,amount=7)])])
    add([g(),a('setGround',g=0,forceVisible=True),create,pulse],failures={'dirty':1})
    for at in [1,2]:
        for change in [a('setGround',g=0,location=None),a('setItem',g=0,amount=65537),a('dropper',g=0,p=None)]:
            add([g(),create],hooks=[dict(event='scene',at=at,actions=[change])])
    add([a('input',id=1),a('input',id=2),a('array',items=[0,0]),a('managerMany',array=0,p=0,location=[3201,3202,0])],hooks=[dict(event='default',actions=[a('arraySet',array=0,slot=1,input=1)])])
    for failed in [False,True]:
        add([g(),a('managerPulse')],hooks=[dict(event='default',actions=[create])],failures={'default':1}if failed else{})
    for event,op in [('trade',create),('pluginRemove',destroy),('lookup',create)]:
        add([a('definition',id=1,plugin=4),g(),create,pulse],hooks=[dict(event=event,actions=[op])])
    add([g(),g(),a('setItems',items=[0,1]),a('managerGet',id=1,p=1,location=[3201,3202,0])],hooks=[dict(event='ticks',at=3,actions=[a('mutateList',list='chunk',kind='add',g=None),a('mutateList',list='chunk',kind='remove',g=None)])])
    for tick in [-2147483648,2147483647]:
        add([g(),create,pulse,a('tick',value=tick-100 if tick>0 else tick+100),pulse],tick=tick)
    for name in [None,'Ordinary item','Baby impling jar','Baby Impling jar','impling jar']:
        for options in [None,['drop'],['destroy'],['DiSsOlVe'],[None,'destroy','drop']]:
            for value in [None,False,True,1,'true']:
                add([a('definition',id=1,name=name,options=options,tradeable=value),g(),create])
    for replacement in [None,[1]]:
        add([g(),g(p=1),a('setItems',items=[0,1]),a('captureItems'),a('managerGet',id=1,p=1,location=[3201,3202,0])],hooks=[dict(event='ticks',at=3,actions=[a('setItems',items=replacement)])])
    for private,active in [(False,True),(False,False),(True,False)]:
        add([g(p=None),a('virtual',g=0,private=private,active=active,auto=True),create,a('sync',p=0),a('sync',p=1),pulse,destroy])
    return cases

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args()
    base.c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    import sys
    base.generate(args.check,sys.modules[__name__],'ground-manager')
