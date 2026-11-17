package core.integrations.discord

/** Discord.postNewOffer: a recorded service. */
object Discord {
    @JvmStatic
    fun postNewOffer(isSale: Boolean, itemId: Int, value: Int, qty: Int, user: String) {
        oracle.Support.emit("discord", isSale, itemId, value, qty, user)
    }
}