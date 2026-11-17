package core.game.node.entity.player.info
import core.game.container.Container
import core.game.node.entity.player.Player
/** Recording trade log. */
object PlayerMonitor {
    @JvmStatic fun logTrade(player1: Player, player2: Player, container1: Container, container2: Container) {
        oracle.Support.emit("logTrade", oracle.Support.index(player1), oracle.Support.index(player2), oracle.Support.items(container1.toArray()), oracle.Support.items(container2.toArray()))
    }
}
