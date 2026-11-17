package core.game.world.repository
/** Controlled live-name set; the real SQL and contact parser remain original. */
object Repository {
    var online=setOf<String>()
    val lookups=mutableListOf<String>()
    fun getPlayerByName(name:String):Any? {lookups.add(name);return if(name in online)name else null}
}
