package oracle
import core.game.interaction.*
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.map.Location
import core.game.world.map.path.Path
import org.json.simple.*
import org.json.simple.parser.JSONParser
import java.io.PrintWriter
import java.util.IdentityHashMap

class Failure:RuntimeException()
class CallbackFailure(val kind:String):RuntimeException(){override fun printStackTrace(pw:PrintWriter){pw.print("callback:$kind\n")}}
class Exec(val key:String):Function1<Int,Boolean>,Function3<Player,Node,Int,Boolean>,Function4<Player,Node,Node,Int,Boolean>{
    override fun invoke(stage:Int)=call(listOf(stage))
    override fun invoke(p:Player,n:Node,stage:Int)=call(listOf(p.id,n.id,stage))
    override fun invoke(p:Player,n:Node,w:Node,stage:Int)=call(listOf(p.id,n.id,w.id,stage))
    fun call(args:List<Int>):Boolean{try{Model.hook("exec:$key","invoke",args);val cfg=(Model.input["executors"]as? JSONObject)?.get(key)as? JSONObject;val by=cfg?.get("byStage")as? JSONObject;return by?.get(args.last().toString())as? Boolean?:cfg?.get("result")as? Boolean?:false}catch(e:Exception){throw CallbackFailure(Model.error(e))}}
    override fun equals(other:Any?):Boolean{Model.hook("exec:$key","equals",(other as? Exec)?.key);return other is Exec && Model.group(key)==Model.group(other.key)}
    override fun hashCode()=System.identityHashCode(this)
}
object Model {
    lateinit var input:JSONObject
    lateinit var entity:Entity
    val p get()=entity.scripts
    val processors=mutableMapOf<String,ScriptProcessor>()
    val scripts=sortedMapOf<String,Script<*>?>();val names=IdentityHashMap<Script<*>,String>();val execs=sortedMapOf<String,Exec>()
    val nodes=sortedMapOf<Int,Node>();val nodeData=sortedMapOf<Int,JSONObject>();val locations=sortedMapOf<String,Location>()
    val nodeLocations=mutableMapOf<String,Location?>()
    val events=arrayListOf<Any?>();val counts=mutableMapOf<String,Int>()
    var tick=0;var step=0;var movement=0;var lock=0;var created=0
    var own:Location?=null;var modal=false;var opened=false;var chatbox=false;var stun=false;var running=false;var sight=true
    fun number(o:JSONObject,k:String,d:Int=0)=(o[k]as? Number)?.toInt()?:d
    fun group(key:String):String=((input["executors"]as? JSONObject)?.get(key)as? JSONObject)?.get("group")as? String?:key
    fun exec(key:String?)=key?.let{execs.getOrPut(it){Exec(it)}}
    fun loc(v:Any?):Location?=when(v){null->null;is String->locations[v];is JSONArray->Location((v[0]as Number).toInt(),(v[1]as Number).toInt(),(v[2]as Number).toInt());else->error("bad location $v")}
    fun xyz(v:Location?)=v?.let{listOf(it.x,it.y,it.z)}
    fun name(s:Script<*>?):String?{if(s==null)return null;return names[s]?:"created${created++}".also{names[s]=it;scripts[it]=s}}
    fun field(key:String,owner:ScriptProcessor=p):Any?{val f=ScriptProcessor::class.java.getDeclaredField(key);f.isAccessible=true;return f.get(owner)}
    @Suppress("UNCHECKED_CAST") fun queue()=field("queue")as ArrayList<Script<*>>
    fun hook(who:String,event:String,vararg args:Any?){events.add(listOf(who,event)+args.toList());val key="$who:$event";val count=(counts[key]?:0)+1;counts[key]=count;for(raw in input["hooks"]as? JSONArray?:JSONArray()){val r=raw as JSONObject;if(r["who"]==who&&r["event"]==event&&(r["at"]as? Number)?.toInt().let{it?:1}==count)for(a in r["actions"]as JSONArray)action(a as JSONObject)}}
    @JvmStatic fun ticks():Int{val value=tick;tick+=step;hook("world","ticks",value);return value}
    @JvmStatic fun delayed(e:Entity):Boolean{hook("entity","delayed");val delay=e.scripts.delay;return delay>core.game.world.GameWorld.ticks}
    @JvmStatic fun ownLocation():Location?{hook("entity","location");return own}
    @JvmStatic fun nodeLocation(id:Int):Location?{hook("node:$id","location");return nodeLocations["$id:location"]}
    @JvmStatic fun center(id:Int):Location?{hook("node:$id","center");return nodeLocations["$id:center"]}
    @JvmStatic fun faceLocation(id:Int,own:Location?):Location?{hook("node:$id","face",xyz(own));return nodeLocations["$id:face"]}
    @JvmStatic fun active(id:Int):Boolean{hook("node:$id","active");return nodeData[id]?.get("active")!=false}
    @JvmStatic fun describe(id:Int):String{hook("node:$id","describe");return "node:$id"}
    @JvmStatic fun flag(name:String):Boolean{hook("entity",name);return when(name){"modal"->modal;"opened"->opened;"chatbox"->chatbox;"running"->running;"stun"->stun;else->error(name)}}
    @JvmStatic fun dest(flag:String,id:Int):Location?{hook("dest",flag,id);return nodeLocations["$id:destination"]}
    @JvmStatic fun path(id:Int?):Path{hook("path","find",id);val d=nodeData[id]!!;val pts=d["points"]as? JSONArray?:JSONArray().apply{val l=nodeLocations["$id:location"];if(l!=null)add(JSONArray().apply{add(l.x);add(l.y)})};return Path(pts.map{val a=it as JSONArray;core.game.world.map.path.Point((a[0]as Number).toInt(),(a[1]as Number).toInt())},d["near"]==true)}
    @JvmStatic fun simple(name:String){hook("entity",name)}
    @JvmStatic fun message(text:String){hook("entity","message",text)}
    @JvmStatic fun face(l:Location){hook("entity","face",xyz(l))}
    @JvmStatic fun los(l:Node):Boolean{hook("los","test",xyz(l.location));return sight}
    @JvmStatic fun lock(ticks:Int){hook("entity","lock",ticks);lock=ticks}
    @JvmStatic fun log(owner:Any,level:String,text:String){hook("log",level,(owner as Class<*>).simpleName,text)}
    fun action(a:JSONObject):Any?{
        val p=(a["processor"]as? String)?.let{processors[it]}?:this.p
        val s=(a["script"]as? String)?.let{scripts[it]};val value=number(a,"value");val n=(a["node"]as? Number)?.toInt()?.let{nodes[it]}
        when(a["op"]){
            "aliasProcessor"->processors[a["name"]as String]=p
            "newProcessor"->processors[a["name"]as String]=ScriptProcessor(entity)
            "sameProcessor"->return p===processors[a["other"]as String]
            "processorEntity"->return p.entity.id
            "readProcessor"->return listOf(p.delay,p.interacted,p.apRangeCalled,p.apRange,p.persistent,name(field("apScript",p)as Script<*>?),name(field("opScript",p)as Script<*>?),name(field("currentScript",p)as Script<*>?),(field("interactTarget",p)as Node?)?.id,xyz(p.targetDestination),(field("queue",p)as ArrayList<Script<*>>).map{name(it)})
            "add"->p.addToQueue(s!!,QueueStrength.valueOf(a["strength"]as? String?:"WEAK"))
            "queue"->return p.processQueue()
            "execute"->return p.executeScript(s!!)
            "interact"->p.processInteractScript(s!!)
            "setInteraction"->p.setInteractionScript(n!!,s)
            "pre"->p.preMovement()
            "post"->p.postMovement(a["value"]as Boolean)
            "reset"->p.reset()
            "weak"->p.removeWeakScripts()
            "normal"->p.removeNormalScripts()
            "has"->return p.hasTypeInQueue(QueueStrength.valueOf(a["strength"]as String))
            "approach"->return p.inApproachDistance(s!!)
            "operable"->return p.inOperableDistance()
            "active"->return name(p.getActiveScript())
            "state"->s!!.state=value
            "next"->s!!.nextExecution=value
            "tick"->tick=value
            "step"->step=value
            "delay"->p.delay=value
            "destination"->p.targetDestination=loc(a["value"])
            "own"->own=loc(a["value"])
            "moveClock"->entity.clocks[0]=value
            "modal"->modal=a["value"]as Boolean
            "opened"->opened=a["value"]as Boolean
            "chatbox"->chatbox=a["value"]as Boolean
            "stun"->stun=a["value"]as Boolean
            "running"->running=a["value"]as Boolean
            "sight"->sight=a["value"]as Boolean
            "nodeActive"->nodeData[number(a,"node")]!!["active"]=a["value"]
            "mutateLocation"->{val l=locations[a["name"]as String]!!;val v=a["value"]as JSONArray;l.x=(v[0]as Number).toInt();l.y=(v[1]as Number).toInt();l.z=(v[2]as Number).toInt()}
            "delayScript"->return core.api.delayScript(entity,value)
            "delayEntity"->core.api.delayEntity(entity,value)
            "range"->core.api.apRange(entity,value)
            "clearScripts"->return core.api.clearScripts(entity)
            "restartScript"->return core.api.restartScript(entity)
            "keepRunning"->return core.api.keepRunning(entity)
            "stopExecuting"->return core.api.stopExecuting(entity)
            "apiQueue"->core.api.queueScript(entity,number(a,"delay",1),QueueStrength.valueOf(a["strength"]as? String?:"WEAK"),a["persist"]==true,exec(a["execution"]as String)!!)
            "fail"->throw Failure()
            else->error("unknown action $a")
        };return null
    }
    fun error(e:Exception):String=when(e){is NullPointerException->"Null";is IndexOutOfBoundsException->"Index";is Failure->"Binding";else->throw e}
    fun observe(result:Any?,error:String?):Any{
        // Assign fresh API-created scripts in graph traversal order before maps.
        queue().forEach{ name(it) };listOf("apScript","opScript","currentScript").forEach{name(field(it)as Script<*>?)}
        val state=scripts.mapValues{(_,s)->s?.let{listOf(it.javaClass.simpleName,(it.execution as? Exec)?.key,it.persist,it.state,it.nextExecution)}}
        return mapOf("result" to result,"error" to error,"queue" to queue().map{ name(it) },"scripts" to state,"approach" to name(field("apScript")as Script<*>?),"operable" to name(field("opScript")as Script<*>?),"current" to name(field("currentScript")as Script<*>?),"active" to name(p.getActiveScript()),"target" to (field("interactTarget")as Node?)?.id,"delay" to p.delay,"interacted" to p.interacted,"rangeCalled" to p.apRangeCalled,"range" to p.apRange,"persistent" to p.persistent,"destination" to xyz(p.targetDestination),"own" to xyz(own),"locations" to locations.mapValues{xyz(it.value)},"tick" to tick,"movement" to entity.clocks[0],"lock" to lock,"events" to events.toList())
    }
    fun evaluate(o:JSONObject):Any{
        input=o;processors.clear();scripts.clear();names.clear();execs.clear();nodes.clear();nodeData.clear();nodeLocations.clear();locations.clear();events.clear();counts.clear();tick=number(o,"tick");step=number(o,"step");lock=0;created=0;modal=o["modal"]==true;opened=o["opened"]==true;chatbox=o["chatbox"]==true;stun=o["stun"]==true;running=o["running"]==true;sight=o["sight"]!=false
        for((k,v)in o["locations"]as? JSONObject?:JSONObject()){locations[k as String]=loc(v)!!}
        own=loc(if(o.containsKey("own"))o["own"]else JSONArray().apply{add(0);add(0);add(0)})
        entity=when(o["kind"]){"ai"->core.game.bots.AIPlayer();"other"->Entity(0);else->Player()};nodes[0]=entity;nodeData[0]=JSONObject().apply{put("active",true)};entity.clocks[0]=number(o,"movement")
        for(id in 1..4){val d=(o["nodes"]as? JSONObject)?.get(id.toString())as? JSONObject?:JSONObject().apply{put("location",JSONArray().apply{add(id);add(0);add(0)})};nodeData[id]=d;nodes[id]=when(d["kind"]){"npc"->core.game.node.entity.npc.NPC(id);"scenery"->core.game.node.scenery.Scenery(id);"ground"->core.game.node.item.GroundItem(id);else->Node(id)}}
        for((name,raw)in o["scripts"]as JSONObject){if(raw==null){scripts[name as String]=null;continue};val d=raw as JSONObject;val f=exec(d["execution"]as? String?:if(d.containsKey("execution"))null else "f0");val persist=d["persist"]==true;val strength=QueueStrength.valueOf(d["strength"]as? String?:"WEAK");val s=when(d["form"]){"interaction"->Interaction(f!!,number(d,"distance",1),persist);"use"->UseWithInteraction(f!!,number(d,"distance",1),persist,nodes[1]!!,nodes[2]!!);"queuedUse"->QueuedUseWith(f!!,strength,persist,nodes[1]!!,nodes[2]!!);"basic"->Script(f,persist);else->QueuedScript(f!!,strength,persist)};s.state=number(d,"state");s.nextExecution=number(d,"next");scripts[name as String]=s;names[s]=name}
        for((id,d)in nodeData){val l=loc(d["location"]);nodeLocations["$id:location"]=l;for(k in listOf("center","face","destination"))nodeLocations["$id:$k"]=if(d.containsKey(k))loc(d[k])else l}
        processors["main"]=p;val states=arrayListOf<Any>();for(a in o["actions"]as JSONArray){events.clear();var result:Any?=null;var error:String?=null;try{result=action(a as JSONObject)}catch(e:Exception){error=Model.error(e)};states.add(observe(result,error))};return states
    }
}
fun main(){for(line in generateSequence(::readLine)){val o=JSONParser().parse(line)as JSONObject;o["expected"]=Model.evaluate(o);println(JSONValue.toJSONString(o))}}
