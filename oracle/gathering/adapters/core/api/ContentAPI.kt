package core.api

import core.game.interaction.QueueStrength
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.game.world.map.zone.ZoneBorders
import core.game.world.update.flag.context.Animation
import oracle.Support

// Recording stand-ins for the ContentAPI functions the gathering sources call.
// Each is one named service; results come from the case's declared state.
fun finishedMoving(entity: Entity): Boolean = Support.emit("api.finishedMoving", Support.clocks[0] < Support.ticks)
fun animationFinished(entity: Entity): Boolean = Support.emit("api.animationFinished", Support.clocks[1] < Support.ticks)
fun clockReady(entity: Entity, clock: Int): Boolean = Support.emit("api.clockReady", Support.clocks[clock] <= Support.ticks, clock)
fun delayClock(entity: Entity, clock: Int, ticks: Int, delayScript: Boolean = false): Boolean {
    Support.emit("api.delayClock", false, clock, ticks, delayScript)
    Support.clocks[clock] = Support.ticks + ticks
    return false
}
fun delayScript(entity: Entity, ticks: Int): Boolean {
    Support.emit("api.delayScript", false, ticks)
    if (Support.hasScript) Support.scriptNext = Support.ticks + ticks
    return false
}
fun keepRunning(entity: Entity): Boolean {
    Support.emit("api.keepRunning", false)
    if (Support.hasScript) Support.scriptNext = Support.ticks + 1
    return false
}
fun clearScripts(entity: Entity): Boolean {
    Support.emit("api.clearScripts", true)
    Support.hasScript = false
    return true
}
fun restartScript(entity: Entity): Boolean {
    val persistent = Support.hasScript && Support.scriptPersist
    Support.emit("api.restartScript", true)
    if (!persistent) Support.hasScript = false
    return true
}
fun stopExecuting(entity: Entity): Boolean = Support.emit("api.stopExecuting", true)
fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    Support.scripts.add(script)
    Support.emit("api.queueScript", null, delay, strength.name, persist, Support.scripts.size - 1)
}
fun sendMessage(player: Player, message: String) { Support.emit("api.sendMessage", null, player.ref, message) }
fun sendDialogue(player: Player, message: String) { Support.emit("api.sendDialogue", null, message) }
fun addItem(player: Player, id: Int, amount: Int = 1, container: Container = Container.INVENTORY): Boolean =
    Support.emit("api.addItem", Support.add(id, amount), id, amount, container.name)
fun addItemOrDrop(player: Player, id: Int, amount: Int = 1) {
    Support.emit("api.addItemOrDrop", null, id, amount)
    Support.add(id, amount)
}
fun freeSlots(player: Player): Int = Support.emit("api.freeSlots", Support.freeSlots())
fun <T> getAttribute(entity: Entity, attribute: String, default: T): T {
    @Suppress("UNCHECKED_CAST") val v = Support.attributes[attribute.replace("/save:", "")] as T?
    return Support.emit("api.getAttribute", v ?: default, attribute, default)
}
fun <T> setAttribute(entity: Entity, attribute: String, value: T) {
    Support.emit("api.setAttribute", null, attribute, value)
    Support.attributes[attribute.removePrefix("/save:")] = value
}
fun playAudio(player: Player, id: Int) { Support.emit("api.playAudio", null, player.ref, id) }
fun openDialogue(player: Player, dialogue: Any, vararg args: Any) {
    val npc = args[0] as core.game.node.entity.npc.NPC
    Support.emit("api.openDialogue", null, dialogue.javaClass.simpleName, npc.rawId, npc.rawLocation)
}
fun getQuestStage(player: Player, quest: content.data.Quests): Int = Support.emit("api.getQuestStage", Support.questStage, quest.name)
inline fun <reified T> getOrStartTimer(entity: Entity, vararg args: Any): T {
    Support.emit("api.getOrStartTimer", null, T::class.java.simpleName)
    return T::class.java.getDeclaredConstructor().newInstance()
}
inline fun <reified T> hasTimerActive(entity: Entity): Boolean =
    Support.emit("api.hasTimerActive", Support.timers.contains(T::class.java.simpleName), T::class.java.simpleName)
fun getDynLevel(entity: Entity, skill: Int): Int = Support.emit("api.getDynLevel", Support.dynamic[skill], skill)
fun getFamiliarBoost(player: Player, skill: Int): Int = Support.emit("api.getFamiliarBoost", Support.boosts[skill], skill)
fun rewardXP(player: Player, skill: Int, amount: Double) { Support.emit("api.rewardXP", null, skill, amount) }
fun getItemName(id: Int): String = Support.emit("api.getItemName", Support.name(id), id)
fun getItemFromEquipment(player: Player, slot: EquipmentSlot): Item? {
    val id = Support.equipment[slot.ordinal]
    Support.emit("api.getItemFromEquipment", id, slot.name)
    return if (id == null) null else Item(id)
}
fun inEquipment(player: Player, id: Int, amount: Int = 1): Boolean =
    Support.emit("api.inEquipment", Support.equipment.containsValue(id), id, amount)
fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    val id = when (item) {
        is Item -> item.id
        is Int -> item
        else -> throw IllegalStateException("Invalid value passed for item")
    }
    if (container == Container.INVENTORY) {
        // Simple slot model: one unit of the id leaves the declared inventory.
        val held = Support.amount(id) >= 1
        Support.emit("api.removeItem", held, id, container.name)
        if (held) Support.take(id)
        return held
    }
    val slot = Support.equipment.entries.firstOrNull { it.value == id }?.key
    val removed = container == Container.EQUIPMENT && slot != null && Support.addResult
    Support.emit("api.removeItem", removed, id, container.name)
    if (removed) Support.equipment.remove(slot)
    return removed
}
fun getUsedOption(player: Player): String = Support.emit("api.getUsedOption", Support.usedOption)
fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean = Support.emit("api.inInventory", Support.amount(id) >= amount, id, amount)
fun hasLevelDyn(player: Player, skill: Int, level: Int): Boolean = Support.emit("api.hasLevelDyn", Support.dynamic[skill] >= level, skill, level)
fun hasSpaceFor(player: Player, item: Item): Boolean = Support.emit("api.hasSpaceFor", Support.space(item.id, item.amount), item.id, item.amount)
fun Int.asItem(): Item = Item(this)
fun inBorders(entity: Entity, borders: ZoneBorders): Boolean {
    val l = Support.location
    val inside = minOf(borders.x1, borders.x2) <= l.x && minOf(borders.y1, borders.y2) <= l.y && maxOf(borders.x1, borders.x2) >= l.x && maxOf(borders.y1, borders.y2) >= l.y
    return Support.emit("api.inBorders", inside, borders.x1, borders.y1, borders.x2, borders.y2)
}
fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) {
    val a = when (anim) {
        is Int -> Animation(anim)
        is Animation -> anim
        else -> throw IllegalStateException("Invalid value passed for anim")
    }
    Support.emit("api.animate", null, a.id, a.priority?.name, forced)
}
