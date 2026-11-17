package content.global.bots
import core.game.bots.Script
import core.game.container.Container
import core.game.node.entity.player.Player
/** Recording DoublingMoney: only the two calls the trade makes. */
class DoublingMoney : Script() {
    @JvmField var name: String? = null
    fun tradeReceived(p: Player) {
        oracle.Support.emit("botTradeReceived", name, oracle.Support.index(p))
    }
    fun itemsReceived(p: Player, container: Container?) {
        oracle.Support.emit("botItemsReceived", name, oracle.Support.index(p), oracle.Support.items(container?.toArray()))
    }
}
