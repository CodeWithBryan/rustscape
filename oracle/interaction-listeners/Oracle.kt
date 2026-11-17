package oracle
import core.game.interaction.*
import core.game.node.Node
import core.game.node.item.Item
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.map.Location
import java.util.Locale
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser
typealias Obj=Map<String,Any?>
fun num(m:Obj,k:String,d:Int=0)= (m[k] as? Number)?.toInt()?:d
fun bool(m:Obj,k:String,d:Boolean=false)=m[k] as? Boolean?:d
fun str(m:Obj,k:String,d:String="use")=m[k] as? String?:d
fun ints(v:Any?,d:IntArray=intArrayOf(1)):IntArray=(v as? List<*>)?.map{(it as Number).toInt()}?.toIntArray()?:d
fun options(m:Obj):Array<String> = (m["options"] as? List<*>)?.map{it as String}?.toTypedArray()?:arrayOf(str(m,"option"))
interface Identified{val code:Int;val category:String}
class OnHandler(override val code:Int):Identified,(Player,Node)->Boolean{override val category="on";override fun invoke(p:Player,n:Node):Boolean{Model.emit("invokeOn",code,p.tag,n.tag);return Model.result}}
class UseHandler(override val code:Int):Identified,(Player,Node,Node)->Boolean{override val category="use";override fun invoke(p:Player,u:Node,w:Node):Boolean{Model.emit("invokeUse",code,p.tag,u.tag,w.tag);return Model.result}}
class PredHandler(override val code:Int):Identified,(Int,Int)->Boolean{override val category="predicate";override fun invoke(u:Int,w:Int):Boolean{Model.emit("predicate",code,u,w);return Model.predicate}}
class DestHandler(override val code:Int):Identified,(Entity,Node)->Location{override val category="destination";override fun invoke(p:Entity,n:Node)=Location(n.tag)}
class InterHandler(override val code:Int):Identified,(Player,Node,Int)->Boolean{override val category="interaction";override fun invoke(p:Player,n:Node,s:Int)=true}
class UseInterHandler(override val code:Int):Identified,(Player,Node,Node,Int)->Boolean{override val category="useInteraction";override fun invoke(p:Player,u:Node,w:Node,s:Int)=true}
class Scripts(val id:Int){fun removeWeakScripts(){Model.emit("removeWeak",id)};fun setInteractionScript(n:Node,s:Interaction){Model.emit("install",id,n.tag,Model.functionId(s.execution),s.distance,s.persist);Model.installed=Model.functionId(s.execution)}}
class Locks{val isInteractionLocked:Boolean get(){Model.emit("interactionLocked");return Model.locked};val isMovementLocked:Boolean get(){Model.emit("movementLocked");return Model.movement}}
class Manager(val id:Int){fun run(p:MovementPulse){Model.emit("runPulse",id,Model.pulses.indexOf(p))}}
class Zone(val id:Int){fun interact(n:Node,o:Option):Boolean{Model.emit("zoneOn",id,n.tag,Model.units(o.name),o.index);return Model.zone};fun useWith(u:Item,w:Node):Boolean{Model.emit("zoneUse",id,u.tag,w.tag);return Model.zone}}
object Model {
 lateinit var input:Obj;lateinit var nodes:Array<Node>;lateinit var player:Player
 var result=true;var predicate=true;var zone=false;var locked=false;var movement=false;var option:String?=null;var installed:Any?=null
 val events=ArrayList<Any?>();val calls=HashMap<String,Int>();val hooks=HashSet<Int>();val pulses=ArrayList<MovementPulse>()
 val on=Array(2){OnHandler(it)};val use=Array(2){UseHandler(it)};val pred=Array(2){PredHandler(it)};val dest=Array(2){DestHandler(it)};val inter=Array(2){InterHandler(it)};val useInter=Array(2){UseInterHandler(it)}
 fun units(s:String)=s.map{it.code}
 fun functionId(v:Any?):Any?=if(v is Identified)listOf(v.category,v.code)else null
 fun emit(event:String,vararg args:Any?){events.add(listOf(event)+args.toList());val count=(calls[event]?:0)+1;calls[event]=count
  val hs=input["hooks"] as? List<*>?:emptyList<Any?>();for((i,h0)in hs.withIndex()){val h=h0 as Obj;if(str(h,"event")==event&&num(h,"at",1)==count&&hooks.add(i)){for(a in h["actions"] as List<*>)act(a as Obj)}}
  val fs=input["failures"] as? Obj?:emptyMap();if((fs[event] as? Number)?.toInt()==count)throw Failure()
 }
 fun act(a:Obj):Any?{val f=num(a,"function");val t=num(a,"kind",1);val k=if(t in 0..4)IntType.values()[t]else IntType.SCENERY;val id=num(a,"id",123);val opt=str(a,"option");val u=num(a,"used",1);val w=num(a,"with_",2)
  return when(str(a,"op","")){
   "add"->{InteractionListeners.add(id,t,options(a),on[f]);null}
   "addArrays"->{InteractionListeners.add(ints(a["ids"],intArrayOf(id)),t,options(a),on[f]);null}
   "generic"->{InteractionListeners.add(options(a),t,on[f]);null}
   "use"->{InteractionListeners.add(u,w,t,use[f]);null}
   "useIds"->{InteractionListeners.add(t,u,ints(a["with_"],intArrayOf(w)),use[f]);null}
   "useArrays"->{InteractionListeners.add(t,ints(a["used"],intArrayOf(u)),ints(a["with_"],intArrayOf(w)),use[f]);null}
   "any"->{InteractionListeners.add(t,ints(a["with_"],intArrayOf(w)),use[f]);null}
   "wildcard"->{InteractionListeners.addWildcard(t,pred[f],use[f]);null}
   "equip"->{if(bool(a,"equip",true))InteractionListeners.addEquip(id,on[f])else InteractionListeners.addUnequip(id,on[f]);null}
   "get"->functionId(InteractionListeners.get(id,t,opt))
   "getGeneric"->functionId(InteractionListeners.get(opt,t))
   "getEquip"->functionId(if(bool(a,"equip",true))InteractionListeners.getEquip(id)else InteractionListeners.getUnequip(id))
   "getDest"->functionId(when(str(a,"mode","specific")){"specific"->InteractionListeners.getOverride(t,id,opt);"id"->InteractionListeners.getOverride(t,id);else->InteractionListeners.getOverride(t,opt)})
   "instant"->{val name=str(a,"class","oracle.OnHandler");if(bool(a,"enabled",true))InteractionListeners.instantClasses.add(name)else InteractionListeners.instantClasses.remove(name);null}
   "isInstant"->if(bool(a,"use"))InteractionListeners.isUseWithInstant(use[f])else InteractionListeners.isInstant(on[f])
   "getUse"->functionId(InteractionListeners.get(u,w,t))
   "destination"->{when(str(a,"mode","specific")){"specific"->InteractionListeners.addDestOverrides(t,intArrayOf(id),options(a),dest[f]);"id"->InteractionListeners.addDestOverride(t,id,dest[f]);else->InteractionListeners.addDestOverrides(t,options(a),dest[f])};null}
   "metadata"->{val m=InteractionListener.InteractionMetadata(inter[f],num(a,"distance",1),bool(a,"persist"));if(bool(a,"generic"))InteractionListeners.addGenericMetadata(options(a),k,m)else InteractionListeners.addMetadata(ints(a["ids"],intArrayOf(id)),k,options(a),m);null}
   "useMetadata"->{val m=InteractionListener.UseWithMetadata(useInter[f],num(a,"distance",1),bool(a,"persist"));if(a["with_"] is List<*>)InteractionListeners.addMetadata(u,ints(a["with_"]),k,m)else InteractionListeners.addMetadata(u,w,k,m);null}
   "run"->InteractionListeners.run(id,k,opt,player,nodes[num(a,"node",2)])
   "runUse"->InteractionListeners.run(nodes[num(a,"usedNode",1)],nodes[num(a,"withNode",2)],k,player)
   "runEquip"->InteractionListeners.run(id,player,nodes[num(a,"node",2)],bool(a,"equip",true))
   "pulse"->pulses.getOrNull(num(a,"index"))?.pulse()
   "field"->{when(str(a,"name","")){"manager"->player.managerId=num(a,"value");"scripts"->player.scriptsId=num(a,"value");"zone"->player.zoneId=num(a,"value");"nodeId"->nodes[num(a,"node",2)].rawId=num(a,"value");"locked"->locked=bool(a,"value");"movement"->movement=bool(a,"value");"result"->result=bool(a,"value");"predicate"->predicate=bool(a,"value");"locale"->Locale.setDefault(if(str(a,"value")=="tr")Locale("tr")else Locale.ROOT)};null}
   else->throw AssertionError(a)
  }
 }
 fun value(v:Any?):Any?=when(v){is Identified->functionId(v);is InteractionListener.InteractionMetadata->listOf(functionId(v.handler),v.distance,v.persist);is InteractionListener.UseWithMetadata->listOf(functionId(v.handler),v.distance,v.persist);is Pair<*,*>->listOf(value(v.first),value(v.second));is List<*>->v.map{value(it)};else->v}
 fun snapshot(result:Any?,error:String?,message:Any?):Any?{val maps=linkedMapOf<String,Any?>();for(field in InteractionListeners::class.java.declaredFields){field.isAccessible=true;val v=field.get(null);if(v is Map<*,*>){maps[field.name]=v.entries.sortedBy{it.key.toString()}.map{listOf(units(it.key.toString().replace(Regex("\\[I@[0-9a-f]+"),"[I@array")),value(it.value))}}};return linkedMapOf("result" to result,"error" to error,"message" to message,"events" to events.toList(),"maps" to maps,"option" to option?.let{units(it)},"installed" to installed,"pulseCount" to pulses.size,"playerFields" to listOf(player.scriptsId,player.managerId,player.zoneId),"nodeIds" to nodes.map{it.rawId})}
 fun reset(row:Obj){input=row;events.clear();calls.clear();hooks.clear();pulses.clear();option=null;installed=null;player=Player(0);nodes=arrayOf(player,Item(1,1),Node(2,2),Player(3));result=bool(row,"result",true);predicate=bool(row,"predicate",true);locked=bool(row,"locked");movement=bool(row,"movementLocked");zone=bool(row,"zone");Locale.setDefault(if(str(row,"locale","ROOT")=="tr")Locale("tr")else Locale.ROOT)
  for(field in InteractionListeners::class.java.declaredFields){field.isAccessible=true;val v=field.get(null);if(v is MutableMap<*,*>)v.clear()};InteractionListeners.instantClasses.clear();if(bool(row,"instant")){InteractionListeners.instantClasses.add("oracle.OnHandler");InteractionListeners.instantClasses.add("oracle.UseHandler")}
 }
}
class Failure:RuntimeException()
fun main(){val reader=System.`in`.bufferedReader(Charsets.UTF_8);reader.forEachLine{line->val row=JSONParser().parse(line) as MutableMap<String,Any?>;Model.reset(row);val expected=ArrayList<Any?>();for(action in row["actions"] as List<*>){var result:Any?=null;var error:String?=null;var message:Any?=null;try{result=Model.act(action as Obj)}catch(e:Throwable){error=when(e){is Failure->"Binding";is NullPointerException->"Null";is IndexOutOfBoundsException->"Index";is ClassCastException->"Type";is ConcurrentModificationException->"Concurrent";is IllegalStateException->"State";else->e.javaClass.simpleName};if(e is IllegalStateException)message=Model.units(e.message?:"")};expected.add(Model.snapshot(result,error,message))};row["expected"]=expected;println(JSONValue.toJSONString(row))}}
