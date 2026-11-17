package core.net.packet.`in`

import core.game.node.entity.player.Player

/** The one Packet subclass this oracle decodes into. */
sealed class Packet {
    class GESetOfferItem(val player: Player, val itemId: Int) : Packet()
}