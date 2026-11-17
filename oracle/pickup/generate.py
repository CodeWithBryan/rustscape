"""Complete unchanged PickupHandler and PickupPlugin over original ground state."""
import argparse,hashlib,importlib.util,io,itertools,json,os,pathlib,sys,zipfile
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('spawn',ROOT/'oracle/ground-spawn/generate.py');spawn=importlib.util.module_from_spec(spec);spec.loader.exec_module(spawn)
SCOPE='Complete unchanged PickupHandler.kt and PickupPlugin.java joined to unchanged GroundItem/GroundItemManager/GroundSpawn/PulseRunner and original packets. Original GodType forCape/hasAny and RunePouch forItem lookup bodies with explicit canonical enum-field adapters. Player attributes/ironman/owned-item queries, inventory receiver/space/add results, dialogue, log, animation, audio and event delivery are recorded external services; static enum startup item events are excluded. PICK2_2582 is an explicit numeric2582 binding; the pinned bundled ConstLib lacks that symbol (jar/class evidence retained). Native tests separately bind the port to actual native containers. Not full content/profile/registry/approach, inventory listeners or live authoritative Take.'

def transform(name,text,source,c):
    text=spawn.transform(name,text,source,c)
    if name=='oracle/Support.java':
        text=text.replace('static Object coord(','public static Object coord(')
        text=text.replace('SpawnSupport.reset();','SpawnSupport.reset();PickupSupport.reset();')
        text=text.replace('ManagerSupport.snapshot(s);','ManagerSupport.snapshot(s);if(Boolean.TRUE.equals(input.get("pickup")))PickupSupport.snapshot(s);')
    elif name=='oracle/SpawnSupport.java':text=text.replace('default:throw new IllegalArgumentException((String)a.get("op"));','default:return PickupSupport.act(a);')
    elif name=='core/game/node/item/Item.java':
        text=text.replace('public class Item {','public class Item extends core.game.node.Node {')
        text=text.replace('oracle.Support.emit("item",id,amount,charge);','if(!oracle.PickupSupport.initializing)oracle.Support.emit("item",id,amount,charge);')
    elif name=='core/game/node/entity/player/Player.java':
        text=text.replace('public String name;','private String name;')
        text=text.replace('public int id,uid;','public int id,uid;public java.util.Map<String,Object>pickupAttributes=new java.util.HashMap<>();public int inventoryId;public java.util.Set<Integer>owned=new java.util.HashSet<>();public boolean ironman;')
        text=text[:-2]+'''public oracle.PickupSupport.Inventory getInventory(){var v=oracle.PickupSupport.inventories[inventoryId];oracle.Support.emit("inventory",id,v.id);return v;}
        public<T>T getAttribute(String k,T d){Object v=pickupAttributes.get(k);if(v==null)v=d;oracle.Support.emit("deadline",id,k,v);return(T)v;}
        public java.util.Map<String,Object>getAttributes(){return pickupAttributes;}
        public void removeAttribute(String k){pickupAttributes.remove(k);oracle.Support.emit("removeAttribute",id,k);}
        public Dispatch getPacketDispatch(){return new Dispatch();}public class Dispatch{public void sendMessage(String s){oracle.Support.emit("packetMessage",id,s);}}
        public void sendMessage(String s){oracle.Support.emit("message",id,s);}public void sendMessages(String...s){oracle.Support.emit("messages",id,java.util.Arrays.asList(s));}
        public boolean hasItem(core.game.node.item.Item i){boolean v=owned.contains(i.getId());oracle.Support.emit("hasItem",id,i.getId(),v);return v;}
        public Iron getIronmanManager(){return new Iron();}public class Iron{public boolean checkRestriction(){boolean v=ironman;oracle.Support.emit("ironman",id,v);return v;}}
        public Dialogue getDialogueInterpreter(){return new Dialogue();}public class Dialogue{public void sendDialogues(int n,core.game.dialogue.FacialExpression f,String...s){oracle.Support.emit("dialogue",id,n,f.name(),java.util.Arrays.asList(s));}}
        public String rawName(){return name;}public void rawName(String v){name=v;}public String getName(){String v=name;oracle.Support.emit("playerName",id);return v;}public void animate(core.game.world.update.flag.context.Animation a){oracle.Support.emit("animate",id,a.id);}
        public void dispatch(core.game.event.PickUpEvent e){oracle.Support.emit("dispatch",id,e.getId());}}
        '''
    elif name=='core/game/node/item/ItemPlugin.java':text=text[:-1]+'public boolean canPickUp(core.game.node.entity.player.Player p,GroundItem g,int type){boolean v=oracle.PickupSupport.pluginResult;oracle.Support.emit("canPickUp",id,p.id,oracle.ManagerSupport.id(g),type,v);return v;}}'
    elif name=='core/cache/def/impl/ItemDefinition.java':text=text[:-1]+'public static void setOptionHandler(String s,core.game.interaction.OptionHandler h){oracle.Support.emit("registerPickup",s,h==oracle.PickupSupport.handler);}}'
    elif name=='core/game/world/map/RegionManager.java':text=text[:-1]+'public static boolean isTeleportPermitted(Location l){boolean v=oracle.PickupSupport.permitted;oracle.Support.emit("permitted",oracle.Support.coord(l),v);return v;}}'
    elif name=='core/api/API.kt':text+='\nfun getItemName(id:Int):String {val name=oracle.ManagerSupport.definition(id).name;oracle.Support.emit("itemName",id);return name}\nfun playAudio(p:core.game.node.entity.player.Player,id:Int){oracle.Support.emit("audio",p.id,id)}\n'
    elif name=='core/game/world/GameWorld.kt':text=text.replace('@JvmStatic fun getTicks():Int','@get:JvmStatic val ticks:Int get()')
    return text

