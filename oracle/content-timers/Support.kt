package oracle

import content.global.ame.RandomEventNPC
import content.global.ame.RandomEvents
import content.global.skill.summoning.familiar.Familiar
import core.api.*
import core.api.utils.WeightBasedTable
import core.game.dialogue.DialoguePlugin
import core.game.event.*
import core.game.event.Event as GameEvent
import core.game.node.entity.Entity
import core.game.node.entity.combat.equipment.WeaponInterface
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.SpellBookManager.SpellBook
import core.game.node.entity.player.link.SpellBookManager.SpellbookChangeSource
import core.game.node.entity.player.link.prayer.PrayerType
import core.game.node.entity.skill.Skills
import core.game.node.item.Item
import core.game.system.timer.*
import core.game.system.timer.impl.*
import core.game.world.map.Location
import core.tools.RandomFunction
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStream
import java.io.PrintStream
import java.nio.charset.StandardCharsets
import java.util.IdentityHashMap
import java.util.Random
import org.json.simple.JSONArray
import org.json.simple.JSONObject
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser

/** The per-class operations a case can name; the reified ContentAPI functions need one instance per class. */
class Ops(
    val spawn: (Array<out Any>) -> RSTimer,
    val has: (Entity) -> Boolean,
    val get: (Entity) -> RSTimer?,
    val remove: (Entity) -> Unit,
    val getOrStart: (Entity, Array<out Any>) -> RSTimer,
)

inline fun <reified T : RSTimer> ops() = Ops(
    { a -> spawnTimer<T>(*a) },
    { e -> hasTimerActive<T>(e) },
    { e -> getTimer<T>(e) },
    { e -> removeTimer<T>(e) },
    { e, a -> getOrStartTimer<T>(e, *a) },
)

/** A hook of the case: records each event it receives. */
class ForeignHook(val name: String) : EventHook<GameEvent> {
    override fun process(entity: Entity, event: GameEvent) {
        Support.emit("foreign_hook", name, Support.ix(entity), Support.describe(event))
    }
}

/** Records every stack trace TimerManager prints (its first line is the exception). */
class Traces : PrintStream(object : OutputStream() { override fun write(b: Int) {} }) {
    override fun println(x: Any?) {
        if (x is Throwable) Support.note("exception", Support.errorName(x))
    }
}

object Support {
    val CLASSES = listOf("AntiMacro", "Disease", "DragonFireImmunity", "Frozen", "FrozenImmunity", "HealOverTime", "Miasmic",
        "MiasmicImmunity", "Poison", "PoisonImmunity", "SkillRestore", "Skulled", "SpellbookSwap", "Teleblock")
    val OPS: Map<String, Ops> = mapOf(
        "AntiMacro" to ops<AntiMacro>(), "Disease" to ops<Disease>(), "DragonFireImmunity" to ops<DragonFireImmunity>(),
        "Frozen" to ops<Frozen>(), "FrozenImmunity" to ops<FrozenImmunity>(), "HealOverTime" to ops<HealOverTime>(),
        "Miasmic" to ops<Miasmic>(), "MiasmicImmunity" to ops<MiasmicImmunity>(), "Poison" to ops<Poison>(),
        "PoisonImmunity" to ops<PoisonImmunity>(), "SkillRestore" to ops<SkillRestore>(), "Skulled" to ops<Skulled>(),
        "SpellbookSwap" to ops<SpellbookSwap>(), "Teleblock" to ops<Teleblock>(),
        "RSTimer" to ops<RSTimer>(), "PersistTimer" to ops<PersistTimer>(),
    )

    class Failure : RuntimeException()

    class Rng(seed: Long) : Random(seed) {
        override fun nextInt(bound: Int): Int {
            val v = super.nextInt(bound)
            emit("next_int", bound, v)
            return v
        }
    }

    lateinit var input: JSONObject
    var events = ArrayList<Any?>()
    var calls = 0
    var failAt = -1
    var tick = 0
    val entities = ArrayList<Entity>()
    val specs = IdentityHashMap<Entity, JSONObject>()
    val labels = IdentityHashMap<RSTimer, String>()
    val labelled = ArrayList<RSTimer>()
    var auto = 0
    val npcs = ArrayList<RandomEventNPC>()
    val weapons = IdentityHashMap<Entity, WeaponInterface>()
    var nonSkill = 0

