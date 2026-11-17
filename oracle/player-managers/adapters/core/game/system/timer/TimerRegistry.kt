package core.game.system.timer

import core.game.system.timer.impl.Skulled

/** getTimerInstance: the registry holds Skulled under "skulled" unless the case empties it. */
object TimerRegistry {
    fun getTimerInstance(identifier: String, vararg args: Any): RSTimer? {
        oracle.Support.emit("timerInstance", identifier, args.toList())
        if (!oracle.Support.registry() || identifier.lowercase() != "skulled") return null
        val t = Skulled().getTimer(*args)
        oracle.Support.track(t)
        return t
    }
}
