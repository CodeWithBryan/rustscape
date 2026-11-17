package content.global.skill.skillcapeperks

import core.game.node.entity.player.Player
import oracle.Support

enum class SkillcapePerks {
    NEST_HUNTER, PRECISION_MINER, GREAT_AIM, DAMAGE_SPONG;
    companion object {
        @JvmStatic fun isActive(perk: SkillcapePerks, player: Player): Boolean =
            Support.emit("skillcapePerks.isActive", Support.perks.contains(perk.name), perk.name)
    }
}
