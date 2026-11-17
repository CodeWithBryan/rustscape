package core.api

import core.cache.def.impl.ItemDefinition
import core.game.component.Component
import core.game.ge.GrandExchangeRecords
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.tools.Log

/*
 * The ContentAPI members the GE, interface and input classes reach.
 * Original text (inserted at generation time): getAttribute, setAttribute,
 * removeAttribute, openInterface, closeInterface, inInventory,
 * amountInInventory, hasSpaceFor, itemDefinition, getItemName, both
 * sendInputDialogue forms and hasAwaitingGrandExchangeCollections. removeItem
 * and addItem keep their original INVENTORY branch only (no caller passes
 * another container). Everything else is a recorded service.
 */

/** `core.api.Container`: only INVENTORY is passed by the GE classes. */
enum class Container { INVENTORY, BANK, EQUIPMENT, BoB }

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", type.name, message)
}

@JvmOverloads
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("setVarp", oracle.Support.index(player), varpIndex, value)
}

fun sendMessage(player: Player, message: String) {
    oracle.Support.emit("contentMessage", oracle.Support.index(player), message)
}

fun sendDialogue(player: Player, message: String) {
    oracle.Support.emit("dialogue", oracle.Support.index(player), message)
}

fun notify(player: Player, message: String) {
    oracle.Support.emit("notify", oracle.Support.index(player), message)
}

@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: core.game.world.map.Location? = null, radius: Int = 8) {
    oracle.Support.emit("audio", oracle.Support.index(player), id)
}

fun playJingle(player: Player, jingleId: Int) {
    oracle.Support.emit("jingle", oracle.Support.index(player), jingleId)
}

fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    item ?: return false
    val it = when (item) {
        is Item -> item
        is Int -> Item(item)
        else -> throw IllegalStateException("Invalid value passed for item")
    }

    return when (container) {
        Container.INVENTORY -> player.inventory.remove(it)
        else -> throw UnsupportedOperationException("only the inventory branch is reached")
    }
}

fun addItem(player: Player, id: Int, amount: Int = 1, container: Container = Container.INVENTORY): Boolean {
    val cont = when (container) {
        Container.INVENTORY -> player.inventory
        else -> throw UnsupportedOperationException("only the inventory branch is reached")
    }

    return cont.add(Item(id, amount))
}

/* ORIGINAL_GET_ATTRIBUTE */

/* ORIGINAL_SET_ATTRIBUTE */

/* ORIGINAL_REMOVE_ATTRIBUTE */

/* ORIGINAL_OPEN_INTERFACE */

/* ORIGINAL_CLOSE_INTERFACE */

/* ORIGINAL_IN_INVENTORY */

/* ORIGINAL_AMOUNT_IN_INVENTORY */

/* ORIGINAL_HAS_SPACE_FOR */

/* ORIGINAL_ITEM_DEFINITION */

/* ORIGINAL_GET_ITEM_NAME */

/* ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC */

/* ORIGINAL_SEND_INPUT_DIALOGUE_TYPE */

/* ORIGINAL_HAS_AWAITING_COLLECTIONS */