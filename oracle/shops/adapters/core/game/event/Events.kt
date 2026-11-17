package core.game.event

import core.game.node.item.Item

/** Event and the two shop events, as in the original Events.kt. */
interface Event {}

data class ItemShopPurchaseEvent(val itemId: Int, val amount: Int, val currency: Item) : Event
data class ItemShopSellEvent(val itemId: Int, val amount: Int, val currency: Item) : Event
