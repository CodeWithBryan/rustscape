package core.game.node.entity.npc

import core.game.node.entity.Entity
import core.game.node.item.Item

/** Recording behaviour; the method signature is the original's. */
open class NPCBehavior {
    open fun onDropTableRolled(self: NPC, killer: Entity, drops: ArrayList<Item>) {
        oracle.Support.rolled(self, killer, drops)
    }
}
