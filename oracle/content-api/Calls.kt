package oracle

import content.data.Quests
import core.api.*
import core.api.regionspec.*
import core.api.regionspec.contracts.*
import core.api.utils.Vector
import core.game.dialogue.DialogueFile
import core.game.dialogue.FacialExpression
import core.game.diary.DiaryLevel
import core.game.interaction.QueueStrength
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.combat.ImpactHandler
import core.game.node.entity.impl.Animator
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.IronmanMode
import core.game.node.entity.player.link.TeleportManager
import core.game.node.entity.player.link.diary.DiaryType
import core.game.node.entity.player.link.emote.Emotes
import core.game.node.item.GroundItem
import core.game.node.item.Item
import core.game.node.scenery.Scenery
import core.game.requirement.QuestReq
import core.game.requirement.QuestRequirements
import core.game.system.task.Pulse
import core.game.world.map.Direction
import core.game.world.map.Location
import core.game.world.map.zone.MapZone
import core.game.world.map.zone.ZoneBorders
import core.game.world.update.flag.context.Animation
import core.game.world.update.flag.context.Graphics
import core.tools.Log

/**
 * Calls one ContentAPI (or other core/api) function named by the case, with its positional arguments decoded.
 * Overloads and defaults are separate keys, so the Kotlin compiler resolves each call as content code would.
 */
object Calls {
    private lateinit var args: List<Any?>

    private fun raw(i: Int): Any? = args[i]
    private fun I(i: Int): Int = (raw(i) as Number).toInt()
    private fun D(i: Int): Double = (raw(i) as Number).toDouble()
    private fun B(i: Int): Boolean = raw(i) as Boolean
    private fun S(i: Int): String = raw(i) as String
    private fun SN(i: Int): String? = raw(i) as String?
    private fun <T> node(i: Int): T = R.node(raw(i)) as T
    private fun P(i: Int): Player = node(i)
    private fun E(i: Int): Entity = node(i)
    private fun N(i: Int): Node = node(i)
    private fun NPC_(i: Int): NPC = node(i)
    private fun IT(i: Int): Item = node(i)
    private fun ITN(i: Int): Item? = raw(i)?.let { R.node(it) as Item }
    private fun SC(i: Int): Scenery = node(i)
    private fun L(i: Int): Location = R.node(raw(i)) as Location
    private fun LN(i: Int): Location? = raw(i)?.let { R.node(it) as Location }
    private fun INTS(i: Int): IntArray = (raw(i) as List<*>).map { (it as Number).toInt() }.toIntArray()
    private fun STRS(i: Int): Array<String> = (raw(i) as List<*>).map { it as String }.toTypedArray()
    private fun ITEMS(i: Int): Array<Item> = (raw(i) as List<*>).map { R.node(it) as Item }.toTypedArray()
    private fun ANYS(i: Int): Array<Any> = (raw(i) as List<*>).map { any(it)!! }.toTypedArray()
    private fun A(i: Int): Any? = any(raw(i))

    /** An argument of a generic or Any parameter: {"int":n}, {"node":k}, {"str":s}, {"bool":b}, {"anim":[id,prio]}, {"gfx":[id,h,d]}, {"long":n}, {"double":d}, {"fn":name}. */
    fun any(v: Any?): Any? {
        if (v == null) return null
        val m = v as Map<*, *>
        return when {
            m.containsKey("int") -> (m["int"] as Number).toInt()
            m.containsKey("node") -> R.node(m["node"])
            m.containsKey("str") -> m["str"] as String
            m.containsKey("bool") -> m["bool"] as Boolean
            m.containsKey("long") -> (m["long"] as Number).toLong()
            m.containsKey("double") -> (m["double"] as Number).toDouble()
            m.containsKey("loc") -> R.node(m)
            m.containsKey("anim") -> (m["anim"] as List<*>).let { Animation((it[0] as Number).toInt(), Animator.Priority.valueOf(it[1] as String)) }
            m.containsKey("gfx") -> (m["gfx"] as List<*>).let { Graphics((it[0] as Number).toInt(), (it[1] as Number).toInt(), (it[2] as Number).toInt()) }
            m.containsKey("fn") -> task(m["fn"] as String)
            m.containsKey("dialogueFile") -> DialogueFile().also { R.labels[it] = "file:" + m["dialogueFile"] }
            m.containsKey("action") -> action(m["action"] as String)
            else -> throw IllegalArgumentException("any $m")
        }
    }

    // ------------------------------------------------------------ content lambdas the cases hand ContentAPI
    fun <T : Any> named(name: String, f: T): T { R.labels[f] = "fn:$name"; return f }
    fun task(name: String): () -> Unit = named(name) { R.v("@lambda", name) }
    fun stage(name: String): (Int) -> Boolean = named(name) { s: Int -> R.b("@lambda", name, s) }
    fun playerHandler(name: String): (Player) -> Unit = named(name) { p: Player -> R.v("@lambda", name, p) }
    fun valueHandler(name: String): (Any) -> Unit = named(name) { v: Any -> R.v("@lambda", name, v) }
    fun select(name: String): (Int, Int) -> Unit = named(name) { a: Int, b: Int -> R.v("@lambda", name, a, b) }
    fun action(name: String): core.game.dialogue.DialogueAction = named(name, core.game.dialogue.DialogueAction { p, b -> R.v("@lambda", name, p, b) })
    fun command(name: String): (Player, Array<String>) -> Unit = named(name) { p: Player, a: Array<String> -> R.v("@lambda", name, p, a) }

