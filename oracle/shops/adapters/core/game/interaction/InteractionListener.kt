package core.game.interaction

import core.api.ContentInterface
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.world.map.Location

/**
 * The InteractionListener members Shops uses, with the original signatures.
 * Registration is recorded by the harness (oracle.Support) instead of
 * InteractionListeners, whose dispatch is not part of this oracle.
 */
interface InteractionListener : ContentInterface {
    val NPC
        get() = IntType.NPC

    fun on(id: Int, type: IntType, vararg option: String, handler: (player: core.game.node.entity.player.Player, node: Node) -> Boolean) {
        oracle.Support.listen(intArrayOf(id), type, option, handler)
    }
    fun on(ids: IntArray, type: IntType, vararg option: String, handler: (player: core.game.node.entity.player.Player, node: Node) -> Boolean) {
        oracle.Support.listen(ids, type, option, handler)
    }
    fun on(type: IntType, vararg option: String, handler: (player: core.game.node.entity.player.Player, node: Node) -> Boolean) {
        oracle.Support.listen(null, type, option, handler)
    }
    fun defineDestinationOverrides() {}
    fun setDest(type: IntType, vararg options: String, handler: (Entity, Node) -> Location) {
        oracle.Support.destination(type, options, handler)
    }
    fun defineListeners()
}
