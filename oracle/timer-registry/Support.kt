package oracle
import core.api.*
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.system.timer.*
import org.json.simple.*
import org.json.simple.parser.JSONParser
object RegistryModel {
    var logs=0
    fun setup(input:JSONObject):Entity{RegistryReset.reset();logs=0;return if(input["player"]==false)Entity()else Player()}
    fun create(desc:JSONObject):RSTimer?=when(desc["class"]){"FactoryTimer"->FactoryTimer();"FactoryChild"->FactoryChild();"FactoryPersistent"->FactoryPersistent();else->null}
    fun log(level:Int,message:String){
        Model.events.add(listOf("registryLog",if(level==2)"Warn" else "Error",message));logs++
        if((Model.request["logFailAt"]as? Number)?.toInt()==logs)throw Failure()
    }
    fun factory(t:RSTimer,args:Array<out Any>?,fresh:()->RSTimer):RSTimer{
        Model.hook(t,if(args==null)"retrieve" else "factory");Model.events.add(listOf("arguments",args?.toList()))
        val desc=Model.configs[t]!!;val target=desc["factoryTarget"]as? String
        if(target!=null)return Model.handles[target]!!
        Model.building=desc;val instance=fresh();val label="created${Model.created++}"
        Model.names[instance]=label;Model.handles[label]=instance;Model.configs[instance]=desc
        if(desc["useArgs"]==true&&args!=null)instance.runInterval=(args[0]as Number).toInt()
        return instance
    }
    @Suppress("UNCHECKED_CAST")
    fun action(a:JSONObject):Any? {
        val args=((a["args"]as? JSONArray)?.toTypedArray()?:arrayOf())as Array<Any>
        val id=a["id"]as? String?:"missing";val type=a["class"]as? String?:"RSTimer"
        val timer=Model.handles[a["timer"]as? String]
        when(a["op"]){
            "registryRegister"->TimerRegistry.registerTimer(timer!!)
            "registryPut"->(TimerRegistry.timerMap as MutableMap<String,RSTimer?>)[id]=timer
            "autoAppend"->(TimerRegistry.autoTimers as MutableList<RSTimer?>).add(timer)
            "autoClear"->TimerRegistry.autoTimers.clear()
            "auto"->TimerRegistry.addAutoTimers(Model.entity)
            "registryGet"->return Model.name(TimerRegistry.getTimerInstance(id,*args))
            "getOrStart"->return Model.name(getOrStartTimer(Model.entity,id,*args))
            "has"->return hasTimerActive(Model.entity,id)
            "registryType"->return Model.name(when(type){"PlainTimer"->TimerRegistry.getTimerInstance<PlainTimer>(*args);"PersistentTimer"->TimerRegistry.getTimerInstance<PersistTimer>(*args);"FactoryChild"->TimerRegistry.getTimerInstance<FactoryChild>(*args);else->TimerRegistry.getTimerInstance<RSTimer>(*args)})
            "spawnType"->return Model.name(when(type){"PlainTimer"->spawnTimer<PlainTimer>(*args);"PersistentTimer"->spawnTimer<PersistTimer>(*args);"FactoryChild"->spawnTimer<FactoryChild>(*args);else->spawnTimer<RSTimer>(*args)})
            "getOrStartType"->return Model.name(when(type){"PlainTimer"->getOrStartTimer<PlainTimer>(Model.entity,*args);"PersistentTimer"->getOrStartTimer<PersistTimer>(Model.entity,*args);"FactoryChild"->getOrStartTimer<FactoryChild>(Model.entity,*args);else->getOrStartTimer<RSTimer>(Model.entity,*args)})
            else->error("unknown registry action $a")
        }
        return null
    }
    fun observe():Any=mapOf("map" to TimerRegistry.timerMap.entries.map{listOf(it.key,Model.name(it.value))},"auto" to TimerRegistry.autoTimers.map{Model.name(it)})
}
open class FactoryTimer:PlainTimer(){override fun retrieveInstance()=RegistryModel.factory(this,null){super.retrieveInstance()};override fun getTimer(vararg args:Any)=RegistryModel.factory(this,args){super.retrieveInstance()}}
class FactoryChild:FactoryTimer()
class FactoryPersistent:PersistentTimer(){override fun retrieveInstance()=RegistryModel.factory(this,null){super.retrieveInstance()};override fun getTimer(vararg args:Any)=RegistryModel.factory(this,args){super.retrieveInstance()}}
fun main(){
    Model.entityFactory=RegistryModel::setup;Model.timerFactory=RegistryModel::create;Model.extraAction=RegistryModel::action;Model.extraObservation=RegistryModel::observe
    for(line in generateSequence(::readLine)){val input=JSONParser().parse(line)as JSONObject;input["expected"]=Model.evaluate(input);println(JSONValue.toJSONString(input))}
}
