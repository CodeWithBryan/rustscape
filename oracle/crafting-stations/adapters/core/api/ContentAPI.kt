package core.api

import core.cache.def.impl.ItemDefinition
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.game.system.task.Pulse
import oracle.W

// Adapters: each answers from the case's declared state or records its call.
// hasLevelDyn, rewardXP, submitIndividualPulse, itemDefinition, getAttribute, setAttribute and the
// Boolean sendInputDialogue are the pinned text, spliced verbatim over the adapter classes they call.

fun amountInInventory(player: Player, id: Int): Int { W.emit("amount", id); return W.amount(id) }
fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean {
    W.emit("inInventory", id, amount)
    return W.amount(id) >= amount
}
/** A removal succeeds when at least the amount is held. */
fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    val it = when (item) {
        is Item -> item
        is Int -> Item(item)
        else -> throw IllegalStateException("Invalid value passed for item")
    }
    val removed = W.take(it.id, it.amount)
    W.emit("removeItem", it.id, it.amount, container.name, removed)
    return removed
}
/** An addition succeeds unless the case declares the inventory full. */
fun addItem(player: Player, id: Int, amount: Int = 1, container: Container = Container.INVENTORY): Boolean {
    val added = !W.flag("full")
    if (added) W.give(id, amount)
    W.emit("addItem", id, amount, container.name, added)
    return added
}
fun sendMessage(player: Player, message: String) { W.emit("sendMessage", message) }
fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) {
    val id = when (anim) {
        is Int -> anim
        else -> throw IllegalStateException("Invalid value passed for anim")
    }
    W.emit("animate", id, forced)
}
@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1) { W.emit("audio", id, delay, loops) }
fun openInterface(player: Player, id: Int) { W.emit("openInterface", id) }
fun closeInterface(player: Player) { W.emit("closeInterface") }
fun sendItemOnInterface(player: Player, iface: Int, child: Int, item: Int, amount: Int = 1) {
    W.emit("itemOnInterface", iface, child, item, amount)
}
fun sendInputDialogue(player: Player, type: InputType, prompt: String, handler: (value: Any) -> Unit) {
    W.emit("input", type.name, prompt)
    W.input = handler
}

/* SEND_INPUT_BOOLEAN */

/* ITEM_DEFINITION */

/* GET_ATTRIBUTE */

/* SET_ATTRIBUTE */

/* HAS_LEVEL_DYN */

/* REWARD_XP */

/* SUBMIT_INDIVIDUAL_PULSE */
