package content.global.handlers.item.equipment.fistofguthixgloves

import core.game.node.entity.player.Player

/** Recording service: glove charges and equipment removal are outside this oracle. */
class FOGGlovesManager {
    companion object {
        @JvmStatic
        fun updateCharges(player: Player, charges: Int = 1): Int {
            return oracle.Support.fogCharges(player, charges)
        }
    }
}