    private fun pulse(name: String, delay: Int): Pulse = object : Pulse(delay) {
        override fun pulse(): Boolean = R.b("@lambda", name)
    }.also { R.labels[it] = "fn:$name" }

    private fun zone(name: String): MapZone = MapZone().also { it.key = name }

    private fun skillInit(spec: Map<*, *>): SkillDialogueBuilder.() -> Unit = {
        val items = spec["items"] as List<*>?
        val ids = spec["ids"] as List<*>?
        if (items != null) withItems(*items.map { R.node(it) as Item }.toTypedArray())
        if (ids != null) withItems(*ids.map { (it as Number).toInt() }.toIntArray())
        if (spec["create"] == true) create { id, amount -> R.v("@lambda", "create", id, amount) }
        if (spec["max"] == true) calculateMaxAmount { id -> R.i("@lambda", "max", id) }
    }

    fun call(fn: String, a: List<Any?>): Any? {
        args = a
        return when (fn) {
            // -------------------------------------------------- skills
            "getTool" -> getTool(P(0), B(1))
            "hasLevelDyn" -> hasLevelDyn(P(0), I(1), I(2))
            "hasLevelStat" -> hasLevelStat(P(0), I(1), I(2))
            "rewardXP" -> rewardXP(P(0), I(1), D(2))
            "setTempLevel" -> setTempLevel(E(0), I(1), I(2))
            "getStatLevel" -> getStatLevel(E(0), I(1))
            "getDynLevel" -> getDynLevel(E(0), I(1))
            "adjustLevel" -> adjustLevel(E(0), I(1), I(2))
            "heal" -> heal(E(0), I(1))
            "getMasteredSkillNames" -> getMasteredSkillNames(P(0))
            "getFamiliarBoost" -> getFamiliarBoost(P(0), I(1))
            "modPrayerPoints" -> modPrayerPoints(P(0), D(1))
            "hasDragonfireShieldProtection" -> hasDragonfireShieldProtection(P(0))
            "hasDragonfireShieldProtection(wyvern)" -> hasDragonfireShieldProtection(P(0), B(1))
            "calculateDragonfireMaxHit" -> calculateDragonfireMaxHit(E(0), I(1))
            "calculateDragonfireMaxHit(all)" -> calculateDragonfireMaxHit(E(0), I(1), B(2), I(3), B(4))
            // -------------------------------------------------- containers
            "amountInInventory" -> amountInInventory(P(0), I(1))
            "amountInBank" -> amountInBank(P(0), I(1))
            "amountInBank(includeSecondary)" -> amountInBank(P(0), I(1), B(2))
            "amountInEquipment" -> amountInEquipment(P(0), I(1))
            "inInventory" -> inInventory(P(0), I(1))
            "inInventory(amount)" -> inInventory(P(0), I(1), I(2))
            "inBank" -> inBank(P(0), I(1))
            "inBank(amount)" -> inBank(P(0), I(1), I(2))
            "inEquipment" -> inEquipment(P(0), I(1))
            "inEquipment(amount)" -> inEquipment(P(0), I(1), I(2))
            "anyInInventory" -> anyInInventory(P(0), *INTS(1))
            "inEquipmentOrInventory" -> inEquipmentOrInventory(P(0), I(1))
            "inEquipmentOrInventory(amount)" -> inEquipmentOrInventory(P(0), I(1), I(2))
            "allInEquipment" -> allInEquipment(P(0), *INTS(1))
            "anyInEquipment" -> anyInEquipment(P(0), *INTS(1))
            "getItemFromEquipment" -> getItemFromEquipment(P(0), EquipmentSlot.valueOf(S(1)))
            "hasAnItem(id)" -> hasAnItem(P(0), I(1)).let { listOf("ContainerisedItem", R.value(it.container), it.itemId, it.exists()) }
            "hasAnItem(id,flags)" -> hasAnItem(P(0), I(1), B(2), B(3)).let { listOf("ContainerisedItem", R.value(it.container), it.itemId, it.exists()) }
            "hasAnItem(ids,flags)" -> hasAnItem(P(0), INTS(1).toTypedArray(), B(2), B(3)).let { listOf("ContainerisedItem", R.value(it.container), it.itemId, it.exists()) }
            "ContainerisedItem.remove" -> ContainerisedItem(raw(0)?.let { R.containers[it as String] }, I(1)).remove()
            "hasGodItem" -> hasGodItem(P(0), God.valueOf(S(1)))
            "shouldRemoveNothings" -> shouldRemoveNothings(P(0))
            "removeItem" -> removeItem(P(0), A(1))
            "removeItem(container)" -> removeItem(P(0), A(1), Container.valueOf(S(2)))
            "removeItemsIfPlayerHasEnough" -> removeItemsIfPlayerHasEnough(P(0), *ITEMS(1))
            "addItem" -> addItem(P(0), I(1))
            "addItem(amount)" -> addItem(P(0), I(1), I(2))
            "addItem(container)" -> addItem(P(0), I(1), I(2), Container.valueOf(S(3)))
            "replaceSlot" -> replaceSlot(P(0), I(1), IT(2))
            "replaceSlot(current)" -> replaceSlot(P(0), I(1), IT(2), ITN(3))
            "replaceSlot(container)" -> replaceSlot(P(0), I(1), IT(2), ITN(3), Container.valueOf(S(4)))
            "replaceAllItems" -> replaceAllItems(P(0), I(1), I(2))
            "addItemOrDrop" -> addItemOrDrop(P(0), I(1))
            "addItemOrDrop(amount)" -> addItemOrDrop(P(0), I(1), I(2))
            "addItemOrBank" -> addItemOrBank(P(0), I(1))
            "addItemOrBank(amount)" -> addItemOrBank(P(0), I(1), I(2))
            "freeSlots" -> freeSlots(P(0))
            "hasSpaceFor" -> hasSpaceFor(P(0), IT(1))
            "removeAll" -> removeAll(P(0), A(1))
            "removeAll(container)" -> removeAll(P(0), A(1), Container.valueOf(S(2)))
            "note" -> note(IT(0))
            "unnote" -> unnote(IT(0))
            "equipSlot" -> equipSlot(I(0))
            "hasSealOfPassage" -> hasSealOfPassage(P(0))
            "dumpContainer" -> dumpContainer(P(0), R.containers[S(1)]!!)
            "dumpBeastOfBurden" -> dumpBeastOfBurden(P(0))
            "decantContainer" -> decantContainer(R.containers[S(0)]!!).let { listOf(R.value(it.first), R.value(it.second)) }
            // -------------------------------------------------- animation, graphics, audio
            "getAnimation" -> getAnimation(I(0))
            "getAnimationWithPriority" -> getAnimationWithPriority(I(0), Animator.Priority.valueOf(S(1)))
            "resetAnimator" -> resetAnimator(P(0))
            "renderAnimation" -> renderAnimation(P(0), I(1))
            "resetRenderAnimation" -> resetRenderAnimation(P(0))
            "animationDuration" -> animationDuration(A(0) as Animation)
            "animationCycles" -> animationCycles(I(0))
            "animate" -> animate(E(0), A(1))
            "animate(forced)" -> animate(E(0), A(1), B(2))
            "visualize" -> visualize(E(0), A(1), A(2))
            "sendGraphics" -> sendGraphics(A(0), L(1))
            "emote" -> emote(E(0), Emotes.valueOf(S(1)))
            "animateScenery(player)" -> animateScenery(P(0), SC(1), I(2))
            "animateScenery(player,global)" -> animateScenery(P(0), SC(1), I(2), B(3))
            "animateScenery" -> animateScenery(SC(0), I(1))
            "animateInterface" -> animateInterface(P(0), I(1), I(2), I(3))
            "spawnProjectile(entity)" -> spawnProjectile(E(0), E(1), I(2))
            "spawnProjectile(location)" -> spawnProjectile(L(0), L(1), I(2), I(3), I(4), I(5), I(6), I(7))
            "playJingle" -> playJingle(P(0), I(1))
            "playAudio" -> playAudio(P(0), I(1))
            "playAudio(delay,loops)" -> playAudio(P(0), I(1), I(2), I(3))
            "playAudio(all)" -> playAudio(P(0), I(1), I(2), I(3), LN(4), I(5))
            "playGlobalAudio" -> playGlobalAudio(L(0), I(1))
            "playGlobalAudio(all)" -> playGlobalAudio(L(0), I(1), I(2), I(3), I(4))
            "playHurtAudio" -> playHurtAudio(P(0))
            "playHurtAudio(delay)" -> playHurtAudio(P(0), I(1))
            "impact" -> impact(E(0), I(1))
            "impact(type)" -> impact(E(0), I(1), ImpactHandler.HitsplatType.valueOf(S(2)))
            // -------------------------------------------------- scenery, ground items, definitions
            "replaceScenery" -> replaceScenery(SC(0), I(1), I(2))
            "replaceScenery(location)" -> replaceScenery(SC(0), I(1), I(2), LN(3))
            "replaceScenery(direction)" -> replaceScenery(SC(0), I(1), I(2), Direction.valueOf(S(3)))
            "replaceScenery(direction,location)" -> replaceScenery(SC(0), I(1), I(2), Direction.valueOf(S(3)), LN(4))
            "addScenery(id)" -> addScenery(I(0), L(1))
            "addScenery(id,rotation,type)" -> addScenery(I(0), L(1), I(2), I(3))
            "addScenery(scenery)" -> addScenery(SC(0))
            "removeScenery" -> removeScenery(SC(0))
            "getScenery(xyz)" -> getScenery(I(0), I(1), I(2))
            "getScenery(location)" -> getScenery(L(0))
            "getChildren" -> getChildren(I(0))
            "sceneryDefinition" -> sceneryDefinition(I(0)).let { "SceneryDefinition:" + it.id }
            "getItemName" -> getItemName(I(0))
            "itemDefinition" -> itemDefinition(I(0)).let { "ItemDefinition:" + it.id }
            "hasOption" -> hasOption(N(0), S(1))
            "removeGroundItem" -> removeGroundItem(node<GroundItem>(0))
            "isValidGroundItem" -> isValidGroundItem(node<GroundItem>(0))
            "produceGroundItem(player)" -> produceGroundItem(P(0), I(1))
            "produceGroundItem(owner)" -> produceGroundItem(raw(0)?.let { R.node(it) as Player }, I(1), I(2), L(3))
            "adjustCharge" -> adjustCharge(N(0), I(1))
            "getCharge" -> getCharge(N(0))
            "setCharge" -> setCharge(N(0), I(1))
            // -------------------------------------------------- world, NPCs, movement
            "getWorldTicks" -> getWorldTicks()
            "poofClear" -> poofClear(NPC_(0))
            "findNPC(id)" -> findNPC(I(0))
            "findNPC(location)" -> findNPC(L(0), I(1))
            "findLocalNPC" -> findLocalNPC(E(0), I(1))
            "findLocalNPCs(location)" -> findLocalNPCs(L(0), I(1))
            "findLocalNPCs(ids)" -> findLocalNPCs(E(0), INTS(1))
            "findLocalNPCs(ids,distance)" -> findLocalNPCs(E(0), INTS(1), I(2))
            "getRegionBorders" -> getRegionBorders(I(0))
            "location" -> location(I(0), I(1), I(2))
            "inBorders(borders)" -> inBorders(E(0), (raw(1) as List<*>).let { ZoneBorders((it[0] as Number).toInt(), (it[1] as Number).toInt(), (it[2] as Number).toInt(), (it[3] as Number).toInt(), (it[4] as Number).toInt()) })
            "inBorders(corners)" -> inBorders(E(0), I(1), I(2), I(3), I(4))
            "transformNpc" -> transformNpc(NPC_(0), I(1), I(2))
            "transformNpcWithHpCarryover" -> transformNpcWithHpCarryover(NPC_(0), I(1), I(2))
            "forceWalk" -> forceWalk(E(0), L(1), S(2))
            "truncateLoc" -> truncateLoc(E(0), L(1)).let { listOf(it.first, R.value(it.second)) }
            "forceMove" -> forceMove(P(0), L(1), L(2), I(3), I(4))
            "forceMove(all)" -> forceMove(P(0), L(1), L(2), I(3), I(4), raw(5)?.let { Direction.valueOf(it as String) }, I(6), raw(7)?.let { task(it as String) })
            "stopWalk" -> stopWalk(E(0))
            "flee" -> flee(E(0), E(1))
            "teleport" -> teleport(E(0), L(1))
            "teleport(type)" -> teleport(E(0), L(1), TeleportManager.TeleportType.valueOf(S(2)))
            "face" -> face(E(0), N(1))
            "face(duration)" -> face(E(0), N(1), I(2))
            "resetFace" -> resetFace(E(0))
            "getPathableRandomLocalCoordinate" -> getPathableRandomLocalCoordinate(E(0), I(1), L(2))
            "getPathableRandomLocalCoordinate(attempts)" -> getPathableRandomLocalCoordinate(E(0), I(1), L(2), I(3))
            "getPathableCardinal" -> getPathableCardinal(E(0), L(1))
            "registerMapZone" -> registerMapZone(zone(S(0)), (raw(1) as List<*>).let { ZoneBorders((it[0] as Number).toInt(), (it[1] as Number).toInt(), (it[2] as Number).toInt(), (it[3] as Number).toInt(), (it[4] as Number).toInt()) })
            "addClimbDest" -> addClimbDest(L(0), L(1))
            "handleDoor" -> handleDoor(E(0), SC(1))
            "handleAutowalkDoor" -> handleAutowalkDoor(raw(0)?.let { R.node(it) as Entity }, raw(1)?.let { R.node(it) as Scenery })
            "hasLineOfSight" -> hasLineOfSight(E(0), N(1))
            "isPlayer" -> isPlayer(N(0))
            // -------------------------------------------------- interfaces and dialogue
            "openInterface" -> openInterface(P(0), I(1))
            "openOverlay" -> openOverlay(P(0), I(1))
            "closeOverlay" -> closeOverlay(P(0))
            "openSingleTab" -> openSingleTab(P(0), I(1))
            "closeInterface" -> closeInterface(P(0))
            "closeTabInterface" -> closeTabInterface(P(0))
            "closeAllInterfaces" -> closeAllInterfaces(P(0))
            "setInterfaceText" -> setInterfaceText(P(0), S(1), I(2), I(3))
            "sendString" -> sendString(P(0), S(1), I(2), I(3))
            "setComponentVisibility" -> setComponentVisibility(P(0), I(1), I(2), B(3))
            "setTitle" -> setTitle(P(0), I(1))
            "removeTabs" -> removeTabs(P(0), *(raw(1) as List<*>).map { GAMETAB.valueOf(it as String) }.toTypedArray())
            "restoreTabs" -> restoreTabs(P(0))
            "sendPlayerOnInterface" -> sendPlayerOnInterface(P(0), I(1), I(2))
            "sendAnimationOnInterface" -> sendAnimationOnInterface(P(0), I(1), I(2), I(3))
            "sendItemOnInterface" -> sendItemOnInterface(P(0), I(1), I(2), I(3))
            "sendItemOnInterface(amount)" -> sendItemOnInterface(P(0), I(1), I(2), I(3), I(4))
            "sendItemZoomOnInterface" -> sendItemZoomOnInterface(P(0), I(1), I(2), I(3))
            "sendItemZoomOnInterface(zoom)" -> sendItemZoomOnInterface(P(0), I(1), I(2), I(3), I(4))
            "sendMessage" -> sendMessage(P(0), S(1))
            "sendChat" -> sendChat(E(0), S(1))
            "sendChat(delay)" -> sendChat(E(0), S(1), I(2))
            "sendNews" -> sendNews(S(0))
            "sendDialogue" -> sendDialogue(P(0), S(1))
            "sendDialogueLines" -> sendDialogueLines(P(0), *STRS(1))
            "sendDialogueOptions" -> sendDialogueOptions(P(0), S(1), *STRS(2))
            "sendPlayerDialogue" -> sendPlayerDialogue(P(0), S(1))
            "sendPlayerDialogue(all)" -> sendPlayerDialogue(P(0), S(1), FacialExpression.valueOf(S(2)), B(3))
            "sendNPCDialogue" -> sendNPCDialogue(P(0), I(1), S(2))
            "sendNPCDialogue(all)" -> sendNPCDialogue(P(0), I(1), S(2), FacialExpression.valueOf(S(3)), B(4))
            "sendNPCDialogueLines" -> sendNPCDialogueLines(P(0), I(1), FacialExpression.valueOf(S(2)), B(3), *STRS(4))
            "sendItemDialogue" -> sendItemDialogue(P(0), A(1)!!, S(2))
            "sendDoubleItemDialogue" -> sendDoubleItemDialogue(P(0), A(1)!!, A(2)!!, S(3))
            "sendInputDialogue(numeric)" -> sendInputDialogue(P(0), B(1), S(2), valueHandler(S(3)))
            "sendInputDialogue(type)" -> sendInputDialogue(P(0), InputType.valueOf(S(1)), S(2), valueHandler(S(3)))
            "openDialogue" -> openDialogue(P(0), A(1)!!, *ANYS(2))
            "closeDialogue" -> closeDialogue(P(0))
            "addDialogueAction" -> addDialogueAction(P(0), action(S(1)))
            "runcs2" -> runcs2(P(0), I(1), *ANYS(2))
            "sendItemSelect" -> sendItemSelect(P(0), *STRS(1), callback = select(S(2)))
            "sendItemSelect(keepAlive)" -> sendItemSelect(P(0), *STRS(1), keepAlive = B(2), callback = select(S(3)))
            "sendSkillDialogue" -> sendSkillDialogue(P(0), skillInit(raw(1) as Map<*, *>))
            "registerLogoutListener" -> registerLogoutListener(P(0), S(1), playerHandler(S(2)))
            "clearLogoutListener" -> clearLogoutListener(P(0), S(1))
            "registerHintIcon(location)" -> registerHintIcon(P(0), L(1), I(2))
            "registerHintIcon(node)" -> registerHintIcon(P(0), N(1))
            "clearHintIcon" -> clearHintIcon(P(0))
            // -------------------------------------------------- attributes, timers, locks
            "getAttribute" -> getAttribute(E(0), S(1), A(2))
            "setAttribute" -> setAttribute(E(0), S(1), A(2))
            "removeAttribute" -> removeAttribute(E(0), S(1))
            "removeAttributes" -> removeAttributes(E(0), *STRS(1))
            "registerTimer" -> registerTimer(E(0), raw(1)?.let { R.node(it) as core.game.system.timer.RSTimer })
            "getOrStartTimer" -> getOrStartTimer(E(0), S(1), *ANYS(2))?.key
            "getOrStartTimer<Poison>" -> getOrStartTimer<core.game.system.timer.impl.Poison>(E(0), *ANYS(1)).key
            "spawnTimer" -> spawnTimer(S(0), *ANYS(1))?.key
            "spawnTimer<Poison>" -> spawnTimer<core.game.system.timer.impl.Poison>(*ANYS(0)).key
            "hasTimerActive<Poison>" -> hasTimerActive<core.game.system.timer.impl.Poison>(E(0))
            "getTimer<Poison>" -> getTimer<core.game.system.timer.impl.Poison>(E(0))?.key
            "removeTimer<Poison>" -> removeTimer<core.game.system.timer.impl.Poison>(E(0))
            "hasTimerActive" -> hasTimerActive(E(0), S(1))
            "getTimer" -> getTimer(E(0), S(1))?.key
            "removeTimer(identifier)" -> removeTimer(E(0), S(1))
            "removeTimer(timer)" -> removeTimer(E(0), R.node(raw(1)) as core.game.system.timer.RSTimer)
            "applyPoison" -> applyPoison(E(0), E(1), I(2))
            "isPoisoned" -> isPoisoned(E(0))
            "curePoison" -> curePoison(E(0))
            "lock" -> lock(E(0), I(1))
            "lockInteractions" -> lockInteractions(E(0), I(1))
            "unlock" -> unlock(E(0))
            // -------------------------------------------------- varps
            "getVarp" -> getVarp(P(0), I(1))
            "getVarbit" -> getVarbit(P(0), I(1))
            "getVarbit(definition)" -> getVarbit(P(0), R.varbit(I(1)))
            "setVarp" -> setVarp(P(0), I(1), I(2))
            "setVarp(save)" -> setVarp(P(0), I(1), I(2), B(3))
            "saveVarp" -> saveVarp(P(0), I(1))
            "unsaveVarp" -> unsaveVarp(P(0), I(1))
            "setVarbit" -> setVarbit(P(0), I(1), I(2))
            "setVarbit(save)" -> setVarbit(P(0), I(1), I(2), B(3))
            "setVarbit(definition)" -> setVarbit(P(0), R.varbit(I(1)), I(2), B(3))
            "setVarc" -> setVarc(P(0), I(1), I(2))
            "reinitVarps" -> reinitVarps(P(0))
            // -------------------------------------------------- pulses and scripts
            "getUsedOption" -> getUsedOption(P(0))
            "submitWorldPulse" -> submitWorldPulse(pulse(S(0), I(1)))
            "runWorldTask" -> runWorldTask(task(S(0))).let { R.label(it) }
            "submitIndividualPulse" -> submitIndividualPulse(E(0), pulse(S(1), I(2)))
            "runTask" -> runTask(E(0), task = task(S(1)))
            "runTask(delay,repeat)" -> runTask(E(0), I(1), I(2), task(S(3)))
            "finishedMoving" -> finishedMoving(E(0))
            "delayScript" -> delayScript(E(0), I(1))
            "delayEntity" -> delayEntity(E(0), I(1))
            "apRange" -> apRange(E(0), I(1))
            "animationFinished" -> animationFinished(E(0))
            "clearScripts" -> clearScripts(E(0))
            "restartScript" -> restartScript(E(0))
            "keepRunning" -> keepRunning(E(0))
            "stopExecuting" -> stopExecuting(E(0))
            "queueScript" -> queueScript(E(0), script = stage(S(1)))
            "queueScript(all)" -> queueScript(E(0), I(1), QueueStrength.valueOf(S(2)), B(3), stage(S(4)))
            "delayClock" -> delayClock(E(0), I(1), I(2))
            "delayClock(script)" -> delayClock(E(0), I(1), I(2), B(3))
            "clockReady" -> clockReady(E(0), I(1))
            "delayAttack" -> delayAttack(E(0), I(1))
            "stun" -> stun(E(0), I(1))
            "stun(message)" -> stun(E(0), I(1), B(2))
            "isStunned" -> isStunned(E(0))
            "setCurrentScriptState" -> setCurrentScriptState(E(0), I(1))
            // -------------------------------------------------- quests and requirements
            "getQuestPoints" -> getQuestPoints(P(0))
            "getQuestStage" -> getQuestStage(P(0), Quests.valueOf(S(1)))
            "setQuestStage" -> setQuestStage(P(0), Quests.valueOf(S(1)), I(2))
            "isQuestInProgress" -> isQuestInProgress(P(0), Quests.valueOf(S(1)), I(2), I(3))
            "isQuestStarted" -> isQuestStarted(P(0), Quests.valueOf(S(1)))
            "isQuestComplete" -> isQuestComplete(P(0), Quests.valueOf(S(1)))
            "getQuest" -> getQuest(P(0), Quests.valueOf(S(1))).key
            "startQuest" -> startQuest(P(0), Quests.valueOf(S(1)))
            "finishQuest" -> finishQuest(P(0), Quests.valueOf(S(1)))
            "requireQuest" -> requireQuest(P(0), Quests.valueOf(S(1)), S(2))
            "hasRequirement(req)" -> hasRequirement(P(0), QuestReq(QuestRequirements.valueOf(S(1))))
            "hasRequirement(req,message)" -> hasRequirement(P(0), QuestReq(QuestRequirements.valueOf(S(1))), B(2))
            "hasRequirement(quest)" -> hasRequirement(P(0), Quests.valueOf(S(1)))
            "hasRequirement(quest,message)" -> hasRequirement(P(0), Quests.valueOf(S(1)), B(2))
            "finishTask" -> finishTask(P(0), DiaryType.valueOf(S(1)), DiaryLevel.valueOf(S(2)), I(3))
            "getDiaryLevelIndex" -> getDiaryLevelIndex(DiaryType.valueOf(S(0)), DiaryLevel.valueOf(S(1)))
            // -------------------------------------------------- slayer, ironman, bank, shops, house
            "getSlayerTask" -> getSlayerTask(P(0))
            "getSlayerTaskName" -> getSlayerTaskName(P(0))
            "getSlayerTaskKillsRemaining" -> getSlayerTaskKillsRemaining(P(0))
            "getSlayerMaster" -> getSlayerMaster(P(0))
            "getSlayerMasterLocation" -> getSlayerMasterLocation(P(0))
            "getSlayerTip" -> getSlayerTip(P(0)).toList()
            "getSlayerTaskFlags" -> getSlayerTaskFlags(P(0))
            "getSlayerFlags" -> getSlayerFlags(P(0)).let { "SlayerFlags" }
            "hasSlayerTask" -> hasSlayerTask(P(0))
            "hasIronmanRestriction" -> hasIronmanRestriction(P(0), IronmanMode.valueOf(S(1)))
            "restrictForIronman" -> restrictForIronman(P(0), IronmanMode.valueOf(S(1)), task(S(2)))
            "openGrandExchange" -> openGrandExchange(P(0))
            "hasAwaitingGrandExchangeCollections" -> hasAwaitingGrandExchangeCollections(P(0))
            "openGrandExchangeCollectionBox" -> openGrandExchangeCollectionBox(P(0))
            "openBankAccount" -> openBankAccount(P(0))
            "openDepositBox" -> openDepositBox(P(0))
            "openBankPinSettings" -> openBankPinSettings(P(0))
            "activateSecondaryBankAccount" -> activateSecondaryBankAccount(P(0))
            "hasActivatedSecondaryBankAccount" -> hasActivatedSecondaryBankAccount(P(0))
            "toggleBankAccount" -> toggleBankAccount(P(0))
            "isUsingSecondaryBankAccount" -> isUsingSecondaryBankAccount(P(0))
            "getBankAccountName" -> getBankAccountName(P(0))
            "getBankAccountName(invert)" -> getBankAccountName(P(0), B(1))
            "openNpcShop" -> openNpcShop(P(0), I(1))
            "hasHouse" -> hasHouse(P(0))
            "storeBookInHouse(id)" -> storeBookInHouse(P(0), I(1))
            "storeBookInHouse(node)" -> storeBookInHouse(P(0), N(1))
            "getCutscene" -> P(0).getCutscene()
            "getCutsceneStage" -> P(0).getCutsceneStage()
            "updateCredits" -> updateCredits(P(0), I(1))
            "getCredits" -> getCredits(P(0))
            "announceIfRare" -> announceIfRare(P(0), IT(1))
            // -------------------------------------------------- logging and configuration
            "log" -> log(Calls::class.java, Log.valueOf(S(0)), S(1))
            "logWithStack" -> logWithStack(Calls::class.java, Log.valueOf(S(0)), S(1))
            "exceptionToString" -> R.frames(exceptionToString(Exception(S(0))))
            "getServerConfig" -> getServerConfig()
            // -------------------------------------------------- the other core/api files
            "splitLines" -> splitLines(S(0)).toList()
            "splitLines(limit)" -> splitLines(S(0), I(1)).toList()
            "EquipmentSlot.slotByName" -> EquipmentSlot.slotByName(S(0))
            "God.validItems" -> God.valueOf(S(0)).validItems.toList()
            "IfaceSettingsBuilder" -> iface(raw(0) as List<*>)
            "asItem" -> I(0).asItem()
            "IntProgression.toIntArray" -> IntProgression.fromClosedRange(I(0), I(1), I(2)).toIntArray().toList()
            "Collection<IntArray>.toIntArray" -> (raw(0) as List<*>).map { (it as List<*>).map { x -> (x as Number).toInt() }.toIntArray() }.toIntArray().toList()
            "IntArray.isLast" -> INTS(0).isLast(I(1))
            "IntArray.getNext" -> INTS(0).getNext(I(1))
            "IntArray.isNextLast" -> INTS(0).isNextLast(I(1))
            "Collection.isLast" -> INTS(0).toList().isLast(I(1))
            "Collection.getNext" -> INTS(0).toList().getNext(I(1))
            "Collection.isNextLast" -> INTS(0).toList().isNextLast(I(1))
            "tryPop" -> java.util.LinkedList(INTS(0).toList()).let { listOf(it.tryPop(raw(1)?.let { x -> (x as Number).toInt() }), it.toList()) }
            "parseEnumEntry<God>" -> parseEnumEntry<God>(S(0))
            "Event" -> eventNames()
            "Commands.reject" -> commands.reject(P(0), *STRS(1))
            "Commands.notify" -> commands.notify(P(0), S(1))
            "Commands.notify(log)" -> commands.notify(P(0), S(1), B(2))
            "Commands.define" -> commands.define(S(0), handle = command(S(1)))
            "Commands.define(all)" -> commands.define(S(0), core.game.system.command.Privilege.valueOf(S(1)), S(2), S(3), command(S(4)))
            "MapArea.zone" -> mapArea()
            "Vector" -> vector(raw(0) as List<*>)
            "regionspec" -> regionspec(raw(0) as String)
            else -> throw UnsupportedOperationException(fn)
        }
    }