    @JvmStatic fun emit(name: String, vararg args: Any?) {
        val row = ArrayList<Any?>()
        row.add(name)
        row.addAll(args)
        events.add(row)
        calls++
        if (calls == failAt) throw Failure()
    }

    /** An observation that is not a call (a printed stack trace): it never fails. */
    @JvmStatic fun note(name: String, vararg args: Any?) {
        val row = ArrayList<Any?>()
        row.add(name)
        row.addAll(args)
        events.add(row)
    }

    @JvmStatic fun ix(o: Any?): Int? {
        if (o == null) return null
        for (i in entities.indices) if (entities[i] === o) return i
        throw IllegalStateException("unknown entity")
    }

    @JvmStatic fun ticks(): Int {
        emit("ticks", tick)
        return tick
    }

    @JvmStatic fun spec(e: Any?, key: String): Any? = specs[e as Entity]!![key]
    @JvmStatic fun specOr(e: Any?, key: String, fallback: Any?): Any? = spec(e, key) ?: fallback
    @JvmStatic fun flag(e: Any?, key: String, fallback: Boolean): Boolean = (spec(e, key) as? Boolean) ?: fallback
    @JvmStatic fun number(e: Any?, key: String, fallback: Int): Int = (spec(e, key) as? Number)?.toInt() ?: fallback
    @JvmStatic fun name(e: Any?): String? = spec(e, "name") as? String
    @JvmStatic fun location(e: Any?): Location? {
        val l = spec(e, "location") as? JSONArray ?: return null
        return Location(n(l[0]), n(l[1]), n(l[2]))
    }
    @JvmStatic fun entityAt(e: Any?, key: String): Entity? = (spec(e, key) as? Number)?.let { entities[it.toInt()] }
    @JvmStatic fun worn(e: Any?, slot: Int): Int? = ((spec(e, "worn") as? JSONObject)?.get(slot.toString()) as? Number)?.toInt()
    @JvmStatic fun byUid(uid: Int): Player? = ((input["uids"] as? JSONObject)?.get(uid.toString()) as? Number)?.let { entities[it.toInt()] as Player }
    @JvmStatic fun dialogueIds(label: String): IntArray? {
        val ids = (input["dialogues"] as? JSONObject)?.get(label) as? JSONArray ?: return null
        return IntArray(ids.size) { n(ids[it]) }
    }
    @JvmStatic fun extension(e: Entity, clazz: Class<*>): Any? {
        if (clazz != WeaponInterface::class.java) return null
        val id = (spec(e, "weapon") as? Number)?.toInt() ?: return null
        return weapons.getOrPut(e) { WeaponInterface(e as Player, id) }
    }
    @JvmStatic fun skillRandom(skill: Int): RandomEvents? =
        ((input["skillRandom"] as? JSONObject)?.get(skill.toString()) as? String)?.let { RandomEvents.valueOf(it) }
    @JvmStatic fun nonSkillRandom(): RandomEvents {
        val pool = input["nonSkill"] as? JSONArray ?: JSONArray().also { it.add("GENIE") }
        return RandomEvents.valueOf(pool[nonSkill++ % pool.size] as String)
    }
    @JvmStatic fun createNpc(prototype: RandomEventNPC, loot: WeightBasedTable?): RandomEventNPC {
        val npc = RandomEventNPC("npc${npcs.size}")
        val missing = (input["spawnMissing"] as? JSONArray)?.contains(npcs.size.toLong()) == true
        npcs.add(npc)
        npc.loot = loot
        if (!missing) npc.spawnLocation = Location(3222, 3222, 0)
        return npc
    }
    @JvmStatic fun loot(label: String): ArrayList<Item> {
        val out = ArrayList<Item>()
        val rows = (input["loot"] as? JSONObject)?.get(label) as? JSONArray ?: return out
        for (r in rows) out.add(Item(n((r as JSONArray)[0]), n(r[1])))
        return out
    }

    @JvmStatic fun value(v: Any?): Any? = when (v) {
        null -> null
        is Boolean, is Number, is String -> v
        is RandomEventNPC -> JSONObject().also { it["npc"] = v.label }
        else -> v.javaClass.simpleName
    }

