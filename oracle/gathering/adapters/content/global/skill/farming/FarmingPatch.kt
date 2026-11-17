package content.global.skill.farming

import core.game.node.entity.player.Player
import core.game.node.scenery.Scenery
import oracle.Support

class FarmingPatch(val varbit: Int) {
    fun getPatchFor(player: Player, addPatch: Boolean = true): Patch {
        Support.emit("farmingPatch.getPatchFor", null, varbit, addPatch)
        return Patch()
    }
    companion object {
        @JvmStatic fun forObject(obj: Scenery): FarmingPatch? {
            val varbit = if (Support.patchVarbit < 0) null else Support.patchVarbit
            Support.emit("farmingPatch.forObject", varbit, obj.ref)
            return if (varbit == null) null else FarmingPatch(varbit)
        }
    }
}
