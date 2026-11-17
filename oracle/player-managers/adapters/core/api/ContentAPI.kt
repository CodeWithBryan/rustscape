package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.system.timer.RSTimer
import core.game.system.timer.TimerRegistry
import core.tools.Log

/** Recorded services, with the original text of removeTimer, registerTimer and spawnTimer. */
@JvmOverloads
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("varp", player.slot, varpIndex, value, save)
}

@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1) {
    oracle.Support.emit("audio", player.slot, id, delay, loops)
}

fun getWorldTicks(): Int {
    return 0
}

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", type.name, message)
}

/* ORIGINAL_REGISTER_TIMER */

/* ORIGINAL_SPAWN_TIMER */

/* ORIGINAL_REMOVE_TIMER */