    private val commands = object : Commands { override fun defineCommands() {} }

    private fun iface(steps: List<*>): Int {
        val b = IfaceSettingsBuilder()
        for (s in steps) {
            val step = s as List<*>
            fun n(i: Int) = (step[i] as Number).toInt()
            when (step[0]) {
                "enableOption" -> b.enableOption(n(1))
                "enableOptions(ids)" -> b.enableOptions(*step.drop(1).map { (it as Number).toInt() }.toIntArray())
                "enableOptions(range)" -> b.enableOptions(n(1) until n(2))
                "enableOptions(names)" -> b.enableOptions(*step.drop(1).map { it as String? }.toTypedArray())
                "enableAllOptions" -> b.enableAllOptions()
                "setUseOnSettings" -> b.setUseOnSettings(step[1] as Boolean, step[2] as Boolean, step[3] as Boolean, step[4] as Boolean, step[5] as Boolean, step[6] as Boolean)
                "setInterfaceEventsDepth" -> b.setInterfaceEventsDepth(n(1))
                "enableSlotSwitch" -> b.enableSlotSwitch()
                "enableUseOption" -> b.enableUseOption()
                "enableExamine" -> b.enableExamine()
                "enableUseOnSelf" -> b.enableUseOnSelf()
                "enableNullSlotSwitch" -> b.enableNullSlotSwitch()
                else -> throw IllegalArgumentException(step[0].toString())
            }
        }
        return b.build()
    }