    @JvmStatic fun describe(event: Any?): Any? = when (event) {
        is PrayerActivatedEvent -> listOf("PrayerActivatedEvent", event.type.name)
        is PrayerDeactivatedEvent -> listOf("PrayerDeactivatedEvent", event.type.name)
        is SpellCastEvent -> listOf("SpellCastEvent", event.spellBook.name, event.spellId)
        is SpellbookChangeEvent -> listOf("SpellbookChangeEvent", event.oldSpellBook.name, event.newSpellBook.name, event.source.name)
        is DialogueCloseEvent -> listOf("DialogueCloseEvent", event.dialogue.label)
        else -> listOf(event?.javaClass?.simpleName)
    }

    /** The hook's name: SkillRestore's two objects, a SpellbookSwap instance's three hooks, or a case hook. */
    @JvmStatic fun hookLabel(hook: Any?): String {
        when (hook) {
            null -> return "null"
            is ForeignHook -> return "foreign:" + hook.name
            SkillRestore.PrayerActivatedHook -> return "PrayerActivatedHook"
            SkillRestore.PrayerDeactivatedHook -> return "PrayerDeactivatedHook"
        }
        val owners = ArrayList<RSTimer>(labelled)
        for (e in entities) owners.addAll(listOf(e.timers.activeTimers, e.timers.newTimers, e.timers.toRemoveTimers).flatten().filterNotNull())
        for (t in owners) {
            if (t !is SpellbookSwap) continue
            for (field in listOf("spellCastHook", "spellBookChangeHook", "dialogueCloseHook")) {
                val f = SpellbookSwap::class.java.getDeclaredField(field)
                f.isAccessible = true
                if (f.get(t) === hook) return label(t) + ":" + field
            }
        }
        throw IllegalStateException("unknown hook")
    }

    @JvmStatic fun label(t: RSTimer?): String? {
        if (t == null) return null
        labels[t]?.let { return it }
        val name = "t${auto++}"
        name(t, name)
        return name
    }

    fun name(t: RSTimer, name: String) {
        if (labels.containsKey(t)) return
        labels[t] = name
        labelled.add(t)
    }

    fun n(o: Any?): Int = (o as Number).toInt()

    fun errorName(t: Throwable): String = when (t) {
        is Failure -> "Binding"
        is NullPointerException -> "Null"
        is UninitializedPropertyAccessException -> "Uninitialized"
        is NumberFormatException -> "Number"
        is ClassCastException -> "Cast"
        is IndexOutOfBoundsException -> "Index"
        is ConcurrentModificationException -> "ConcurrentModification"
        else -> t.javaClass.name
    }

    fun argument(o: Any?): Any = when (o) {
        is Long -> o.toInt()
        is JSONObject -> when {
            o.containsKey("entity") -> entities[n(o["entity"])]
            o.containsKey("long") -> (o["long"] as Number).toLong()
            o.containsKey("event") -> RandomEvents.valueOf(o["event"] as String)
            else -> throw IllegalStateException("argument $o")
        }
        null -> throw IllegalStateException("null argument")
        else -> o
    }

    fun arguments(op: JSONObject): Array<Any> {
        val list = op["args"] as? JSONArray ?: JSONArray()
        return Array(list.size) { argument(list[it]) }
    }

    fun timer(op: JSONObject, key: String = "timer"): RSTimer? {
        val name = op[key] as? String ?: return null
        return labelled.firstOrNull { labels[it] == name }
    }

    fun parse(text: String): JSONObject = JSONParser().parse(text) as JSONObject

    fun book(o: Any?): SpellBook = SpellBook.valueOf(o as String)

    fun event(o: JSONObject): GameEvent = when (o["type"]) {
        "PrayerActivated" -> PrayerActivatedEvent(PrayerType.valueOf(o["prayer"] as String))
        "PrayerDeactivated" -> PrayerDeactivatedEvent(PrayerType.valueOf(o["prayer"] as String))
        "SpellCast" -> SpellCastEvent(book(o["book"]), n(o["spell"]))
        "SpellbookChange" -> SpellbookChangeEvent(book(o["old"]), book(o["new"]), SpellbookChangeSource.valueOf(o["source"] as String))
        "DialogueClose" -> DialogueCloseEvent(DialoguePlugin(o["dialogue"] as String))
        else -> throw IllegalStateException("event $o")
    }

