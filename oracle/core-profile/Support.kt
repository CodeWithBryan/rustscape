package oracle
import core.game.node.entity.player.Player
import core.game.node.entity.player.info.login.*
import core.game.container.Container
import core.game.node.item.Item
import core.game.world.map.Location
import core.cache.def.impl.ItemDefinition
import org.json.simple.*
import org.json.simple.parser.JSONParser

object Core {
    val player get() = Model.entity as Player
    fun field(obj:Any,name:String):java.lang.reflect.Field {
        var type:Class<*>?=obj.javaClass
        while(type!=null) {try{return type.getDeclaredField(name).apply{isAccessible=true}}catch(e:NoSuchFieldException){type=type.superclass}}
        error(name)
    }
    fun setup(input:JSONObject):Player {
        ItemDefinition.getDefinitions().clear()
        for(i in 0..7){val d=ItemDefinition();field(d,"id").set(d,i);field(d,"name").set(d,"item$i");field(d,"value").set(d,i*11);ItemDefinition.getDefinitions()[i]=d}
        return Player()
    }
    fun container(name:String):Container = when(name){"bank"->player.bankPrimary;"secondary"->player.bankSecondary;"equipment"->player.equipment;else->player.inventory}
    fun number(a:JSONObject,k:String,d:Int=0)=(a[k]as? Number)?.toInt()?:d
    fun action(a:JSONObject):Any? {
        when(a["op"]){
            "coreLoad"->{val root=JSONParser().parse(a["text"]as String)as JSONObject?;PlayerSaveParser(player,root).parseCore()}
            "coreSave"->PlayerSaver(player).saveCoreData(Model.root)
            "coreVarp"->{val i=number(a,"index");player.varpMap[i]=number(a,"value");if(a.containsKey("save"))player.saveVarp[i]=a["save"]as Boolean?}
            "coreSecondary"->player.useSecondaryBank=a["value"]as Boolean
            "coreLocation"->{val v=a["value"]as JSONArray?;player.location=if(v==null)null else Location((v[0]as Number).toInt(),(v[1]as Number).toInt(),(v[2]as Number).toInt())}
            "coreTab"->{val tabs=if(a["secondary"]==true)player.bankSecondary.tabStartSlot else player.bankPrimary.tabStartSlot;tabs[number(a,"index")]=number(a,"value")}
            "coreRaw"->{val c=container(a["container"]as? String?:"inventory");val item=if(a["null"]==true)null else Item(number(a,"id",1),number(a,"amount",1),number(a,"charge",1000));if(item!=null)item.index=number(a,"index",number(a,"slot"));c.toArray()[number(a,"slot")]=item}
            else->error("unknown core action $a")
        }
        return null
    }
    fun observeContainer(c:Container):Any {
        val items=c.toArray().mapIndexedNotNull{i,t->t?.let{listOf(i,it.id,it.amount,it.charge,it.index)}}
        return listOf(c.toArray().size,field(c,"type").get(c).toString(),field(c,"sortType").get(c).toString(),c.listeners.size,items,c.event.slots.toList(),c.event.isClear)
    }
    fun observe():Any {
        val p=player;val loc=p.location
        return linkedMapOf("inventory" to observeContainer(p.inventory),"bank" to observeContainer(p.bankPrimary),"secondary" to observeContainer(p.bankSecondary),"equipment" to observeContainer(p.equipment),"tabs" to p.bankPrimary.tabStartSlot.toList(),"tabsSecondary" to p.bankSecondary.tabStartSlot.toList(),"secondarySelected" to p.useSecondaryBank,"location" to loc?.let{listOf(it.x,it.y,it.z)},"varps" to p.varpMap.entries.map{listOf(it.key,it.value)},"saveVarps" to p.saveVarp.mapKeys{it.key.toString()})
    }
}
fun main(){
    Model.entityFactory=Core::setup;Model.extraAction=Core::action;Model.extraObservation=Core::observe
    for(line in generateSequence(::readLine)){val input=JSONParser().parse(line)as JSONObject;input["expected"]=Model.evaluate(input);println(JSONValue.toJSONString(input))}
}
