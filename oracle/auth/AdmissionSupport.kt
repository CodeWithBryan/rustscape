import core.auth.*
import java.nio.ByteBuffer
/** Decoder/world boundaries are explicit adapters; the original event prefix is verbatim. */
data class AdmissionInfo(val username:String,val password:String)
class AdmissionSession {
    val responses=mutableListOf<Int>();var ready:UserAccountInfo?=null
    fun write(response:AuthResponse){responses.add(response.ordinal)}
}
object AdmissionLogin {
    var response=AuthResponse.Success;var info:AdmissionInfo?=null
    fun decodeFromBuffer(buffer:ByteBuffer):Pair<AuthResponse,AdmissionInfo?> = Pair(response,info)
}
object AdmissionWorld {lateinit var authenticator:AuthProvider<AccountStorageProviderAlias>}
typealias AccountStorageProviderAlias=core.storage.AccountStorageProvider
