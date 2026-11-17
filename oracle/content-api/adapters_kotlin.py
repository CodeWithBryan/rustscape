"""Recording adapters for the Kotlin classes ContentAPI and the other core/api files reach."""

ADAPTERS = {
'core/game/world/GameWorld.kt': r'''package core.game.world

import core.game.system.task.PulseRunner

object GameWorld {
    @JvmStatic var ticks = 0
    @JvmStatic var settings: GameSettings? = null
    @JvmStatic var Pulser = PulseRunner()
}

class GameSettings(val isDevMode: Boolean)
''',
'core/game/world/repository/Repository.kt': r'''package core.game.world.repository

import core.game.node.entity.npc.NPC
import oracle.R

object Repository {
    val npcs: List<NPC> get() = R.list("Repository.getNpcs")
    fun findNPC(npcId: Int): NPC? = R.n("Repository.findNPC", npcId)
    fun sendNews(string: String, icon: Int = 12, color: String = "CC6600") { R.v("Repository.sendNews", string, icon, color) }
}
''',
'core/game/world/map/RegionManager.kt': r'''package core.game.world.map

import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.game.node.scenery.Scenery
import oracle.R

object RegionManager {
    fun forId(regionId: Int): Region = Region(regionId)
    fun getObject(l: Location): Scenery? = R.n("RegionManager.getObject", l)
    fun getObject(x: Int, y: Int, z: Int): Scenery? = R.n("RegionManager.getObject", x, y, z)
    fun getRegionChunk(l: Location): RegionChunk { R.ev("RegionManager.getRegionChunk", l); val c = RegionChunk(); R.fresh(c, "chunk"); return c }
    @JvmStatic fun getLocalPlayers(location: Location, distance: Int): List<Player> = R.list("RegionManager.getLocalPlayers", location, distance)
    @JvmStatic fun getLocalNPCs(location: Location, distance: Int): List<NPC> = R.list("RegionManager.getLocalNPCs", location, distance)
    @JvmStatic fun getLocalNPCs(location: Location): List<NPC> = R.list("RegionManager.getLocalNPCs", location)
    @JvmStatic fun getTeleportLocation(location: Location, radius: Int): Location = R.n("RegionManager.getTeleportLocation", location, radius)
}
''',
'core/game/system/timer/RSTimer.kt': r'''package core.game.system.timer

abstract class RSTimer {
    var key: String? = null
}
''',
'core/game/system/timer/TimerManager.kt': r'''package core.game.system.timer

import core.game.node.entity.Entity
import oracle.R

class TimerManager(val entity: Entity) {
    fun registerTimer(timer: RSTimer) { R.v("TimerManager.registerTimer", entity, timer.key) }
    inline fun <reified T : RSTimer> removeTimer() { R.v("TimerManager.removeTimer", entity, T::class.java.simpleName) }
    inline fun <reified T : RSTimer> getTimer(): T? = R.n<Any>("TimerManager.getTimer", entity, T::class.java.simpleName) as T?
    fun getTimer(identifier: String): RSTimer? = R.n("TimerManager.getTimer", entity, identifier)
    fun removeTimer(identifier: String) { R.v("TimerManager.removeTimer", entity, identifier) }
    fun removeTimer(timer: RSTimer) { R.v("TimerManager.removeTimer", entity, timer.key) }
}
''',
'core/game/system/timer/TimerRegistry.kt': r'''package core.game.system.timer

import oracle.R

object TimerRegistry {
    fun getTimerInstance(identifier: String, vararg args: Any): RSTimer? = R.n("TimerRegistry.getTimerInstance", identifier, args)
    inline fun <reified T> getTimerInstance(vararg args: Any): T? = R.n<Any>("TimerRegistry.getTimerInstance", T::class.java.simpleName, args) as T?
}
''',
'core/game/system/timer/impl/Poison.kt': r'''package core.game.system.timer.impl

import core.game.node.entity.Entity
import core.game.system.timer.RSTimer
import oracle.R

class Poison : RSTimer() {
    lateinit var damageSource: Entity
    private var oSeverity = 0
    var severity: Int
        get() = oSeverity
        set(value) { R.v("Poison.setSeverity", key, value); oSeverity = value }
    fun oracleSeverity(value: Int) { oSeverity = value }
    fun oracleSource(source: Entity?) { if (source != null) damageSource = source }
    fun oracleState(): List<Any?> = listOf(oSeverity, if (this::damageSource.isInitialized) R.value(damageSource) else null)
}
''',
'core/game/system/timer/impl/Immunities.kt': r'''package core.game.system.timer.impl

import core.game.system.timer.RSTimer

class PoisonImmunity : RSTimer()
class DragonFireImmunity : RSTimer()
''',
'core/game/interaction/ScriptProcessor.kt': r'''package core.game.interaction

import core.game.node.entity.Entity
import oracle.R

class ScriptProcessor(val entity: Entity) {
    var delay = 0
        set(value) { R.v("ScriptProcessor.setDelay", entity, value); field = value }
    var apRangeCalled = false
        set(value) { R.v("ScriptProcessor.setApRangeCalled", entity, value); field = value }
    var apRange = 10
        set(value) { R.v("ScriptProcessor.setApRange", entity, value); field = value }
    fun reset() { R.v("ScriptProcessor.reset", entity) }
    fun addToQueue(script: Script<*>, strength: QueueStrength) { R.script(entity, script, strength) }
    fun getActiveScript(): Script<*>? = R.n("ScriptProcessor.getActiveScript", entity)
}
''',
'core/game/interaction/InteractionListeners.kt': r'''package core.game.interaction

import core.game.node.Node
import core.game.node.entity.player.Player
import oracle.R

object InteractionListeners {
    fun run(id: Int, player: Player, node: Node, isEquip: Boolean): Boolean = R.b("InteractionListeners.run", id, player, node, isEquip)
}
''',
'core/game/node/entity/combat/CombatPulse.kt': r'''package core.game.node.entity.combat

class CombatPulse(val owner: Any?) {
    fun delayNextAttack(ticks: Int) { oracle.R.v("CombatPulse.delayNextAttack", owner, ticks) }
}
''',
'core/game/node/entity/combat/CombatSwingHandler.kt': r'''package core.game.node.entity.combat

import core.game.node.Node

abstract class CombatSwingHandler {
    companion object {
        fun isProjectileClipped(entity: Node, victim: Node?, checkClose: Boolean): Boolean = oracle.R.b("CombatSwingHandler.isProjectileClipped", entity, victim, checkClose)
    }
}
''',
'core/game/node/entity/player/link/audio/Audio.kt': r'''package core.game.node.entity.player.link.audio

class Audio @JvmOverloads constructor(val id: Int, val delay: Int = 0, val loops: Int = 1, val radius: Int = defaultAudioRadius) : oracle.R.Describable {
    companion object {
        const val defaultAudioRadius = 8
    }
    override fun describe(): Any = listOf("Audio", id, delay, loops, radius)
}
''',
'core/game/world/update/UpdateMasks.kt': r'''package core.game.world.update

import core.game.world.update.flag.EntityFlag

class UpdateMasks(val owner: Any?) {
    fun register(flag: EntityFlag, context: Any?, sync: Boolean = false): Boolean = oracle.R.b("UpdateMasks.register", owner, flag, context, sync)
}
''',
'core/game/world/update/flag/EntityFlag.kt': r'''package core.game.world.update.flag

enum class EntityFlag { ForceMove }
''',
'core/game/dialogue/DialogueFile.kt': r'''package core.game.dialogue

open class DialogueFile
''',
'core/game/dialogue/SkillDialogueHandler.kt': r'''package core.game.dialogue

import core.game.node.entity.player.Player
import core.game.node.item.Item

open class SkillDialogueHandler(val player: Player, val type: SkillDialogue?, vararg val items: Item) {
    fun open() { oracle.R.handler("SkillDialogueHandler.open", this, player, type, items) }
    open fun create(amount: Int, index: Int) {}
    open fun getAll(index: Int): Int = 0
    enum class SkillDialogue { ONE_OPTION, MAKE_SET_ONE_OPTION, TWO_OPTION, MAKE_SET_TWO_OPTION, THREE_OPTION, MAKE_SET_THREE_OPTION, FOUR_OPTION, FIVE_OPTION }
}
''',
'core/game/activity/Cutscene.kt': r'''package core.game.activity

open class Cutscene {
    companion object {
        const val ATTRIBUTE_CUTSCENE = "cutscene"
        const val ATTRIBUTE_CUTSCENE_STAGE = "cutscene:stage"
    }
}
''',
'core/game/system/config/ItemConfigParser.kt': r'''package core.game.system.config

class ItemConfigParser {
    companion object {
        const val EQUIP_SLOT = "equipment_slot"
        const val RARE_ITEM = "rare_item"
    }
}
''',
'core/game/system/config/ServerConfigParser.kt': r'''package core.game.system.config

import com.moandjiezana.toml.Toml

object ServerConfigParser {
    var tomlData: Toml? = null
}
''',
'com/moandjiezana/toml/Toml.kt': r'''package com.moandjiezana.toml

class Toml {
    init { oracle.R.v("Toml"); oracle.R.fresh(this, "toml") }
}
''',
'core/ServerConstants.kt': r'''package core

class ServerConstants {
    companion object {
        var MAX_PATHFIND_DISTANCE = 25
        var DATA_PATH: String? = null
    }
}
''',
'core/game/ge/GrandExchangeRecords.kt': r'''package core.game.ge

import core.game.node.entity.player.Player
import core.game.node.item.Item
import oracle.R

class OfferRecord(val uid: Long, val slot: Int)
class GrandExchangeOffer { var withdraw = arrayOfNulls<Item>(2) }

class GrandExchangeRecords(private val player: Player? = null) {
    var offerRecords = arrayOfNulls<OfferRecord>(6)
    fun getOffer(record: OfferRecord?): GrandExchangeOffer? = R.n("GrandExchangeRecords.getOffer", player, record)
    fun openCollectionBox() { R.v("GrandExchangeRecords.openCollectionBox", player) }
    companion object {
        @JvmStatic fun getInstance(player: Player? = null): GrandExchangeRecords = R.records(player)
    }
}
''',
'core/game/shops/Shops.kt': r'''package core.game.shops

import core.game.node.entity.player.Player

class Shop {
    fun openFor(player: Player) { oracle.R.v("Shop.openFor", this, player) }
}

class Shops {
    companion object {
        @JvmStatic val shopsByNpc = HashMap<Int, Shop>()
    }
}
''',
'content/global/handlers/iface/ge/StockMarket.kt': r'''package content.global.handlers.iface.ge

import core.game.node.entity.player.Player

class StockMarket {
    companion object {
        fun openFor(player: Player) { oracle.R.v("StockMarket.openFor", player) }
    }
}
''',
'content/global/skill/slayer/SlayerManager.kt': r'''package content.global.skill.slayer

import core.game.node.entity.player.Player
import oracle.R

class SlayerFlags(val owner: Any?) {
    val taskFlags: Int get() = R.i("SlayerFlags.getTaskFlags", owner)
}

class SlayerManager(val player: Player? = null) {
    val flags: SlayerFlags = SlayerFlags(player)
    val taskName: String get() = R.s("SlayerManager.getTaskName", player) as String
    var activeTask: Tasks?
        get() = R.s("SlayerManager.getActiveTask", player)?.let { Tasks.valueOf(it) }
        set(value) {}
    var master: Master?
        get() = R.s("SlayerManager.getMaster", player)?.let { Master.valueOf(it) }
        set(value) {}
    fun hasTask(): Boolean = R.b("SlayerManager.hasTask", player)
    var amount: Int
        get() = R.i("SlayerManager.getAmount", player)
        set(value) {}
    companion object {
        @JvmStatic fun getInstance(player: Player): SlayerManager = R.slayer(player)
    }
}
''',
'content/global/skill/slayer/SlayerEquipmentFlags.kt': r'''package content.global.skill.slayer

import core.game.node.entity.player.Player

object SlayerEquipmentFlags {
    fun updateFlags(player: Player) { oracle.R.v("SlayerEquipmentFlags.updateFlags", player) }
    fun isSlayerEq(item: Int): Boolean = oracle.R.b("SlayerEquipmentFlags.isSlayerEq", item)
}
''',
'content/global/skill/construction/decoration/pohstorage/StorageInterface.kt': r'''package content.global.skill.construction.decoration.pohstorage

import core.game.node.entity.player.Player
import oracle.R

class Storable(val bitIndex: Int)

class StorageContainer(val key: String) {
    fun addItem(id: Int) { R.v("StorageContainer.addItem", key, id) }
    fun contains(id: Int): Boolean = R.b("StorageContainer.contains", key, id)
}

class StorageState(val player: Player) {
    fun getContainer(type: StorableFamily): StorageContainer = StorageContainer(type.name)
}

class StorageInterface {
    companion object {
        fun getStorageContainer(player: Player, type: StorableFamily): StorageContainer = R.ev("StorageInterface.getStorageContainer", player, type).let { StorageContainer(type.name) }
        fun findStorableByDisplayId(id: Int): Storable? = (R.ev("StorageInterface.findStorableByDisplayId", id) as Number?)?.let { Storable(it.toInt()) }
    }
    object BookcaseBitfields {
        data class BookcaseBits(var low: Long, var high: Long)
        fun get(player: Player): BookcaseBits = R.bits(R.ev("BookcaseBitfields.get", player))
        fun set(player: Player, bits: BookcaseBits) { R.v("BookcaseBitfields.set", player, listOf(bits.low, bits.high)) }
        fun isBitSet(bits: BookcaseBits, bitIndex: Int): Boolean = R.b("BookcaseBitfields.isBitSet", listOf(bits.low, bits.high), bitIndex)
        fun setBit(bits: BookcaseBits, bitIndex: Int): BookcaseBits = R.bits(R.ev("BookcaseBitfields.setBit", listOf(bits.low, bits.high), bitIndex))
    }
}
''',
'core/game/node/entity/player/info/PlayerMonitor.kt': r'''package core.game.node.entity.player.info

import core.game.node.entity.player.Player

object PlayerMonitor {
    @JvmStatic fun log(player: Player, type: LogType, details: String) { oracle.R.v("PlayerMonitor.log", player, type, details) }
}

enum class LogType(val token: String) {
    DUPE_ALERT("dupe_warning")
}
''',
'core/tools/SystemLogger.kt': r'''package core.tools

object SystemLogger {
    fun processLogEntry(clazz: Class<*>, log: Log, message: String) { oracle.R.v("SystemLogger.processLogEntry", clazz.simpleName, log, oracle.R.frames(message)) }
    fun logStartup(message: String) { oracle.R.v("SystemLogger.logStartup", message) }
}

enum class Log {
    FINE,
    INFO,
    WARN,
    ERR,
    DEBUG
}
''',
'core/game/requirement/Requirement.kt': r'''package core.game.requirement

import content.data.Quests
import core.game.node.entity.player.Player
import core.game.node.entity.skill.Skills

interface Requirement {
    fun evaluate(player: Player): Pair<Boolean, List<Requirement>>
}

open class SkillReq(val skillId: Int, val level: Int, val soft: Boolean = false) : Requirement, oracle.R.Describable {
    override fun describe(): Any = listOf("SkillReq", skillId, level, soft)
    override fun evaluate(player: Player): Pair<Boolean, List<Requirement>> = oracle.R.evaluate(this, player)
}

open class QuestReq(val questReq: QuestRequirements, val stageRequired: Int = 100) : Requirement, oracle.R.Describable {
    override fun describe(): Any = listOf("QuestReq", questReq.name, stageRequired)
    override fun evaluate(player: Player): Pair<Boolean, List<Requirement>> = oracle.R.evaluate(this, player)
}

open class QPReq(val amount: Int) : Requirement, oracle.R.Describable {
    override fun describe(): Any = listOf("QPReq", amount)
    override fun evaluate(player: Player): Pair<Boolean, List<Requirement>> = oracle.R.evaluate(this, player)
}

open class QPCumulative(val amount: Int) : Requirement, oracle.R.Describable {
    override fun describe(): Any = listOf("QPCumulative", amount)
    override fun evaluate(player: Player): Pair<Boolean, List<Requirement>> = oracle.R.evaluate(this, player)
}

/* ORIGINAL_QUEST_REQUIREMENTS */
''',
'core/api/utils/PlayerStatsCounter.kt': r'''package core.api.utils

import core.game.node.entity.player.Player
import core.game.node.item.Item

class PlayerStatsCounter {
    companion object {
        @JvmStatic fun incrementRareDrop(player: Player, item: Item) { oracle.R.v("PlayerStatsCounter.incrementRareDrop", player, item) }
    }
}
''',
'core/game/system/command/Command.kt': r'''package core.game.system.command

import core.game.node.entity.player.Player

enum class Privilege { STANDARD, MODERATOR, ADMIN }

class Command(val name: String, val privilege: Privilege, val usage: String, val description: String, val handle: (Player, Array<String>) -> Unit) : oracle.R.Describable {
    override fun describe(): Any = listOf("Command", name, privilege.name, usage, description, oracle.R.value(handle))
}

object CommandMapping {
    fun register(command: Command) { oracle.R.v("CommandMapping.register", command) }
}
''',
'core/game/event/Events.kt': r'''package core.game.event

class ResourceProducedEvent
class NPCKillEvent
class BoneBuryEvent
class TeleportEvent
class LitFireEvent
class LitLightSourceEvent
class InteractionEvent
class ButtonClickEvent
class DialogueOpenEvent
class DialogueOptionSelectionEvent
class DialogueCloseEvent
class UseWithEvent
class SelfDeathEvent
class TickEvent
class PickUpEvent
class InterfaceOpenEvent
class InterfaceCloseEvent
class AttributeSetEvent
class AttributeRemoveEvent
class SpellCastEvent
class SpellbookChangeEvent
class ItemAlchemizationEvent
class ItemEquipEvent
class ItemUnequipEvent
class ItemShopPurchaseEvent
class ItemShopSellEvent
class JobAssignmentEvent
class FairyRingDialEvent
class VarbitUpdateEvent
class DynamicSkillLevelChangeEvent
class SummoningPointsRechargeEvent
class PrayerPointsRechargeEvent
class XPGainEvent
class PrayerActivatedEvent
class PrayerDeactivatedEvent
''',
}
