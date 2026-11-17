package core.net.packet

import content.global.handlers.iface.ge.StockMarket
import core.api.sendMessage
import core.game.ge.GrandExchange.Companion.getOfferStats
import core.game.ge.GrandExchange.Companion.getRecommendedPrice
import core.game.ge.GrandExchangeOffer
import core.game.ge.PriceIndex
import core.net.packet.`in`.Packet

/** PacketProcessor.process with only its GESetOfferItem branch (original text). */
object PacketProcessor {
    @JvmStatic
    fun process(pkt: Packet) {
        when (pkt) {
            /* ORIGINAL_GE_SET_OFFER_ITEM */
        }
    }
}