package core.api

import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.tools.Log
import java.io.PrintWriter
import java.io.StringWriter

// Recording services for systems outside this oracle. exceptionToString is the
// original text, inserted at generation time.
fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.log(origin, type, message)
}

fun shouldRemoveNothings(player: Player): Boolean {
    return oracle.Support.removesNothings(player)
}

fun inEquipment(player: Player, id: Int, amount: Int = 1): Boolean {
    return oracle.Support.inEquipment(player, id, amount)
}

fun announceIfRare(player: Player, item: Item) {
    throw UnsupportedOperationException("createDrop is outside the drop-table oracle")
}

/* ORIGINAL_EXCEPTION_TO_STRING */