    fun eventClass(name: String): Class<*> = when (name) {
        "PrayerActivated" -> core.api.Event.PrayerActivated
        "PrayerDeactivated" -> core.api.Event.PrayerDeactivated
        "SpellCast" -> core.api.Event.SpellCast
        "SpellbookChange" -> core.api.Event.SpellbookChanged
        "DialogueClose" -> core.api.Event.DialogueClosed
        else -> throw IllegalStateException(name)
    }

    fun setField(t: RSTimer, field: String, v: Any?) {
        when (field) {
            "interval" -> t.runInterval = n(v)
            "next" -> t.nextExecution = n(v)
            "last" -> t.lastExecution = n(v)
            "severity" -> (t as Poison).severity = n(v)
            "source" -> (t as Poison).damageSource = entities[n(v)]
            "hitsLeft" -> (t as Disease).hitsLeft = n(v)
            "ticksRemaining" -> when (t) {
                is PoisonImmunity -> t.ticksRemaining = n(v)
                is DragonFireImmunity -> t.ticksRemaining = n(v)
                is FrozenImmunity -> t.ticksRemaining = n(v)
                else -> throw IllegalStateException(field)
            }
            "applyImmunity" -> (t as Frozen).shouldApplyImmunity = v as Boolean
            "healRemaining" -> (t as HealOverTime).healRemaining = n(v)
            "healPerTick" -> (t as HealOverTime).healPerTick = n(v)
            "paused" -> (t as AntiMacro).paused = v as Boolean
            "nextRandom" -> (t as AntiMacro).nextRandom = (v as? String)?.let { RandomEvents.valueOf(it) }
            "since" -> { val a = v as JSONArray; (t as SkillRestore).ticksSinceLastRestore[n(a[0])] = n(a[1]) }
            "restore" -> { val a = v as JSONArray; (t as SkillRestore).restoreTicks[n(a[0])] = n(a[1]) }
            else -> throw IllegalStateException(field)
        }
    }

    fun fields(t: RSTimer): JSONObject {
        val m = JSONObject()
        when (t) {
            is SkillRestore -> { m["since"] = t.ticksSinceLastRestore.toList(); m["restore"] = t.restoreTicks.toList() }
            is Poison -> {
                val f = Poison::class.java.getDeclaredField("damageSource")
                f.isAccessible = true
                m["source"] = ix(f.get(t)) ?: "uninitialized"
                m["severity"] = t.severity
            }
            is Disease -> m["hitsLeft"] = t.hitsLeft
            is PoisonImmunity -> m["ticksRemaining"] = t.ticksRemaining
            is DragonFireImmunity -> m["ticksRemaining"] = t.ticksRemaining
            is FrozenImmunity -> m["ticksRemaining"] = t.ticksRemaining
            is Frozen -> m["applyImmunity"] = t.shouldApplyImmunity
            is HealOverTime -> { m["healRemaining"] = t.healRemaining; m["healPerTick"] = t.healPerTick }
            is AntiMacro -> { m["paused"] = t.paused; m["nextRandom"] = t.nextRandom?.name }
        }
        return m
    }

    fun describeTimer(t: RSTimer): Any = listOf(t.javaClass.simpleName, t.identifier, t.runInterval, t.nextExecution, t.lastExecution,
        t.isSoft, t.isAuto, t.flags.map { it.name }, t is PersistTimer, fields(t))

    fun result(v: Any?): Any? = when (v) {
        null, is Boolean, is Number, is String -> v
        is RSTimer -> JSONObject().also { it["timer"] = label(v) }
        is RandomEventNPC -> JSONObject().also { it["npc"] = v.label }
        is AntiMacro.CommandArgs -> listOf(v.targetPlayer, v.targetEvent?.name)
        is List<*> -> v.map { if (it is Item) listOf(it.id, it.amount) else result(it) }
        else -> throw IllegalStateException("result ${v.javaClass}")
    }

    fun player(e: Entity): Player = e as Player

