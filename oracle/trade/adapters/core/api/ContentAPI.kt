package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.tools.Log

/** Recorded services, plus the original text of removeAttribute and both sendInputDialogue forms. */
fun log(origin: Any, type: Log, message: String) {
    oracle.Support.emit("log", message)
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

/* ORIGINAL_REMOVE_ATTRIBUTE */

/* ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC */

/* ORIGINAL_SEND_INPUT_DIALOGUE_TYPE */