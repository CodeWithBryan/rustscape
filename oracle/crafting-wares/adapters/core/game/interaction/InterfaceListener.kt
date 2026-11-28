package core.game.interaction

import core.game.component.Component
import core.game.node.entity.player.Player
import oracle.W

/** Adapter: keeps what a listener registers with `on(componentID, handler)`, in order, for the driver. */
interface InterfaceListener {
    fun on(componentID: Int, handler: (player: Player, component: Component, opcode: Int, buttonID: Int, slot: Int, itemID: Int) -> Boolean) {
        W.registerButton(componentID, handler)
    }
    fun defineInterfaceListeners()
}
