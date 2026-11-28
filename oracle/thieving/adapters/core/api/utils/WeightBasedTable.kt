package core.api.utils

import core.game.node.entity.player.Player
import core.game.node.item.Item
import oracle.W
import org.rs09.consts.Items

/**
 * Adapter: keeps the entries a table is given, in order, and records
 * canRoll, excluding and roll; their answers are the case's declared ones.
 * The original table (positions, weights, slots) is the drop-tables corpus's.
 */
open class WeightBasedTable {
    val entries = ArrayList<WeightedItem>()
    var owner: String? = null
    var excluded = false

    fun add(element: WeightedItem): Boolean = entries.add(element)

    fun roll(): ArrayList<Item> {
        W.emit("roll", owner, excluded)
        return W.loot()
    }

    fun canRoll(player: Player): Boolean {
        W.emit("canRoll", owner)
        return W.flag("canRoll")
    }

    fun insertEasyClue(weight: Double): WeightBasedTable {
        this.add(WeightedItem(SLOT_CLUE_EASY, 1, 1, weight, false))
        return this
    }

    fun excluding(vararg ids: Int): WeightBasedTable {
        W.emit("excluding", owner, ids.toList())
        val filtered = WeightBasedTable()
        filtered.owner = owner
        filtered.excluded = true
        return filtered
    }

    companion object {
        @JvmStatic
        fun create(vararg items: WeightedItem): WeightBasedTable {
            val table = WeightBasedTable()
            items.forEach { table.add(it) }
            return table
        }

        /* CLUE_SLOTS */
    }
}
