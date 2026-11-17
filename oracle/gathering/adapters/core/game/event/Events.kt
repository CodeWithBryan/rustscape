package core.game.event

import core.game.node.Node
import oracle.Support

class ResourceProducedEvent(val itemId: Int, val amount: Int, val source: Node, val original: Int = -1) {
    override fun toString(): String = "ResourceProducedEvent($itemId,$amount,${Support.ref(source)},$original)"
}
