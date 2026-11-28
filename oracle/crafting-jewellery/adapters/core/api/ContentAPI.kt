package core.api

import core.game.node.entity.player.Player
import oracle.W

// Adapters: each records its call. The Boolean sendInputDialogue is the pinned text, spliced verbatim.

@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1) { W.emit("audio", id, delay, loops) }
fun sendInputDialogue(player: Player, type: InputType, prompt: String, handler: (value: Any) -> Unit) {
    W.emit("input", type.name, prompt)
    W.input = handler
}

/* SEND_INPUT_BOOLEAN */
