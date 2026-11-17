package core.game.system.timer

import core.game.node.entity.Entity

/** RSTimer's constructor properties and `run`; registration and scheduling are the timer ports'. */
abstract class RSTimer(var runInterval: Int, val identifier: String = "generictimer", val isSoft: Boolean = false, val isAuto: Boolean = false) {
    abstract fun run(entity: Entity): Boolean
}