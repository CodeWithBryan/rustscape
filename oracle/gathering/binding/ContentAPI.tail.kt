// ---- binding corpus -------------------------------------------------------
// Still a required service: amountInEquipment needs the item configuration's equip slot.
// The reply is read from the real equipment container.
fun inEquipment(player: Player, id: Int, amount: Int = 1): Boolean =
    Support.emit("api.inEquipment", player.equipment.toArray().any { it != null && it.id == id && it.amount >= amount }, id, amount)
fun getWorldTicks(): Int = Support.emit("api.getWorldTicks", Support.ticks)
fun log(origin: Any?, level: Int, message: String) { throw AssertionError(message) }

// The pinned function texts, spliced at generation time.
/* PINNED_CONTENT_API */