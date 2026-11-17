package core.api

import core.game.node.entity.player.Player
import core.game.system.command.Privilege

/** Commands with the original define signature; definitions are recorded, never run (commands are not ported). */
interface Commands : ContentInterface {
    fun reject(player: Player, vararg message: String) {
        throw UnsupportedOperationException("commands are not exercised")
    }

    fun define(name: String, privilege: Privilege = Privilege.ADMIN, usage: String = "", description: String = "", handle: (Player, Array<String>) -> Unit) {
        oracle.Support.command(name, privilege.name)
    }

    fun defineCommands()
}
