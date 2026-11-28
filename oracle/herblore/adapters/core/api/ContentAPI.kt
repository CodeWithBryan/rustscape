package core.api

import content.data.Quests
import core.game.dialogue.SkillDialogueHandler
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import oracle.W

// Adapters: each answers from the case's declared state or records its call.
// requireQuest, isQuestComplete, sendSkillDialogue and SkillDialogueBuilder are the pinned text,
// spliced verbatim over these adapters.

fun lock(entity: Entity, duration: Int) { W.emit("lock", duration) }
fun getQuestStage(player: Player, quest: Quests): Int = W.questStage(quest.name)
fun hasRequirement(player: Player, quest: Quests, message: Boolean = true): Boolean {
    W.emit("hasRequirement", quest.name, message)
    return W.listed("requirements", quest.name)
}
fun getDynLevel(entity: Entity, skill: Int): Int { W.emit("level", skill); return W.number("level") }
fun sendMessage(player: Player, message: String) { W.emit("sendMessage", message) }
fun replaceSlot(player: Player, slot: Int, item: Item, currentItem: Item? = null): Item? {
    W.emit("replaceSlot", slot, item.id, item.amount, currentItem?.id)
    return null
}
fun rewardXP(player: Player, skill: Int, amount: Double) { W.emit("rewardXP", skill, W.bits(amount)) }
@JvmOverloads
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1) { W.emit("audio", id, delay, loops) }
fun amountInInventory(player: Player, id: Int): Int { W.emit("amount", id); return W.amount(id) }

/* IS_QUEST_COMPLETE */

/* REQUIRE_QUEST */

/* SEND_SKILL_DIALOGUE */

/* SKILL_DIALOGUE_BUILDER */
