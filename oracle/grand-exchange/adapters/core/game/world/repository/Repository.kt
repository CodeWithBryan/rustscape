package core.game.world.repository

import core.game.node.entity.player.Player

/** The Repository members the GE classes read: online players, uid_map and GE news (recorded). */
object Repository {
    @JvmStatic
    val players: List<Player>
        get() {
            oracle.Support.emit("players")
            return oracle.Support.online()
        }

    val uid_map: Map<Int, Player> = oracle.Support.UidMap()

    @JvmStatic
    fun sendGrandExchangeNews(string: String, icon: Int = 12, color: String = "CC6600") {
        oracle.Support.emit("news", string)
    }
}