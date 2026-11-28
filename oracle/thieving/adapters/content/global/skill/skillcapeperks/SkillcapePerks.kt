package content.global.skill.skillcapeperks

import core.game.node.entity.player.Player
import oracle.W

/** Adapter: the declared answer for one perk. */
enum class SkillcapePerks {
    SMOOTH_HANDS;

    companion object {
        @JvmStatic
        fun isActive(perk: SkillcapePerks, player: Player): Boolean {
            W.emit("isActive", perk.name)
            return W.flag("smoothHands")
        }
    }
}
