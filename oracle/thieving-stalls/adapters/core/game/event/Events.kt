package core.game.event

import core.game.node.Node

/** Adapter: the event marker and the one event the pulse dispatches, declared as in the original. */
interface Event
data class ResourceProducedEvent(val itemId: Int, val amount: Int, val source: Node, val original: Int = -1) : Event
