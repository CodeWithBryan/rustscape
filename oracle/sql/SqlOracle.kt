import core.auth.UserAccountInfo
import core.auth.System as Clock
import core.storage.SQLStorageProvider
import core.game.system.communication.CommunicationInfo
import core.game.world.repository.Repository
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser
import java.sql.*
import java.util.TreeMap
import java.io.File

private fun n(value:Any?)=(value as Number).toLong()
private fun encoded(value:Any?):Any?=when(value){is Timestamp->linkedMapOf("millis" to value.time,"nanos" to value.nanos);is Array<*>->value.map{encoded(it)};else->value}
private val fields=arrayOf("username","password","uid","rights","credits","ip","lastUsedIp","muteEndTime","banEndTime","contacts","blocked","clanName","currentClan","clanReqs","timePlayed","lastLogin","online","joinDate")
private fun set(info:UserAccountInfo,index:Int,value:Any?){
    val field=UserAccountInfo::class.java.getDeclaredField(fields[index]);field.isAccessible=true
    val converted=when(index){2,3,4->n(value).toInt();7,8,14,15->n(value);16->value as Boolean;17->{val stamp=value as Map<*,*>;Timestamp(n(stamp["millis"])).also{if(stamp.containsKey("nanos"))it.nanos=n(stamp["nanos"]).toInt()}};else->value as String}
    field.set(info,converted)
}
private fun snapshot(info:UserAccountInfo):Any=linkedMapOf("values" to encoded(info.toArray()),"initial" to try{encoded(info.initialValues)}catch(e:Exception){null},
    "changed" to try{info.getChangedFields().first}catch(e:Exception){null},"hash" to info.hashCode(),"default" to info.isDefault())
private const val ROWS="SELECT username,password,UID,rights,credits,ip,lastGameIp,muteTime,banTime,contacts,blocked,clanName,currentClan,clanReqs,timePlayed,lastLogin,CAST(online AS SIGNED),DATE_FORMAT(joined_date,'%Y-%m-%d %H:%i:%s.%f') FROM members ORDER BY UID"
private fun rows(root:Connection):Any?=try {root.createStatement().use{statement->statement.executeQuery(ROWS).use{result->val rows=mutableListOf<Any>();while(result.next()){rows.add((1..18).map{result.getObject(it)})};rows}}}catch(e:Exception){null}
private fun execute(test:Map<*,*>,host:String,port:Int,database:String,schema:String,expectedDir:String,tls:Boolean):List<Any?> {
    Clock.now=n(test["now"]);Repository.online=emptySet();Repository.lookups.clear()
    val url="jdbc:mysql://$host:$port/?useTimezone=true&serverTimezone=UTC"
    val records=TreeMap<String,UserAccountInfo>();val output=mutableListOf<Any?>()
    DriverManager.getConnection(url,"root","").use {root->
        root.createStatement().use{statement->statement.executeQuery("SELECT @@datadir").use{data->check(data.next());check(File(data.getString(1)).canonicalFile==File(expectedDir).canonicalFile)}}
        check(database=="rs09_jvm")
        val prepared=schema.replace("CREATE DATABASE global;","DROP DATABASE IF EXISTS $database; CREATE DATABASE $database;").replace("USE global;","USE $database;")
        root.createStatement().use{statement->for(sql in prepared.split(';'))if(sql.isNotBlank())statement.execute(sql)}
        val store=SQLStorageProvider();store.configure("$host:$port",database,"root","")
        store.getConnection().use{transport->transport.createStatement().use{statement->statement.executeQuery("SHOW SESSION STATUS LIKE 'Ssl_cipher'").use{cipher->check(cipher.next());check(cipher.getString(2).isNotEmpty()==tls)}}}
        for(raw in test["actions"] as List<*>) {
            val action=raw as Map<*,*>;val id=action["id"] as? String ?: "";var result:Any?=null;var error=false
            try {
                when(action["op"]) {
                    "new"->{val info=UserAccountInfo.createDefault();for((key,value)in (action["fields"] as? Map<*,*> ?: emptyMap<Any,Any>()))set(info,key.toString().toInt(),value);records[id]=info}
                    "set"->set(records[id]!!,n(action["field"]).toInt(),action["value"])
                    "get"->{records[id]=store.getAccountInfo(action["key"] as String)}
                    "store"->store.store(records[id]!!)
                    "update"->store.update(records[id]!!)
                    "remove"->store.remove(records[id]!!)
                    "baseline"->records[id]!!.setInitialReferenceValues()
                    "uninitialize"->{val field=UserAccountInfo::class.java.getDeclaredField("initialValues");field.isAccessible=true;field.set(records[id],null)}
                    "timestamp_time"->records[id]!!.joinDate.time=n(action["value"])
                    "timestamp_nanos"->records[id]!!.joinDate.nanos=n(action["value"]).toInt()
                    "clock"->Clock.now=n(action["value"])
                    "taken"->result=store.checkUsernameTaken(action["key"] as String)
                    "ip"->result=store.getUsernamesWithIP(action["key"] as String)
                    "friends"->result=store.getOnlineFriends(action["key"] as String)
                    "online"->{Repository.online=(action["value"] as List<*>).map{it as String}.toSet()}
                    "sql"->root.createStatement().use{it.execute(action["value"] as String)}
                    else->throw IllegalStateException("Unknown SQL action")
                }
            }catch(e:Exception){error=true}
            output.add(linkedMapOf("output" to result,"error" to error,"records" to records.mapValues{snapshot(it.value)},"rows" to rows(root),"lookups" to Repository.lookups.toList()))
        }
    }
    return output
}
fun main(args:Array<String>){
    java.util.Locale.setDefault(java.util.Locale.US);Class.forName("com.mysql.cj.jdbc.Driver");val parser=JSONParser()
    val schema=File(args[3]).readText(Charsets.UTF_8)
    generateSequence(::readLine).forEach{line->
        @Suppress("UNCHECKED_CAST") val test=parser.parse(line)as MutableMap<String,Any?>
        when(test["kind"]) {
            "sequence"->test["expected"]=execute(test,args[0],args[1].toInt(),args[2],schema,args[4],args[5]=="tls")
            "contacts"->{try{test["expected"]=CommunicationInfo.parseContacts(test["text"]as String).keys.toList();test["error"]=false}catch(e:Exception){test["expected"]=null;test["error"]=true}}
            "map_order"->{val map=HashMap<String,Int>();for((i,key)in (test["keys"]as List<*>).withIndex())map[key as String]=i;test["expected"]=map.keys.toList()}
            "update_query"->{val method=SQLStorageProvider.Companion::class.java.getDeclaredMethod("buildUpdateInfoQuery",ArrayList::class.java);method.isAccessible=true;test["expected"]=method.invoke(SQLStorageProvider.Companion,ArrayList((test["indices"]as List<*>).map{n(it).toInt()}))}
            else->throw IllegalStateException("Unknown SQL case")
        }
        println(JSONValue.toJSONString(test))
    }
}
