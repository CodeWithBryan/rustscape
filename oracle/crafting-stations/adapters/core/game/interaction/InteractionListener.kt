package core.game.interaction

import core.game.node.Node
import core.game.node.entity.player.Player
import oracle.W

/** Adapter: keeps what a listener registers, in order, for the driver. */
interface InteractionListener {
    fun onUseWith(type: IntType, used: Int, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        W.registerUse(type.name, intArrayOf(used), with, handler)
    }
    fun onUseWith(type: IntType, used: IntArray, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        W.registerUse(type.name, used, with, handler)
    }
    fun defineListeners()
}
