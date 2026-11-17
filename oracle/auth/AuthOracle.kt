import core.auth.*
import core.auth.System as Clock
import core.storage.*
import core.game.node.entity.player.*
import core.game.system.SystemManager
import core.game.system.security.EncryptionManager
import core.ServerConstants
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser
import java.sql.Timestamp
import java.sql.SQLDataException
import java.security.SecureRandom
import java.util.TreeMap

private fun n(value:Any?)=(value as Number).toLong()
private fun timestamp(value:Any?):Timestamp {
    val data=value as Map<*,*>;return Timestamp(n(data["millis"])).also {if(data.containsKey("nanos"))it.nanos=n(data["nanos"]).toInt()}
}
private fun encoded(value:Any?):Any?=when(value) {is Timestamp->linkedMapOf("millis" to value.time,"nanos" to value.nanos);is Array<*>->value.map{encoded(it)};else->value}
private val fields=arrayOf("username","password","uid","rights","credits","ip","lastUsedIp","muteEndTime","banEndTime","contacts","blocked","clanName","currentClan","clanReqs","timePlayed","lastLogin","online","joinDate")
private fun set(info:UserAccountInfo,index:Int,value:Any?) {
    val field=UserAccountInfo::class.java.getDeclaredField(fields[index]);field.isAccessible=true
    field.set(info,when(index) {2,3,4->n(value).toInt();7,8,14,15->n(value);16->value as Boolean;17->timestamp(value);else->value as String})
}
private fun snapshot(info:UserAccountInfo):Any {
    val changed=try{info.getChangedFields().first}catch(e:Exception){null}
    return linkedMapOf("values" to encoded(info.toArray()),"hash" to info.hashCode(),"default" to info.isDefault(),"changed" to changed,
        "initial" to try{encoded(info.initialValues)}catch(e:Exception){null})
}
private fun applyFields(info:UserAccountInfo,values:Any?) {
    for((key,value) in (values as? Map<*,*> ?: emptyMap<Any,Any>()))set(info,key.toString().toInt(),value)
}
private class RecordingStorage:AccountStorageProvider {
    val delegate=InMemoryStorageProvider();val calls=mutableListOf<Any>();var fail=""
    var getDelay=0L
    private fun call(method:String,value:Any){calls.add(listOf(method,value));if(fail==method)throw SQLDataException("test storage failure")}
    override fun checkUsernameTaken(username:String):Boolean {call("taken",username);return delegate.checkUsernameTaken(username)}
    override fun getAccountInfo(username:String):UserAccountInfo {call("get",username);val result=delegate.getAccountInfo(username);SystemManager.elapsedMillis+=getDelay;return result}
    override fun store(info:UserAccountInfo){call("store",encoded(info.toArray())!!);delegate.store(info)}
    override fun update(info:UserAccountInfo){call("update",encoded(info.toArray())!!);delegate.update(info)}
    override fun remove(info:UserAccountInfo){call("remove",info.username);delegate.remove(info)}
    override fun getOnlineFriends(username:String):List<String>{call("friends",username);return delegate.getOnlineFriends(username)}
    override fun getUsernamesWithIP(ip:String):List<String>{call("ip",ip);return delegate.getUsernamesWithIP(ip)}
    fun entries():Map<String,UserAccountInfo> {
        val field=InMemoryStorageProvider::class.java.getDeclaredField("storage");field.isAccessible=true
        @Suppress("UNCHECKED_CAST") val map=field.get(delegate) as Map<String,UserAccountInfo>;return TreeMap(map)
    }
}
private fun state(store:RecordingStorage,records:Map<String,UserAccountInfo>):Any {
    val entries=store.entries();val all=TreeMap<String,UserAccountInfo>();for((name,record)in records)all["handle:$name"]=record
    for((name,record)in entries)all["stored:$name"]=record
    val identity=all.map{(name,value)->listOf(name,all.filterValues{it===value}.keys.toList())}
    return linkedMapOf("handles" to records.mapValues{snapshot(it.value)},"stored" to entries.mapValues{snapshot(it.value)},"aliases" to identity,"calls" to store.calls.toList())
}
private fun execute(test:Map<*,*>):List<Any?> {
    SystemManager.elapsedMillis=0;SystemManager.hashDelayMillis=0;SystemManager.verifyDelayMillis=0
    Clock.now=n(test["now"]);ServerConstants.NOAUTH_DEFAULT_ADMIN=test["admin"] as? Boolean ?: true
    SystemManager.salt=test["salt"] as? String ?: "$"+"2a$04$......................";SystemManager.failHash=false
    val store=RecordingStorage();val records=TreeMap<String,UserAccountInfo>()
    val auth:AuthProvider<AccountStorageProvider> = if(test["mode"]=="production") ProductionAuthenticator() else DevelopmentAuthenticator()
    auth.configureFor(store);val result=mutableListOf<Any?>()
    for(raw in test["actions"] as List<*>) {
        val action=raw as Map<*,*>;val op=action["op"] as String;var output:Any?=null;var error=false
        try {
            when(op) {
                "new"->{val record=UserAccountInfo.createDefault( );applyFields(record,action["fields"]);records[action["id"] as String]=record}
                "get"->{records[action["id"] as String]=store.getAccountInfo(action["key"] as String)}
                "set"->set(records[action["id"]]!!,n(action["field"]).toInt(),action["value"])
                "baseline"->records[action["id"]]!!.setInitialReferenceValues()
                "uninitialize"->{val field=UserAccountInfo::class.java.getDeclaredField("initialValues");field.isAccessible=true;field.set(records[action["id"]],null)}
                "timestamp_time"->records[action["id"]]!!.joinDate.time=n(action["value"])
                "timestamp_nanos"->records[action["id"]]!!.joinDate.nanos=n(action["value"]).toInt()
                "store"->store.store(records[action["id"]]!!)
                "update"->store.update(records[action["id"]]!!)
                "remove"->store.remove(records[action["id"]]!!)
                "taken"->output=store.checkUsernameTaken(action["key"] as String)
                "friends"->output=store.getOnlineFriends(action["key"] as String)
                "ip"->output=store.getUsernamesWithIP(action["key"] as String)
                "create"->output=auth.createAccountWith(records[action["id"]]!!)
                "can_create"->output=auth.canCreateAccountWith(records[action["id"]]!!)
                "login"->{val(response,record)=auth.checkLogin(action["username"] as String,action["password"] as String);output=response.ordinal;if(record!=null)records[action["id"] as String]=record}
                "password"->output=auth.checkPassword(Player(Details(action["stored"] as String)),action["password"] as String)
                "update_password"->auth.updatePassword(action["username"] as String,action["password"] as String)
                "clock"->Clock.now=n(action["value"])
                "fail_storage"->store.fail=action["value"] as String
                "fail_hash"->SystemManager.failHash=action["value"] as Boolean
                else->throw IllegalStateException("Unknown auth oracle action: $op")
            }
        }catch(e:Exception){error=true}
        result.add(linkedMapOf("output" to output,"error" to error,"state" to state(store,records)))
    }
    return result
}
private fun factory(test:Map<*,*>):List<Any?> {
    SystemManager.elapsedMillis=0;SystemManager.hashDelayMillis=0;SystemManager.verifyDelayMillis=0
    Clock.now=n(test["now"]);SystemManager.salt="$"+"2a$04$......................";SystemManager.failHash=false
    for(name in listOf("authenticator","storageProvider")) {
        val field=Auth::class.java.getDeclaredField(name);field.isAccessible=true;field.set(null,null)
    }
    ServerConstants.USE_AUTH=false;ServerConstants.PERSIST_ACCOUNTS=false;ServerConstants.NOAUTH_DEFAULT_ADMIN=true
    val ids=java.util.IdentityHashMap<AccountStorageProvider,Int>()
    fun id(provider:AccountStorageProvider?)=provider?.let{ids.getOrPut(it){ids.size+1}}
    fun state():Any {
        val provider=try{Auth.storageProvider}catch(e:Exception){null}
        val auth=try{Auth.authenticator}catch(e:Exception){null}
        val sql=provider as? SQLStorageProvider
        return linkedMapOf("provider" to when(provider){is SQLStorageProvider->"sql";is InMemoryStorageProvider->"memory";else->null},
            "providerId" to id(provider),"auth" to when(auth){is ProductionAuthenticator->"production";is DevelopmentAuthenticator->"development";else->null},
            "authProviderId" to id(auth?.storageProvider),
            "sql" to sql?.let{listOf(it.connectionString,it.connectionUsername,it.connectionPassword)})
    }
    return (test["actions"] as List<*>).map {raw->
        val action=raw as Map<*,*>;var output:Any?=null;var error=false
        try {
            when(action["op"]) {
                "inspect"->{}
                "configure"->{
                    ServerConstants.USE_AUTH=action["use_auth"] as Boolean;ServerConstants.PERSIST_ACCOUNTS=action["persist_accounts"] as Boolean
                    ServerConstants.NOAUTH_DEFAULT_ADMIN=action["default_admin"] as Boolean
                    val db=action["database"] as Map<*,*>
                    ServerConstants.DATABASE_ADDRESS=db["address"] as String?;ServerConstants.DATABASE_NAME=db["name"] as String?
                    ServerConstants.DATABASE_USER=db["username"] as String?;ServerConstants.DATABASE_PASS=db["password"] as String?;ServerConstants.DATABASE_PORT=db["port"] as String?
                    Auth.configure()
                }
                "login"->{val(response,info)=Auth.authenticator.checkLogin(action["username"] as String,action["password"] as String)
                    output=linkedMapOf("response" to response.ordinal,"account" to info?.let{snapshot(it)})}
                "create"->{val info=UserAccountInfo.createDefault();info.username=action["username"] as String;info.password=action["password"] as String
                    var created:Any?=null
                    try{created=Auth.authenticator.createAccountWith(info)}catch(e:Exception){error=true}
                    output=linkedMapOf("created" to created,"account" to snapshot(info))}
                else->throw IllegalStateException("Unknown factory action")
            }
        }catch(e:Exception){error=true}
        linkedMapOf("output" to output,"error" to error,"state" to state())
    }
}
private fun timed(test:Map<*,*>):Any {
    Clock.now=n(test["now"]);SystemManager.elapsedMillis=0;SystemManager.hashDelayMillis=0;SystemManager.verifyDelayMillis=0;SystemManager.failHash=false
    SystemManager.salt="$"+"2a$04$......................";ServerConstants.NOAUTH_DEFAULT_ADMIN=true
    val store=RecordingStorage();val records=TreeMap<String,UserAccountInfo>();val info=UserAccountInfo.createDefault()
    info.username="alice";info.password="password";info.banEndTime=n(test["ban"]);info.online=test["online"] as Boolean;records["a"]=info
    val auth:AuthProvider<AccountStorageProvider> = if(test["mode"]=="production")ProductionAuthenticator()else DevelopmentAuthenticator()
    auth.configureFor(store)
    if(test["existing"]==true){info.password=EncryptionManager().hashPassword("password",SystemManager.salt);store.store(info)}
    SystemManager.hashDelayMillis=n(test["hashDelay"]);SystemManager.verifyDelayMillis=n(test["verifyDelay"]);SystemManager.failHash=test["failHash"] as Boolean;store.getDelay=n(test["getDelay"])
    store.fail=test["failStorage"] as? String ?: ""
    var result:Any?=null;var error=false
    try {
        when(if(test["kind"]=="admission") "admission" else test["operation"]) {
            "admission"->{
                AdmissionLogin.response=AuthResponse.values()[n(test["parseResponse"]).toInt()]
                AdmissionLogin.info=if(test["info"]==true)AdmissionInfo(test["username"] as String,test["password"] as String)else null
                AdmissionWorld.authenticator=auth;val session=AdmissionSession()
                AdmissionGate.read(session,java.nio.ByteBuffer.allocate(0))
                result=linkedMapOf("responses" to session.responses,"ready" to (session.ready!=null))
                session.ready?.let{records["logged"]=it}
            }
            "create"->result=auth.createAccountWith(info)
            "login"->{val(response,record)=auth.checkLogin(test["username"] as String,test["password"] as String);result=response.ordinal;if(record!=null)records["logged"]=record}
            "update"->auth.updatePassword(test["username"] as String,test["password"] as String)
            else->throw IllegalArgumentException("Unknown timed auth operation")
        }
    }catch(e:Exception){error=true}
    return linkedMapOf("result" to result,"error" to error,"now" to Clock.currentTimeMillis(),"state" to state(store,records))
}
fun main() {
    java.util.Locale.setDefault(java.util.Locale.US)
    val parser=JSONParser();val encryption=EncryptionManager()
    generateSequence(::readLine).forEach {line->
        @Suppress("UNCHECKED_CAST") val input=parser.parse(line) as MutableMap<String,Any?>
        when(input["kind"]) {
            "clock"->input["expected"]=timed(input)
            "admission"->input["expected"]=timed(input)
            "factory"->input["expected"]=factory(input)
            "sequence"->input["expected"]=execute(input)
            "hash"->{try{input["expected"]=encryption.hashPassword(input["password"] as String,input["salt"] as String);input["error"]=false}catch(e:Exception){input["expected"]=null;input["error"]=true}}
            "verify"->{try{input["expected"]=encryption.checkPassword(input["password"] as String,input["stored"] as String);input["error"]=false}catch(e:Exception){input["expected"]=null;input["error"]=true}}
            "hash_random"->{input["stored"]=encryption.hashPassword(input["password"] as String);input["expected"]=true}
            "salt"->{val data=(input["bytes"] as List<*>).map{n(it).toByte()}.toByteArray();val random=object:SecureRandom(){override fun nextBytes(bytes:ByteArray){java.lang.System.arraycopy(data,0,bytes,0,bytes.size)}};input["expected"]=EncryptionManager.BCrypt.gensalt(n(input["cost"]).toInt(),random)}
            else->throw IllegalStateException("Unknown auth fixture kind")
        }
        println(JSONValue.toJSONString(input))
    }
}
