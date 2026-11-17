package core.game.ge

/** GrandExchange.addBotOffer as a recorded service. */
class GrandExchange {
    companion object {
        fun addBotOffer(itemID: Int, amount: Int): Boolean {
            oracle.Support.emit("geOffer", itemID, amount)
            return true
        }
    }
}