    fun run(op: JSONObject): Any? {
        val e = entities[(op["e"] as? Number)?.toInt() ?: 0]
        val cls = op["class"] as? String
        val manager = e.timers
        return when (op["op"]) {
            "set" -> { apply(e, op["fields"] as JSONObject); null }
            "tick" -> { tick = n(op["value"]); null }
            "process" -> { manager.processTimers(); null }
            "run" -> { repeat(n(op["n"])) { tick++; manager.processTimers() }; null }
            "spawn" -> { val t = OPS[cls]!!.spawn(arguments(op)); op["label"]?.let { name(t, it as String) }; t }
            "spawnId" -> { val t = spawnTimer(op["id"] as String, *arguments(op)); if (t != null) op["label"]?.let { name(t, it as String) }; t }
            "register" -> { registerTimer(e, timer(op)); null }
            "start" -> { val t = OPS[cls]!!.spawn(arguments(op)); op["label"]?.let { name(t, it as String) }; registerTimer(e, t); t }
            "getOrStart" -> OPS[cls]!!.getOrStart(e, arguments(op))
            "getOrStartId" -> getOrStartTimer(e, op["id"] as String, *arguments(op))
            "has" -> OPS[cls]!!.has(e)
            "hasId" -> hasTimerActive(e, op["id"] as String)
            "get" -> OPS[cls]!!.get(e)
            "remove" -> { removeTimer(e, timer(op)!!); null }
            "removeClass" -> { OPS[cls]!!.remove(e); null }
            "removeId" -> { removeTimer(e, op["id"] as String); null }
            "save" -> { val root = JSONObject(); manager.saveTimers(root); root.toJSONString() }
            "parse" -> { manager.parseTimers(parse(op["text"] as String)); null }
            "timerSave" -> { val root = JSONObject(); (timer(op) as PersistTimer).save(root, e); root.toJSONString() }
            "timerParse" -> { (timer(op) as PersistTimer).parse(parse(op["text"] as String), e); null }
            "death" -> { manager.onEntityDeath(); null }
            "clear" -> { manager.clearTimers(); null }
            "auto" -> { TimerRegistry.addAutoTimers(e); null }
            "dispatch" -> { e.dispatch(event(op["event"] as JSONObject)); null }
            "hookForeign" -> { e.hook(eventClass(op["event"] as String), ForeignHook(op["name"] as String)); null }
            "logout" -> { val p = player(e); p.logoutListeners.forEach(java.util.function.BiConsumer { _, m -> m.invoke(p) }); null }
            "applyPoison" -> { applyPoison(e, entities[n(op["source"])], n(op["severity"])); null }
            "curePoison" -> { curePoison(e); null }
            "isPoisoned" -> isPoisoned(e)
            "pause" -> { AntiMacro.pause(player(e)); null }
            "unpause" -> { AntiMacro.unpause(player(e)); null }
            "forceEvent" -> { AntiMacro.forceEvent(player(e), (op["event"] as? String)?.let { RandomEvents.valueOf(it) }); null }
            "eventNpc" -> AntiMacro.getEventNpc(player(e))
            "terminate" -> { AntiMacro.terminateEventNpc(player(e)); null }
            "loot" -> AntiMacro.rollEventLoot(player(e))
            "parseArgs" -> if (op.containsKey("command")) AntiMacro.parseCommandArgs(op["text"] as String, op["command"] as String)
                           else AntiMacro.parseCommandArgs(op["text"] as String)
            "field" -> { setField(timer(op)!!, op["field"] as String, op["value"]); null }
            "fieldOf" -> { setField(OPS[cls]!!.get(e)!!, op["field"] as String, op["value"]); null }
            else -> throw IllegalStateException("unknown op ${op["op"]}")
        }
    }

    fun apply(e: Entity, m: JSONObject) {
        val spec = specs[e]!!
        for ((k, v) in m) {
            when (k as String) {
                "static" -> for ((s, l) in v as JSONObject) e.skills.staticLevels[(s as String).toInt()] = n(l)
                "dynamic" -> for ((s, l) in v as JSONObject) e.skills.dynamicLevels[(s as String).toInt()] = n(l)
                "lp" -> e.skills.lifepoints = n(v)
                "lastTrained" -> e.skills.lastTrainedSkill = n(v)
                "lastXpGain" -> e.skills.lastXpGain = n(v)
                "special" -> if (e is Player) e.settings.set(n(v)) else spec[k] = v
                "book" -> if (e !is Player) spec[k] = v else {
                    val f = player(e).spellBookManager.javaClass.getDeclaredField("spellBook")
                    f.isAccessible = true
                    f.setInt(player(e).spellBookManager, n(v))
                }
                "attrs" -> for ((a, x) in v as JSONObject) {
                    if (x == null) e.attributes.remove(a as String)
                    else e.attributes[a as String] = if (x is JSONObject) npcs[n(x["npc"])] else x
                }
                else -> spec[k] = v
            }
        }
    }

