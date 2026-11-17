// Kotlin adapters for the dialogue DSL oracle. generate.py splits this bundle at
// each "//// path" marker and replaces every /* ORIGINAL_* */ marker with text
// read from the pinned reference at generation time: ContentAPI's face,
// openDialogue and both sendInputDialogue overloads, SecondaryBankAccount-
// ActivationResult and the ContentAPI class line; Event.DialogueClosed; and
// BankerNPC's ODOVACAR_FEE, canAccessBank and BankerDialogueLabellerFile.
// The bank/quest/configuration members below them are recording stand-ins.
//// core/api/ContentAPI.kt
package core.api

import core.game.dialogue.DialogueFile
import core.game.dialogue.SkillDialogueHandler
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.IronmanMode
import core.game.node.item.Item
import core.game.world.map.Location
import core.tools.Log
import oracle.Rec

/* ORIGINAL_FACE */

/* ORIGINAL_OPEN_DIALOGUE */

/* ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC */

/* ORIGINAL_SEND_INPUT_DIALOGUE_TYPED */

/* ORIGINAL_ACTIVATION_RESULT */

fun log(origin: Class<*>, type: Log, message: String) { Rec.emit("log", origin.simpleName, type.name, Rec.units(message)) }
fun sendMessage(player: Player, message: String) { Rec.emit("apiMessage", player.oid, Rec.units(message)) }
fun removeItem(player: Player, item: Item): Boolean { Rec.emit("removeItem", player.oid, item.ref); return oracle.Oracle.flag("remove") }
fun hasSealOfPassage(player: Player): Boolean { Rec.emit("seal", player.oid); return oracle.Oracle.flag("seal") }
fun hasIronmanRestriction(player: Player, restriction: IronmanMode): Boolean { Rec.emit("ironman", player.oid, restriction.name); return oracle.Oracle.flag("uim") }
fun hasAwaitingGrandExchangeCollections(player: Player): Boolean { Rec.emit("geAwaiting", player.oid); return oracle.Oracle.flag("ge") }
fun openBankAccount(player: Player) { Rec.emit("openBank", player.oid) }
fun openBankPinSettings(player: Player) { Rec.emit("pinSettings", player.oid) }
fun openGrandExchangeCollectionBox(player: Player) { Rec.emit("collectionBox", player.oid) }
fun activateSecondaryBankAccount(player: Player): SecondaryBankAccountActivationResult { Rec.emit("activate", player.oid); return SecondaryBankAccountActivationResult.valueOf(oracle.Oracle.text("activation")) }
fun hasActivatedSecondaryBankAccount(player: Player): Boolean { Rec.emit("activated", player.oid); return oracle.Oracle.flag("activated") }
fun isUsingSecondaryBankAccount(player: Player): Boolean { Rec.emit("usingSecondary", player.oid); return oracle.Oracle.flag("secondary") }
fun toggleBankAccount(player: Player) { Rec.emit("toggleBank", player.oid); oracle.Oracle.toggle("secondary") }

/* ORIGINAL_CONTENT_API_CLASS */
//// core/api/Event.kt
package core.api

import core.game.event.*

object Event {
    /* ORIGINAL_EVENT_DIALOGUE_CLOSED */
}
//// core/ServerConstants.kt
package core

class ServerConstants {
    companion object {
        @JvmField
        var SECOND_BANK = false
    }
}
//// content/data/Quests.kt
package content.data

enum class Quests(val questName: String) {
    COOKS_ASSISTANT("Cook's Assistant"),
    DRAGON_SLAYER("Dragon Slayer"),
}
//// content/global/handlers/npc/BankerNPC.kt
package content.global.handlers.npc

import core.ServerConstants
import core.api.*
import core.game.dialogue.ChatAnim
import core.game.dialogue.DialogueLabeller
import core.game.dialogue.DialogueOption
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.IronmanMode
import core.game.node.item.Item
import org.rs09.consts.Items
import org.rs09.consts.NPCs

class BankerNPC {
    companion object {
        /* ORIGINAL_ODOVACAR_FEE */

        /* ORIGINAL_CAN_ACCESS_BANK */
    }

    /* ORIGINAL_BANKER_DIALOGUE */
}