    private fun eventNames(): List<String> = listOf(
        Event.ResourceProduced, Event.NPCKilled, Event.BoneBuried, Event.Teleported, Event.FireLit, Event.LightSourceLit,
        Event.Interacted, Event.ButtonClicked, Event.DialogueOpened, Event.DialogueOptionSelected, Event.DialogueClosed,
        Event.UsedWith, Event.SelfDeath, Event.Tick, Event.PickedUp, Event.InterfaceOpened, Event.InterfaceClosed,
        Event.AttributeSet, Event.AttributeRemoved, Event.SpellCast, Event.SpellbookChanged, Event.ItemAlchemized,
        Event.ItemEquipped, Event.ItemUnequipped, Event.ItemPurchased, Event.ItemSold, Event.JobAssigned,
        Event.FairyRingDialed, Event.VarbitUpdated, Event.DynamicSkillLevelChanged, Event.SummoningPointsRecharged,
        Event.PrayerPointsRecharged, Event.XpGained, Event.PrayerActivated, Event.PrayerDeactivated
    ).map { it.simpleName }

    private class AreaA : MapArea { override fun defineAreaBorders(): Array<ZoneBorders> = arrayOf() }
    private fun mapArea(): List<Any?> {
        val area = AreaA()
        val zone = zone("zoneA")
        val before = try { area.zone.key } catch (e: NullPointerException) { "Null" }
        area.zone = zone
        return listOf(before, area.zone.key, MapArea.zoneMaps.keys.sorted(), area.getRestrictions().size, area.isPvpAllowed(area as? Entity ?: Entity(), null, null, false), area.canStartDeath(Entity(), Entity()))
    }

