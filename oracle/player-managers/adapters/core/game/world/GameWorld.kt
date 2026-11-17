package core.game.world

import core.game.system.task.Pulse

/** The world pulser: submit records the pulse. */
class Pulser {
    fun submit(pulse: Pulse) {
        oracle.Support.emit("submit", oracle.Support.value(pulse))
    }
}

object GameWorld {
    @JvmStatic val Pulser = Pulser()
}