def register(adapter,source,read,c):
    spawn.register(adapter,source,read,c)
    for name in ['core/game/global/action/PickupHandler.kt','content/global/handlers/item/PickupPlugin.java']:
        adapter(name,source(name)[1])
    adapter('core/game/interaction/OptionHandler.java','package core.game.interaction;public abstract class OptionHandler implements core.plugin.Plugin<Object>{public abstract boolean handle(core.game.node.entity.player.Player p,core.game.node.Node n,String o);public core.game.world.map.Location getDestination(core.game.node.Node n,core.game.node.Node d){return null;}}')
    adapter('core/plugin/Plugin.java','package core.plugin;public interface Plugin<T>{Plugin<T>newInstance(T v)throws Throwable;}')
    adapter('core/plugin/Initializable.java','package core.plugin;public @interface Initializable{}')
    adapter('core/game/dialogue/FacialExpression.java','package core.game.dialogue;public enum FacialExpression{FURIOUS}')
    adapter('core/game/node/entity/player/info/LogType.java','package core.game.node.entity.player.info;public enum LogType{DROP_TRADE}')
    adapter('core/game/node/entity/player/info/PlayerMonitor.java','package core.game.node.entity.player.info;public class PlayerMonitor{public static void log(core.game.node.entity.player.Player p,LogType t,String s){oracle.Support.emit("dropLog",p==null?null:p.id,t.name(),s);}}')
    adapter('core/game/world/update/flag/context/Animation.java','package core.game.world.update.flag.context;public class Animation{public int id;private Animation(int i){id=i;}public static Animation create(int i){return new Animation(i);}}')
    adapter('core/game/event/PickUpEvent.kt','package core.game.event\nclass PickUpEvent(val id:Int)')
    # The bundled pinned jar predates this named constant. Preserve evidence of
    # the gap rather than silently fetching a newer, unpinned dependency.
    evidence=json.loads(read(HERE/'consts-evidence.json'))
    pin=json.loads(read(ROOT/'upstream.lock.json'))['server']['commit']
    blob=spawn.manager.base.run(['git','-C',ROOT/'reference/server','show',pin+':Server/libs/ConstLib-1.4.jar']).stdout
    assert len(blob)==evidence['bytes'] and hashlib.sha256(blob).hexdigest()==evidence['sha256']
    with zipfile.ZipFile(io.BytesIO(blob))as z:sound=z.read('org/rs09/consts/Sounds.class')
    assert hashlib.sha256(sound).hexdigest()==evidence['soundsClassSha256'] and b'PICK2_2582'not in sound
    adapter('org/rs09/consts/Sounds.java','package org.rs09.consts;public class Sounds{public static final int PICK2_2582='+str(evidence['numericAdapter']['PICK2_2582'])+';}')
    source('core/game/node/entity/impl/GameAttributes.java');source('core/api/ContentAPI.kt');read(ROOT/'reference/server/Server/pom.xml')
    _,god=source('content/data/GodType.java')
    adapter('content/data/GodType.java','package content.data;import core.game.node.item.Item;import core.game.node.entity.player.Player;public enum GodType{SARADOMIN(2412),GUTHIX(2413),ZAMORAK(2414);public Item cape;GodType(int id){cape=new Item(id);}public Item getCape(){return cape;}'+c.method(god,'public static GodType forCape(')+c.method(god,'public static boolean hasAny(')+'}')
    _,pouch=source('content/global/skill/runecrafting/RunePouch.java')
    adapter('content/global/skill/runecrafting/RunePouch.java','package content.global.skill.runecrafting;import core.game.node.item.Item;public enum RunePouch{SMALL(5509),MEDIUM(5510),LARGE(5512),GIANT(5514);public Item pouch,decayed;RunePouch(int i){pouch=new Item(i);decayed=new Item(i+1);}public Item getPouch(){return pouch;}public Item getDecayedPouch(){return decayed;}'+c.method(pouch,'public static RunePouch forItem(')+'}')
    adapter('oracle/PickupSupport.java',read(HERE/'PickupSupport.java'));read(HERE/'generate.py')

