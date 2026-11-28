@file:JvmName("ContentAPIKt")
package core.api

import content.data.Quests
import core.game.node.entity.Entity
import core.game.node.entity.combat.ImpactHandler
import core.game.node.entity.player.Player
import oracle.W

/** Adapter: records the call. */
@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0) { W.emit("playAudio", id, delay) }

/** Adapter: records the call; the dialogue it opens is not covered. */
fun sendDialogue(player: Player, message: String) { W.emit("dialogue", message) }

/** Adapter: the declared answer. */
fun isQuestComplete(player: Player, quest: Quests): Boolean {
    W.emit("questComplete", quest.questName)
    return W.flag("questComplete")
}

/** Adapter: records the call; what a stun does is not covered. */
fun stun(entity: Entity, ticks: Int, sendMessage: Boolean) { W.emit("stun", ticks, sendMessage) }

/** Adapter: records the call; the hit and its sound are not covered. */
@JvmOverloads
fun impact(entity: Entity, amount: Int, type: ImpactHandler.HitsplatType = ImpactHandler.HitsplatType.NORMAL) {
    W.emit("impact", amount, type.name)
}
