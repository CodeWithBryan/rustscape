package core.api

import content.data.Quests
import content.global.skill.slayer.SlayerFlags
import core.game.interaction.QueueStrength
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.game.world.update.flag.context.Animation
import oracle.W

// Adapters: each answers from the case's declared state or records its call.
// isQuestStarted, isQuestComplete and removeItemsIfPlayerHasEnough are the pinned functions,
// spliced verbatim over these adapters.

fun getDynLevel(entity: Entity, skill: Int): Int = W.level(skill)
fun sendDialogue(player: Player, message: String) { W.emit("dialogue", message) }
fun sendMessage(player: Player, message: String) { W.emit("message", message) }
fun getQuestStage(player: Player, quest: Quests): Int = W.questStage(quest.name)
fun amountInInventory(player: Player, id: Int): Int = W.amount(id)
fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    item ?: return false
    val it = when (item) {
        is Item -> item
        is Int -> Item(item)
        else -> throw IllegalStateException("Invalid value passed for item")
    }
    return W.remove(it.id, it.amount)
}
fun addItem(player: Player, id: Int, amount: Int = 1, container: Container = Container.INVENTORY): Boolean = W.add(id, amount)
fun rewardXP(player: Player, skill: Int, amount: Double) { W.xp(skill, amount) }
fun hasSpaceFor(player: Player, item: Item): Boolean = W.flag("space")
fun <T> getAttribute(entity: Entity, attribute: String, default: T): T = W.attribute(attribute, default)
fun setAttribute(entity: Entity, attribute: String, value: Any?) { W.setAttribute(attribute, value) }
fun keepRunning(entity: Entity): Boolean { W.emit("keepRunning"); return false }
fun stopExecuting(entity: Entity): Boolean { W.emit("stopExecuting"); return true }
fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    W.queue(delay, strength.name, persist, script)
}
fun delayClock(entity: Entity, clock: Int, ticks: Int, delayScript: Boolean = false): Boolean {
    W.emit("delayClock", clock, ticks, delayScript)
    return false
}
fun clockReady(entity: Entity, clock: Int): Boolean = W.flag("clockReady")
fun delayScript(entity: Entity, ticks: Int): Boolean {
    W.emit("delayScript", ticks)
    return false
}
fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) {
    val animation = when (anim) {
        is Int -> Animation(anim)
        is Animation -> anim
        else -> throw IllegalStateException("Invalid value passed for anim")
    }
    W.emit("animate", animation.id)
}
fun getSlayerFlags(player: Player): SlayerFlags = SlayerFlags()
fun Int.asItem(): Item = Item(this)

/* IS_QUEST_STARTED */

/* IS_QUEST_COMPLETE */

/* REMOVE_ITEMS_IF_PLAYER_HAS_ENOUGH */
