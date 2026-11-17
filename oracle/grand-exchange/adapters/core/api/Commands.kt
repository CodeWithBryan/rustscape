package core.api

import core.game.node.entity.player.Player
import core.game.system.command.Privilege

/** Commands with the original define signature; each definition is recorded and its handler kept for the case. */
interface Commands : ContentInterface {
    fun reject(player: Player, vararg message: String) {
        throw UnsupportedOperationException("reject is not reached by the GE commands")
    }

    fun define(name: String, privilege: Privilege = Privilege.ADMIN, usage: String = "", description: String = "", handle: (Player, Array<String>) -> Unit) {
        oracle.Support.command(name, privilege.name, usage, description, handle)
    }

    fun defineCommands()
}