    private fun vector(spec: List<*>): List<Any?> {
        val v = Vector((spec[0] as Number).toDouble(), (spec[1] as Number).toDouble())
        val w = Vector((spec[2] as Number).toDouble(), (spec[3] as Number).toDouble())
        val out = ArrayList<Any?>()
        out.add(v.magnitude())
        out.add(v.normalized().toString())
        out.add((v * 2.5).toString())
        out.add((v * 3).toString())
        out.add((v + w).toString())
        out.add((v - w).toString())
        out.add(v.invert().toString())
        out.add(R.value(v.toLocation((spec[4] as Number).toInt())))
        out.add(try { v.toDirection().name } catch (e: Exception) { e.javaClass.simpleName })
        out.add(Vector.betweenLocs(Location.create(1, 2, 0), Location.create(4, -2, 1)).toString())
        out.add(Vector.deriveWithEqualComponents((spec[0] as Number).toDouble()).toString())
        return out
    }

    private fun regionspec(kind: String): Any? = when (kind) {
        "empty" -> R.value(RegionSpecification().build())
        "copy" -> R.value(RegionSpecification(copyOf(12850), EmptyChunkContract()).build())
        "using" -> R.value(RegionSpecification(using(core.game.world.map.build.DynamicRegion.create(1)), EmptyChunkContract()).build())
        "fill" -> R.value(RegionSpecification(using(R.node("dyn") as core.game.world.map.build.DynamicRegion),
            fillWith(R.node("chunk") as core.game.world.map.RegionChunk).from(R.node("src") as core.game.world.map.Region).onPlanes(1, 2)
                .onCondition(named("cond") { x: Int, y: Int, z: Int -> R.b("@lambda", "cond", x, y, z) })).build())
        "fillDelegate" -> R.value(RegionSpecification(using(R.node("dyn") as core.game.world.map.build.DynamicRegion),
            fillWith(named<(Int, Int, Int, core.game.world.map.Region) -> core.game.world.map.RegionChunk?>("delegate") { x: Int, y: Int, z: Int, r: core.game.world.map.Region -> R.n<core.game.world.map.RegionChunk>("@lambda", "delegate", x, y, z, r) })
                .from(R.node("src") as core.game.world.map.Region).onCondition(named("cond") { x: Int, y: Int, z: Int -> R.b("@lambda", "cond", x, y, z) })).build())
        "fillLateinit" -> R.value(RegionSpecification(using(R.node("dyn") as core.game.world.map.build.DynamicRegion), fillWith(R.node("chunk") as core.game.world.map.RegionChunk)).build())
        else -> throw IllegalArgumentException(kind)
    }
}
