package oracle

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.system.timer.*
import java.util.IdentityHashMap
import org.json.simple.*
import org.json.simple.parser.JSONParser

class Failure: RuntimeException() {
    override fun printStackTrace() { Model.events.add(listOf("exception", "Binding")) }
}
object Model {
    var entityFactory: ((JSONObject)->Entity)? = null
    var extraAction: ((JSONObject)->Any?)? = null
    var extraObservation: (()->Any?)? = null
    var timerFactory: ((JSONObject)->RSTimer?)? = null
    lateinit var entity: Entity
    lateinit var request: JSONObject
    var tick = 0; var step = 0; var modal = false; var delay = 0
    var building = JSONObject(); var created = 0
    val handles = linkedMapOf<String, RSTimer?>()
    val names = IdentityHashMap<RSTimer, String>()
    val configs = IdentityHashMap<RSTimer, JSONObject>()
    val counts = mutableMapOf<String, Int>()
    val events = arrayListOf<Any?>()
    var root = JSONObject()
    fun number(key: String, fallback: Int = 0) = (building[key] as? Number)?.toInt() ?: fallback
    fun text(key: String, fallback: String = "timer") = building[key] as? String ?: fallback
    fun flags() = if (building["death"] == true) arrayOf(TimerFlag.ClearOnDeath) else arrayOf()
    fun name(t: RSTimer?) = if (t == null) null else names[t] ?: error("unlabelled timer")
    fun construct(desc: JSONObject, name: String): RSTimer {
        building = desc
        val timer = timerFactory?.invoke(desc) ?: when (desc["class"] ?: "PlainTimer") {
            "ChildTimer" -> ChildTimer()
            "PersistentTimer" -> PersistentTimer()
            "ChildPersistent" -> ChildPersistent()
            else -> PlainTimer()
        }
        attach(timer, desc, name)
        return timer
    }
    fun attach(timer: RSTimer, desc: JSONObject, name: String) {
        names[timer] = name; handles[name] = timer; configs[timer] = desc
        timer.nextExecution = (desc["next"] as? Number)?.toInt() ?: 0
        timer.lastExecution = (desc["last"] as? Number)?.toInt() ?: 0
    }
    fun resolve(id: String): RSTimer? {
        events.add(listOf("resolve", id))
        val key = (request["registry"] as? JSONObject)?.get(id) as? String ?: return null
        val prototype = handles[key] ?: return null
        building = configs[prototype]!!
        val timer = prototype.retrieveInstance()
        // Constructors produce their own fresh execution fields, not a prototype copy.
        val label = "created${created++}"
        names[timer] = label; handles[label] = timer; configs[timer] = building
        return timer
    }
    fun ticks(): Int {
        val value = tick; events.add(listOf("tick", value)); tick += step
        return value
    }
    fun hook(t: RSTimer, event: String) {
        val name = name(t)!!
        events.add(listOf(event, name))
        val key = "$name:$event"; val count = (counts[key] ?: 0) + 1; counts[key] = count
        for (raw in request["scripts"] as? JSONArray ?: JSONArray()) {
            val rule = raw as JSONObject
            if (rule["timer"] == name && rule["event"] == event && ((rule["at"] as? Number)?.toInt() ?: 1) == count) {
                for (action in rule["actions"] as JSONArray) action(action as JSONObject)
            }
        }
    }
    fun result(t: RSTimer) = configs[t]!!["result"] != false
    fun initial(t: RSTimer): Int { hook(t, "initial"); return (configs[t]!!["initial"] as? Number)?.toInt() ?: t.runInterval }
    fun list(name: String) = when (name) { "active" -> entity.timers.activeTimers; "pending" -> entity.timers.newTimers; else -> entity.timers.toRemoveTimers }
    fun classMatch(t: RSTimer?, type: String) = when(type) { "PlainTimer" -> t is PlainTimer; "PersistentTimer" -> t is PersistTimer; "ChildTimer" -> t is ChildTimer; "ChildPersistent" -> t is ChildPersistent; else -> t is RSTimer }
    fun getClass(type: String): RSTimer? = when(type) {
        "PlainTimer" -> entity.timers.getTimer<PlainTimer>(); "PersistentTimer" -> entity.timers.getTimer<PersistTimer>()
        "ChildTimer" -> entity.timers.getTimer<ChildTimer>(); "ChildPersistent" -> entity.timers.getTimer<ChildPersistent>()
        else -> entity.timers.getTimer<RSTimer>()
    }
    fun removeClass(type: String) { when(type) {
        "PlainTimer" -> entity.timers.removeTimer<PlainTimer>(); "PersistentTimer" -> entity.timers.removeTimer<PersistTimer>()
        "ChildTimer" -> entity.timers.removeTimer<ChildTimer>(); "ChildPersistent" -> entity.timers.removeTimer<ChildPersistent>()
        else -> entity.timers.removeTimer<RSTimer>()
    } }
    @Suppress("UNCHECKED_CAST")
    fun action(a: JSONObject): Any? {
        val timer = handles[a["timer"] as? String]
        val manager = entity.timers
        val value = (a["value"] as? Number)?.toInt() ?: 0
        when(a["op"]) {
            "register" -> manager.registerTimer(timer!!)
            "optionalRegister" -> core.api.registerTimer(entity, timer)
            "remove" -> manager.removeTimer(timer!!)
            "removeId" -> manager.removeTimer(a["id"] as String)
            "getId" -> return name(manager.getTimer(a["id"] as String))
            "getClass" -> return name(getClass(a["class"] as String))
            "removeClass" -> removeClass(a["class"] as String)
            "process" -> manager.processTimers()
            "death" -> manager.onEntityDeath()
            "clear" -> manager.clearTimers()
            "tick" -> tick = value
            "step" -> step = value
            "modal" -> modal = a["value"] as Boolean
            "delay" -> delay = value
            "setTimer" -> when(a["field"]) { "interval" -> timer!!.runInterval = value; "next" -> timer!!.nextExecution = value; "last" -> timer!!.lastExecution = value }
            "rawAdd" -> (list(a["list"] as String) as MutableList<RSTimer?>).add(timer)
            "rawSet" -> (list(a["list"] as String) as MutableList<RSTimer?>)[value] = timer
            "rawRemove" -> list(a["list"] as String).removeAt(value)
            "rawClear" -> list(a["list"] as String).clear()
            "save" -> manager.saveTimers(root)
            "root" -> root = JSONParser().parse(a["text"] as String) as JSONObject
            "parse" -> manager.parseTimers(JSONParser().parse(a["text"] as String) as JSONObject)
            "baseSave" -> (timer as PersistTimer).save(root, entity)
            "baseParse" -> (timer as PersistTimer).parse(JSONParser().parse(a["text"] as String) as JSONObject, entity)
            "fail" -> throw Failure()
            else -> return (extraAction ?: error("unknown action ${a["op"]}"))(a)
        }
        return null
    }
    fun errorName(e: Exception): String = when(e) {
        is NullPointerException -> "Null"; is NumberFormatException -> "Number"; is ClassCastException -> "Type"
        is IndexOutOfBoundsException -> "Index"; is ConcurrentModificationException -> "ConcurrentModification"
        is Failure -> "Binding"; else -> throw e
    }
    fun snapshot(value: Any?, error: String?): Any {
        val timers = handles.mapValues { (_,t) -> t?.let { listOf(it.javaClass.simpleName,it.identifier,it.runInterval,it.nextExecution,it.lastExecution,it.isSoft,it.isAuto,it.flags.map{f->f.name},it is PersistTimer) } }
        val state = linkedMapOf<String,Any?>("result" to value,"error" to error,"tick" to tick,"active" to entity.timers.activeTimers.map{ name(it) },"pending" to entity.timers.newTimers.map{ name(it) },"removing" to entity.timers.toRemoveTimers.map{ name(it) },"timers" to timers,"root" to root.toJSONString(),"events" to events.toList())
        if (extraObservation != null) state["extra"] = extraObservation!!()
        return state
    }
    fun evaluate(input: JSONObject): Any {
        request = input; handles.clear(); names.clear(); configs.clear(); counts.clear(); events.clear(); root = JSONObject(); created = 0
        tick = (input["tick"] as? Number)?.toInt() ?: 0; step = (input["step"] as? Number)?.toInt() ?: 0
        modal = input["modal"] == true; delay = (input["delay"] as? Number)?.toInt() ?: 0
        entity = entityFactory?.invoke(input) ?: if(input["player"] == false) Entity() else Player()
        for ((name,desc) in input["timers"] as JSONObject) { if(desc == null) handles[name as String] = null else construct(desc as JSONObject,name as String) }
        for(key in listOf("active","pending","removing")) for(name in input[key] as? JSONArray ?: JSONArray()) (list(key) as MutableList<RSTimer?>).add(handles[name as? String])
        val states = arrayListOf<Any>()
        for (raw in input["actions"] as JSONArray) {
            events.clear(); var value: Any? = null; var error: String? = null
            try { value = action(raw as JSONObject) } catch(e: Exception) { error = errorName(e) }
            states.add(snapshot(value,error))
        }
        return states
    }
}
open class PlainTimer: RSTimer(Model.number("interval"),Model.text("id"),Model.building["soft"]==true,Model.building["auto"]==true,Model.flags()) {
    override fun run(entity: Entity): Boolean { Model.hook(this,"run"); return Model.result(this) }
    override fun beforeRegister(entity: Entity) { Model.hook(this,"before") }
    override fun onRegister(entity: Entity) { Model.hook(this,"register") }
    override fun onRemoval(entity: Entity) { Model.hook(this,"remove") }
    override fun getInitialRunDelay() = Model.initial(this)
}
class ChildTimer: PlainTimer()
open class PersistentTimer: PersistTimer(Model.number("interval"),Model.text("id"),Model.building["soft"]==true,Model.building["auto"]==true,Model.flags()) {
    override fun run(entity: Entity): Boolean { Model.hook(this,"run"); return Model.result(this) }
    override fun beforeRegister(entity: Entity) { Model.hook(this,"before") }
    override fun onRegister(entity: Entity) { Model.hook(this,"register") }
    override fun onRemoval(entity: Entity) { Model.hook(this,"remove") }
    override fun getInitialRunDelay() = Model.initial(this)
    override fun save(root: JSONObject, entity: Entity) { Model.hook(this,"save"); super.save(root,entity) }
    override fun parse(root: JSONObject, entity: Entity) { Model.hook(this,"parse"); super.parse(root,entity) }
}
class ChildPersistent: PersistentTimer()
fun main() {
    for(line in generateSequence(::readLine)) {
        val input = JSONParser().parse(line) as JSONObject
        input["expected"] = Model.evaluate(input)
        println(JSONValue.toJSONString(input))
    }
}
