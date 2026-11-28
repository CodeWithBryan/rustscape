package core.game.interaction

import core.game.node.Node
import core.game.node.entity.player.Player
import oracle.W

/** Adapter: keeps what a listener registers, in order, for the driver. */
interface InteractionListener {
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        W.registerOption(type.name, option, handler)
    }
    fun onUseWith(type: IntType, used: IntArray, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        W.registerUse(type.name, used, with, handler)
    }
    fun defineListeners()
}