    fun entityState(e: Entity): String {
        val m = JSONObject()
        val s = e.skills
        m["lp"] = s.lifepoints
        m["lpIncrease"] = s.lifepointsIncrease
        m["lpUpdate"] = s.lifepointsUpdate
        m["dynamic"] = s.dynamicLevels.toList()
        if (e is Player) {
            m["special"] = e.settings.specialEnergy
            m["book"] = e.spellBookManager.spellBook
            m["logout"] = e.logoutListeners.keys.sorted()
        }
        m["active"] = e.timers.activeTimers.map { label(it) }
        m["pending"] = e.timers.newTimers.map { label(it) }
        m["removing"] = e.timers.toRemoveTimers.map { label(it) }
        val attrs = JSONObject()
        for ((k, v) in e.attributes) attrs[k] = value(v)
        m["attrs"] = attrs
        val hooks = JSONObject()
        for ((c, list) in e.hookMap()) hooks[c.simpleName] = list.map { hookLabel(it) }
        m["hooks"] = hooks
        return m.toJSONString()
    }

    fun evaluate(case: JSONObject): List<Any?> {
        input = case
        entities.clear(); specs.clear(); labels.clear(); labelled.clear(); npcs.clear(); weapons.clear()
        auto = 0; nonSkill = 0; events = ArrayList(); calls = 0; failAt = -1
        tick = (case["tick"] as? Number)?.toInt() ?: 0
        RandomFunction.RANDOM = Rng((case["seed"] as? Number)?.toLong() ?: 0L)
        for (raw in case["entities"] as JSONArray) {
            val spec = raw as JSONObject
            val e: Entity = when (spec["kind"]) { "npc" -> NPC(); "familiar" -> Familiar(); else -> Player() }
            entities.add(e)
            specs[e] = JSONObject()
            for (i in 0 until 24) { e.skills.staticLevels[i] = if (i == Skills.HITPOINTS) 10 else 1; e.skills.dynamicLevels[i] = e.skills.staticLevels[i] }
        }
        for ((i, raw) in (case["entities"] as JSONArray).withIndex()) apply(entities[i], raw as JSONObject)
        val steps = ArrayList<Any?>()
        val previous = HashMap<Int, String>()
        var previousTimers = ""
        for (raw in case["ops"] as JSONArray) {
            val op = raw as JSONObject
            events = ArrayList(); calls = 0
            failAt = (op["failAt"] as? Number)?.toInt() ?: -1
            var value: Any? = null
            var error: String? = null
            try {
                value = result(run(op))
            } catch (t: Throwable) {
                error = errorName(t)
            }
            failAt = -1
            val step = JSONObject()
            step["result"] = value
            step["error"] = error
            step["events"] = events
            step["tick"] = tick
            val states = ArrayList<Any?>()
            for ((i, e) in entities.withIndex()) {
                val text = entityState(e)
                states.add(if (previous[i] == text) null else JSONParser().parse(text))
                previous[i] = text
            }
            step["entities"] = states
            val timers = JSONObject()
            for (t in labelled) timers[labels[t]] = describeTimer(t)
            val text = timers.toJSONString()
            step["timers"] = if (text == previousTimers) null else timers
            previousTimers = text
            steps.add(step)
        }
        return steps
    }

    @JvmStatic fun main(args: Array<String>) {
        val stderr = System.err
        try {
            harness()
        } catch (t: Throwable) {
            t.printStackTrace(stderr)
            System.exit(1)
        }
    }

    fun harness() {
        System.setErr(Traces())
        for (cls in CLASSES) TimerRegistry.registerTimer(Class.forName("core.game.system.timer.impl.$cls").getDeclaredConstructor().newInstance() as RSTimer)
        val reader = BufferedReader(InputStreamReader(System.`in`, StandardCharsets.UTF_8))
        val out = StringBuilder()
        while (true) {
            val line = reader.readLine() ?: break
            if (line.isEmpty()) continue
            val case = JSONParser().parse(line) as JSONObject
            val row = JSONObject()
            row["input"] = case
            row["expected"] = evaluate(case)
            out.append(JSONValue.toJSONString(row)).append('\n')
        }
        System.out.write(out.toString().toByteArray(StandardCharsets.UTF_8))
        System.out.flush()
    }
}
