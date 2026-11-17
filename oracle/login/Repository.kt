package core.game.world.repository
/** Test-only online lookup. Authentication and world state are not simulated. */
object Repository {
    val online = mutableSetOf<String>()
    val lookups = mutableListOf<String>()
    fun getPlayerByName(name: String): Any? {lookups.add(name);return if (online.contains(name)) name else null}
}
