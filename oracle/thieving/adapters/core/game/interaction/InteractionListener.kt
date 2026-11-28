package core.game.interaction

import core.game.node.Node
import core.game.node.entity.player.Player
import oracle.W

/** Adapter: keeps what a listener registers, in order, for the driver. */
interface InteractionListener {
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        W.register(type.name, option, handler)
    }
    fun defineListeners()
}