def requests():
    rows=[]
    def a(op,**kw):return dict(op=op,**kw)
    def add(actions,**kw):rows.append(dict(manager=True,spawns=True,pickup=True,actions=actions,**kw))
    def setup(id=1,amount=1,**kw):return[a('newGround',id=id,amount=amount,location=[3201,3202,0],p=None),a('managerCreate',g=0),a('pickupSet',**kw)]
    for id,owned,iron,private,spawned in itertools.product([1,2412,2413,2414,5509,5510,5511,5512,5513,5514,5515,5516,8857,8858,8859,8860], [[],[2413],[5509,5510,5511,5512,5513,5514,5515]], [False,True],[False,True],[False,True]):
        steps=([a('spawnNew',id=id,rate=2,location=[3201,3202,0]),a('spawnInit')]if spawned else setup(id))
        add(steps+[a('pickupSet',owned=owned,ironman=iron),a('setGround',g=0,remainPrivate=private),a('take',g=0),a('take',g=0)])
    for amount,space,added in itertools.product([-2147483648,-1,0,1,2,2147483647],[False,True],[False,True]):
        add(setup(amount=amount)+[a('inventorySet',space=space,add=added),a('take',g=0)])
    for p,uid,deadline,tick in itertools.product([None,0,1],[-1,7,9],[0,1,100,2147483647],[-2147483648,0,1,2147483647]):
        add([a('newGround',p=p,location=[3201,3202,0]),a('managerCreate',g=0),a('player',uid=uid),a('pickupSet',deadline=deadline),a('tick',value=tick),a('take',g=0)])
    for option in [None,False,0,'yes']:
        add(setup()+[a('pickupSet',pickup=option),a('pickupHandle',g=0),a('pickupSet',removePickup=True),a('pickupHandle',g=0)])
    for name in ['take','canTake','pickupHandle']:
        for p,g in itertools.product([None,0],[None,0]):add(setup()+[a(name,p=p,g=g)])
    for deadline in ['bad',None,True]:add(setup()+[a('pickupSet',deadline=deadline),a('take',g=0)])
    for plugin,result,permitted in itertools.product([None,0,5],[False,True],[False,True]):
        add(setup()+[a('definition',plugin=plugin),a('pickupSet',pluginResult=result,permitted=permitted),a('canTake',g=0,type=2),a('pickupHandle',g=0),a('spawnWorld'),a('sync'),a('resetAll')])
    events=['deadline','ticks','item','inventory','space','uid','ironman','hasItem','plugin','canPickUp','addInventory','itemName','playerName','dropLog','permitted','animate','lookup','dirty','audio','dispatch','removeAttribute']
    mutations=[a('fail'),a('setGround',g=0,removed=True,location=None),a('setItem',g=0,amount=17),a('player',uid=9),a('definition',plugin=None),a('pickupSet',inventory=1),a('take',g=0)]
    for event,mutation in itertools.product(events,mutations):
        # Owner p1 allows late drop-trade logging; force-public bypasses privacy.
        prefix=[a('newGround',location=[3201,3202,0],p=1),a('managerCreate',g=0),a('setGround',g=0,forceVisible=True),a('definition',plugin=5),a('pickupSet',permitted=False)]
        add(prefix+[a('pickupHandle',g=0),a('take',g=0)],hooks=[dict(event=event,at=2 if event in ['ticks','item','uid']else 1,actions=[mutation])])
    for loc in [None,[0,0,0]]:add([a('newGround',location=loc,p=None),a('take',g=0),a('managerCreate',g=0),a('take',g=0)])
    for iron,private,force,owner in itertools.product([False,True],repeat=4):
        add([a('newGround',location=[3201,3202,0],p=0 if owner else 1),a('managerCreate',g=0),a('setGround',g=0,remainPrivate=private,forceVisible=force),a('pickupSet',ironman=iron),a('canTake',g=0,type=1),a('take',g=0)])
    for at,mutation in itertools.product([2,3],[a('definition',plugin=None),a('definition',plugin=7),a('setGround',g=0,removed=True),a('fail')]):
        add(setup()+[a('definition',plugin=5),a('take',g=0)],hooks=[dict(event='plugin',at=at,actions=[mutation])])
    for id in [2412,2413,2414,5509,5510,5511,5512,5513,5514,5515,5516,99]:
        for cmd in [a('capeSet',index=1,id=99),a('pouchSet',index=0,id=99,decayed=True),a('pouchSet',index=1,id=99)]:
            add(setup(id)+[cmd,a('pickupSet',owned=[99]),a('canTake',g=0)])
    for mutation in [a('capeSet',index=1,id=99),a('pickupSet',owned=[2413]),a('fail')]:
        add(setup(2412,owned=[99])+[a('take',g=0)],hooks=[dict(event='hasItem',actions=[mutation])])
    for event in ['packetMessage','message','messages','dialogue']:
        prefix=setup(8858 if event=='dialogue' else 2412 if event=='messages' else 5509 if event=='message' else 1,owned=[2412,5509])
        if event=='packetMessage':prefix+=[a('inventorySet',space=False)]
        add(prefix+[a('pickupHandle',g=0)],failures={event:1})
    add([a('pickupRegister'),a('pickupRegister'),a('pickupDestination')])
    return rows

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args()
    spawn.manager.base.c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
    spawn.manager.base.generate(args.check,sys.modules[__name__],'pickup')
