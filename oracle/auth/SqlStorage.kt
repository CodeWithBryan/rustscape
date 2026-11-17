package core.storage
import core.auth.UserAccountInfo
/** Configuration fields are observed; SQL execution remains unsupported here. */
class SQLStorageProvider:AccountStorageProvider {
    var connectionString="";var connectionUsername="";var connectionPassword=""
    fun configure(host:String,database:String,user:String,password:String) {
        connectionString="jdbc:mysql://$host/$database?useTimezone=true&serverTimezone=UTC"
        connectionUsername=user;connectionPassword=password
    }
    override fun checkUsernameTaken(username:String):Boolean=throw UnsupportedOperationException()
    override fun getAccountInfo(username:String):UserAccountInfo=throw UnsupportedOperationException()
    override fun store(info:UserAccountInfo):Unit=throw UnsupportedOperationException()
    override fun update(info:UserAccountInfo):Unit=throw UnsupportedOperationException()
    override fun remove(info:UserAccountInfo):Unit=throw UnsupportedOperationException()
    override fun getOnlineFriends(username:String):List<String> = throw UnsupportedOperationException()
    override fun getUsernamesWithIP(ip:String):List<String> = throw UnsupportedOperationException()
}
