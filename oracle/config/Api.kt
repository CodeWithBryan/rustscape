package core.api
import core.tools.Log
fun log(source:Any?,level:Log,message:Any?) {}
inline fun <reified E:Enum<E>> parseEnumEntry(name:String):E? = try {enumValueOf<E>(name)}catch(e:Exception){null}
