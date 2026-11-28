package core.api

import core.game.node.entity.player.Player
import core.game.node.item.Item
import oracle.W

// Adapters: each answers from the case's declared state or records its call.

/** A removal succeeds when at least the amount is held. */
fun <T> removeItem(player: Player, item: T): Boolean {
    val it = when (item) {
        is Item -> item
        is Int -> Item(item)
        else -> throw IllegalStateException("Invalid value passed for item")
    }
    val removed = W.take(it.id, it.amount)
    W.emit("removeItem", it.id, it.amount, removed)
    return removed
}
/** An addition always succeeds. */
fun addItem(player: Player, id: Int, amount: Int = 1): Boolean {
    W.give(id, amount)
    W.emit("addItem", id, amount)
    return true
}
@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1) { W.emit("audio", id, delay, loops) }
