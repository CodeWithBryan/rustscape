package core.game.interaction

import core.game.node.Node
import core.game.node.entity.player.Player
import oracle.Support

enum class QueueStrength { WEAK, NORMAL, STRONG, SOFT }

/** Records what a listener registers and keeps the actual handler objects for the driver. */
interface InteractionListener {
    fun defineInteraction(type: IntType, ids: IntArray, vararg options: String, persistent: Boolean = false, allowedDistance: Int = 1, handler: (player: Player, node: Node, state: Int) -> Boolean) {
        Registry.interactions.add(handler)
        Support.emit("listeners.defineInteraction", null, type.name, ids.toList(), options.toList(), persistent, allowedDistance)
    }
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        Registry.handlers.add(handler)
        Support.emit("listeners.on", null, type.name, option.toList())
    }
    fun defineListeners()
}
object Registry {
    @JvmField val interactions = ArrayList<(Player, Node, Int) -> Boolean>()
    @JvmField val handlers = ArrayList<(Player, Node) -> Boolean>()
}
