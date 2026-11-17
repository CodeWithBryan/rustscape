package core.game.ge

/** `System.currentTimeMillis()` of the GE classes: the case clock (same-package classes shadow java.lang). */
object System {
    @JvmStatic
    fun currentTimeMillis(): Long = oracle.Support.clock()
}