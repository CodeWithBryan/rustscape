package core.game.interaction

import core.game.node.Node
import core.game.node.entity.player.Player
import oracle.W

enum class QueueStrength { WEAK, NORMAL, STRONG, SOFT }

/** Adapter: keeps what a listener registers, in order, for the driver. */
interface InteractionListener {
    fun onUseWith(type: IntType, used: Int, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        W.register(type.name, intArrayOf(used), with, handler)
    }
    fun onUseWith(type: IntType, used: IntArray, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        W.register(type.name, used, with, handler)
    }
    fun defineListeners()
}
