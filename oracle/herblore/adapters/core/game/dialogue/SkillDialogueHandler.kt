package core.game.dialogue

import core.game.node.entity.player.Player
import core.game.node.item.Item
import oracle.W

/**
 * Adapter: the constructor, `open` (recorded with the type and items), the
 * two overridable methods and the SkillDialogue constants with the pinned
 * forLength spliced verbatim. Display, buttons and amounts are not here.
 */
open class SkillDialogueHandler(
        val player: Player,
        val type: SkillDialogue?,
        vararg val items: Item) {

    fun open() {
        W.open(this, type?.name, items.map { it.id })
    }

    open fun create(amount: Int, index: Int) {}

    open fun getAll(index: Int): Int {
        return W.amount(items[0].id)
    }

    enum class SkillDialogue(val interfaceId: Int, private val baseButton: Int, private val length: Int) {
        /* SKILL_DIALOGUE_CONSTANTS */;

        companion object {
            /* FOR_LENGTH */
        }
    }
}
