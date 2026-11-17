package content.global.handlers.item.equipment.fistofguthixgloves

import core.game.node.entity.player.Player
import oracle.Support

class FOGGlovesManager {
    companion object {
        @JvmStatic fun updateCharges(player: Player, charges: Int = 1): Int {
            Support.emit("fogGloves.updateCharges", null)
            return 0
        }
    }
}