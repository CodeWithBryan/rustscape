package core.api

import com.moandjiezana.toml.Toml
import content.data.Quests
import core.cache.def.impl.ItemDefinition
import core.game.component.Component
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.game.world.GameWorld
import core.tools.Log

/*
 * The ContentAPI members the shop, interface and input classes reach.
 * Original text (inserted at generation time): getAttribute, setAttribute,
 * removeAttribute, openInterface, setInterfaceText, inInventory, hasSpaceFor,
 * itemDefinition, getItemName, getWorldTicks and both sendInputDialogue forms.
 * removeItem and addItem keep their original INVENTORY branch only (the
 * Container enum's bank, equipment and familiar branches are never selected
 * by these callers). Everything else is a recorded service.
 */

/** `core.api.Container`: only INVENTORY is passed by the shop classes. */
enum class Container { INVENTORY, BANK, EQUIPMENT, BoB }

fun log(origin: Any, type: Log, message: String) {
    oracle.Support.emit("log", type.name, message)
}

@JvmOverloads
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("varp", oracle.Support.index(player), varpIndex, value)
    core.net.packet.PacketRepository.send(core.net.packet.out.Config::class.java, core.net.packet.context.ConfigContext(player, varpIndex, value))
}

fun sendMessage(player: Player, message: String) {
    oracle.Support.emit("contentMessage", oracle.Support.index(player), message)
}

fun sendDialogue(player: Player, message: String) {
    oracle.Support.emit("dialogue", oracle.Support.index(player), message)
}

fun getServerConfig(): Toml {
    return Toml()
}

fun getQuestPoints(player: Player): Int {
    oracle.Support.emit("questPoints", oracle.Support.index(player))
    return oracle.Support.questPoints(player)
}

fun getQuestStage(player: Player, quest: Quests): Int {
    oracle.Support.emit("questStage", oracle.Support.index(player), quest.name)
    return oracle.Support.questStage(player, quest.name)
}

fun isQuestComplete(player: Player, quest: Quests): Boolean {
    oracle.Support.emit("questComplete", oracle.Support.index(player), quest.name)
    return oracle.Support.questStage(player, quest.name) == 100
}

fun sendNPCDialogue(player: Player, npc: Int, msg: String, expr: core.game.dialogue.FacialExpression = core.game.dialogue.FacialExpression.NEUTRAL,
                    hide: Boolean = false) {
    oracle.Support.emit("npcDialogue", oracle.Support.index(player), npc, msg, expr.name, hide)
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

/* ORIGINAL_SET_INTERFACE_TEXT */

/* ORIGINAL_IN_INVENTORY */

/* ORIGINAL_HAS_SPACE_FOR */

/* ORIGINAL_ITEM_DEFINITION */

/* ORIGINAL_GET_ITEM_NAME */

/* ORIGINAL_GET_WORLD_TICKS */

/* ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC */

/* ORIGINAL_SEND_INPUT_DIALOGUE_TYPE */
