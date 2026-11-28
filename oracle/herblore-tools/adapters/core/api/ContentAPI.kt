package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import oracle.W

// Adapters: each answers from the case's declared state or records its call.
// amountInInventory, hasLevelStat, rewardXP, getAttribute and setAttribute are the pinned text,
// spliced verbatim over the adapter classes they call.

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
fun sendMessage(player: Player, message: String) { W.emit("sendMessage", message) }
fun sendDialogue(player: Player, message: String) { W.emit("sendDialogue", message) }

/* AMOUNT_IN_INVENTORY */

/* HAS_LEVEL_STAT */

/* REWARD_XP */

/* GET_ATTRIBUTE */

/* SET_ATTRIBUTE */
