import core.ServerConstants
import core.game.system.config.ServerConfigParser
import core.game.world.GameWorld
import core.game.world.map.Location
import core.tools.mysql.Database
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser
import java.io.File
import java.security.Permission
import java.util.TreeMap

private class Exit(val code:Int):SecurityException()
private fun encode(value:Any?):Any?=when(value) {
    is Location->listOf(value.x,value.y,value.z)
    is Database->listOf(value.host(),value.name(),value.username(),value.password())
    is Enum<*>->value.name
    else->value
}
fun main(args:Array<String>) {
    java.util.Locale.setDefault(java.util.Locale.US)
    val fields=File(args[0]).readLines().map{ServerConstants::class.java.getDeclaredField(it).also{it.isAccessible=true}}
    val defaults=fields.associateWith{it.get(null)}
    val file=File(args[1]);val parser=JSONParser()
    java.lang.System.setSecurityManager(object:SecurityManager(){
        override fun checkPermission(permission:Permission){}
        override fun checkExit(status:Int){throw Exit(status)}
    })
    fun state():Any {
        val constants=TreeMap<String,Any?>();for(field in fields)constants[field.name]=encode(field.get(null))
        val world=GameWorld.settings
        val values=world?.let {value->TreeMap<String,Any?>().also{out->for(field in value.javaClass.declaredFields){
            if(java.lang.reflect.Modifier.isStatic(field.modifiers))continue
            field.isAccessible=true;out[field.name]=encode(field.get(value))
        };out["isHosted"]=value.isHosted}}
        val shops:Any=try{core.api.getServerConfig().getBoolean("world.personalized_shops",false)}catch(e:Exception){"error"}
        return linkedMapOf("constants" to constants,"world" to values,"worldString" to world?.toString(),"tomlLoaded" to (ServerConfigParser.tomlData!=null),"personalizedShops" to shops,"listenerPorts" to ListenerPorts.value())
    }
    generateSequence(::readLine).forEach {line->
        @Suppress("UNCHECKED_CAST") val test=parser.parse(line) as MutableMap<String,Any?>
        var error=false;var exit:Int?=null;var value:Any?=null
        for((field,initial)in defaults)field.set(null,initial)
        GameWorld.settings=null;ServerConfigParser.tomlData=null;ServerConfigParser.confFile=null
        val kind=test["kind"]
        try {
            when(kind) {
                "path"->value=ServerConfigParser.parsePath(test["text"] as String)
                "location"->value=encode(ServerConfigParser.parseLocation(test["text"] as String))
                "config"->{
                    val results=mutableListOf<Any?>()
                    for(raw in test["actions"] as List<*>) {
                        val action=raw as Map<*,*>;error=false;exit=null
                        java.lang.System.setProperty("java.version",action["javaVersion"] as? String ?: "11.0.32")
                        file.writeText(action["text"] as String,Charsets.UTF_8)
                        try {
                            if(action["source"]=="path")ServerConfigParser.parse(file.path)
                            else ServerConfigParser.parse(file.toURI().toURL())
                        }catch(e:Exit){error=true;exit=e.code}catch(e:Throwable){error=true}
                        results.add(linkedMapOf("error" to error,"exit" to exit,"state" to state()))
                    }
                    value=results
                }
                else->throw IllegalArgumentException("Unknown configuration case")
            }
        }catch(e:Exit){error=true;exit=e.code}catch(e:Throwable){error=true}
        test["expected"]=value
        if(kind!="config"){test["error"]=error;test["exit"]=exit}
        println(JSONValue.toJSONString(test))
    }
}
