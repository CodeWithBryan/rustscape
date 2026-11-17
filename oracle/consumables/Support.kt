package oracle

import content.data.consumables.Consumables
import content.data.consumables.effects.*
import content.global.handlers.item.ConsumableListener
import core.game.consumable.*
import core.game.interaction.QueueStrength
import core.game.interaction.QueuedScript
import core.game.interaction.Script
import core.game.node.Node
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.diary.DiaryType
import core.game.node.item.Item
import core.game.system.task.Pulse
import core.game.system.timer.RSTimer
import core.game.system.timer.impl.Disease
import core.tools.RandomFunction
import java.io.BufferedReader
import java.io.InputStreamReader
import java.lang.reflect.Modifier
import java.nio.charset.StandardCharsets
import java.util.IdentityHashMap
import java.util.Random
import org.json.simple.JSONArray
import org.json.simple.JSONObject
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser

object Support {
    class Failure : RuntimeException()

    class Rng(seed: Long) : Random(seed) {
        override fun nextInt(bound: Int): Int {
            val v = super.nextInt(bound)
            emit("next_int", bound, v)
            return v
        }
    }

    lateinit var input: JSONObject
    var spec = JSONObject()
    var events = ArrayList<Any?>()
    var calls = 0
    var failAt = -1
    var tick = 0
    lateinit var player: Player
    @JvmField val timers = ArrayList<RSTimer>()
    val timerLabels = IdentityHashMap<RSTimer, String>()
    val pulses = ArrayList<Pulse>()
    @JvmField val queued = ArrayList<Script<*>>()
    @JvmField var handler: ((Player, Node) -> Boolean)? = null

    @JvmStatic fun emit(name: String, vararg args: Any?) {
        val row = ArrayList<Any?>()
        row.add(name)
        row.addAll(args)
        events.add(row)
        calls++
        if (calls == failAt) throw Failure()
    }

    @JvmStatic fun ticks(): Int {
        emit("ticks", tick)
        return tick
    }

    @JvmStatic fun ix(o: Any?): Int? = when {
        o == null -> null
        o === player -> 0
        else -> throw IllegalStateException("unknown entity")
    }

    @JvmStatic fun itemName(id: Int): String = ((input["names"] as? JSONObject)?.get(id.toString()) as? String) ?: "null"

    @JvmStatic fun flag(key: String): Boolean = spec[key] as? Boolean ?: false

    @JvmStatic fun value(v: Any?): Any? = when (v) {
        null, is Boolean, is Number, is String -> v
        else -> v.javaClass.simpleName
    }

    @JvmStatic fun describe(item: Item?): Any? = item?.let { listOf(it.id, it.amount, it.slot) }

    @JvmStatic fun label(t: RSTimer?): String? {
        if (t == null) return null
        return timerLabels.getOrPut(t) { "t${timerLabels.size}" }
    }

    @JvmStatic fun spawn(identifier: String, args: List<Any?>): RSTimer? {
        val unknown = input["unknownTimers"] as? JSONArray
        if (unknown != null && unknown.contains(identifier)) return null
        val t = if (identifier == "disease") Disease() else RSTimer(identifier, args.map { value(it) })
        label(t)
        return t
    }

    @JvmStatic fun runPulse(pulse: Pulse) {
        if (pulses.none { it === pulse }) pulses.add(pulse)
        emit("run_pulse", pulseLabel(pulse), pulse.delay)
    }

    fun pulseLabel(p: Pulse): String = "p" + pulses.indexOfFirst { it === p }

    @JvmStatic fun activeScript(): Script<*>? {
        val active = spec["active"] as? Boolean ?: return null
        return QueuedScript({ true }, QueueStrength.WEAK, active)
    }

    fun n(o: Any?): Int = (o as Number).toInt()
    fun d(o: Any?): Double = (o as Number).toDouble()

    fun errorName(t: Throwable): String = when (t) {
        is Failure -> "Binding"
        is NullPointerException -> "Null"
        is ClassCastException -> "Cast"
        is IndexOutOfBoundsException -> "Index"
        is NumberFormatException -> "Number"
        else -> t.javaClass.name + ": " + t.message
    }

    fun argument(o: Any?): Any? = when (o) {
        is Long -> o.toInt()
        else -> o
    }

    /** One effect object of the case, built with the named class's own constructor. */
    fun effect(o: Any?): ConsumableEffect {
        val m = o as JSONObject
        val a = (m["args"] as? JSONArray) ?: JSONArray()
        return when (m["class"]) {
            "AchievementEffect" -> AchievementEffect(DiaryType.valueOf(a[0] as String), n(a[1]), n(a[2]))
            "AddTimerEffect" -> AddTimerEffect(a[0] as String, *a.drop(1).map { argument(it)!! }.toTypedArray())
            "CureDiseaseEffect" -> CureDiseaseEffect()
            "DamageEffect" -> if (a.size == 2) DamageEffect(d(a[0]), a[1] as Boolean) else DamageEffect(d(a[0]), a[1] as Boolean, n(a[2]))
            "DraynorCabbageEffect" -> DraynorCabbageEffect()
            "DwarvenRockCakeEffect" -> DwarvenRockCakeEffect()
            "EnergyEffect" -> EnergyEffect(n(a[0]))
            "HealingEffect" -> HealingEffect(n(a[0]))
            "KebabEffect" -> KebabEffect()
            "KegOfBeerEffect" -> KegOfBeerEffect()
            "MatureWmbEffect" -> MatureWmbEffect()
            "MultiEffect" -> MultiEffect(*a.map { effect(it) }.toTypedArray())
            "PercentageHealthEffect" -> PercentageHealthEffect(n(a[0]))
            "PercentHeal" -> PercentHeal(n(a[0]), d(a[1]))
            "PoisonEffect" -> PoisonEffect(n(a[0]))
            "PoisonKarambwanEffect" -> PoisonKarambwanEffect()
            "PrayerEffect" -> if (a.size == 2) PrayerEffect(d(a[0]), d(a[1])) else PrayerEffect(d(a[0]), d(a[1]), a[2] as Boolean)
            "RandomEnergyEffect" -> RandomEnergyEffect(n(a[0]), n(a[1]))
            "RandomHealthEffect" -> RandomHealthEffect(n(a[0]), n(a[1]))
            "RandomPrayerEffect" -> RandomPrayerEffect(n(a[0]), n(a[1]))
            "RandomSkillEffect" -> RandomSkillEffect(n(a[0]), n(a[1]), n(a[2]))
            "RemoveTimerEffect" -> RemoveTimerEffect(a[0] as String)
            "RestoreEffect" -> if (a.size == 2) RestoreEffect(d(a[0]), d(a[1])) else RestoreEffect(d(a[0]), d(a[1]), a[2] as Boolean)
            "RestoreSummoningSpecial" -> RestoreSummoningSpecial()
            "RockCakeEffect" -> RockCakeEffect()
            "SetAttributeEffect" -> if (a.size == 2) SetAttributeEffect(a[0] as String, argument(a[1])) else SetAttributeEffect(a[0] as String, argument(a[1]), a[2] as Boolean)
            "SkillEffect" -> SkillEffect(n(a[0]), d(a[1]), d(a[2]))
            "SmellingUgthankiKebabEffect" -> SmellingUgthankiKebabEffect()
            "SummoningEffect" -> SummoningEffect(d(a[0]), d(a[1]))
            "SuperKebabEffect" -> SuperKebabEffect()
            "TroubleBrewingRumEffect" -> TroubleBrewingRumEffect(a[0] as String)
            "UgthankiKebabEffect" -> UgthankiKebabEffect()
            "WizardsMindBombEffect" -> WizardsMindBombEffect()
            "ZamorakMixEffect" -> ZamorakMixEffect()
            else -> throw IllegalStateException("effect $m")
        }
    }

    /** An effect object's own fields (sorted by name), read by reflection. */
    fun describeEffect(e: Any?): Any? {
        if (e == null) return null
        val m = JSONObject()
        m["class"] = e.javaClass.simpleName
        for (f in e.javaClass.declaredFields.sortedBy { it.name }) {
            if (Modifier.isStatic(f.modifiers)) continue
            f.isAccessible = true
            m[f.name] = fieldValue(f.get(e))
        }
        return m
    }

    fun fieldValue(v: Any?): Any? = when (v) {
        null, is Boolean, is Number, is String -> v
        is ConsumableEffect -> describeEffect(v)
        is IntArray -> v.toList()
        is Array<*> -> v.map { fieldValue(it) }
        is Enum<*> -> v.name
        else -> throw IllegalStateException("field ${v.javaClass}")
    }

    fun field(o: Any, cls: Class<*>, name: String): Any? {
        val f = cls.getDeclaredField(name)
        f.isAccessible = true
        return f.get(o)
    }

    fun table(): Any {
        val rows = JSONArray()
        for (row in Consumables.values()) {
            val c = row.consumable
            val r = JSONArray()
            r.add(row.name)
            r.add(c.javaClass.simpleName)
            r.add(c.ids.toList())
            r.add(row.isIgnoreMainClock)
            r.add((field(c, Consumable::class.java, "animation") as? core.game.world.update.flag.context.Animation)?.id)
            r.add((field(c, Consumable::class.java, "messages") as Array<*>).toList())
            r.add(describeEffect(c.effect))
            rows.add(r)
        }
        return rows
    }

    fun node(op: JSONObject): Node {
        (op["slot"] as? Number)?.let { return player.inventory.items[it.toInt()] ?: throw IllegalStateException("empty slot") }
        (op["other"] as? Number)?.let { return OtherNode(it.toInt()) }
        val a = op["item"] as JSONArray
        val item = Item(n(a[0]), n(a[1]))
        item.index = n(a[2])
        return item
    }

    fun row(op: JSONObject): Consumable = Consumables.valueOf(op["row"] as String).consumable

    fun result(v: Any?): Any? = when (v) {
        null, is Boolean, is Number, is String -> v
        is JSONObject -> v
        is List<*> -> v.map { result(it) }
        else -> throw IllegalStateException("result ${v.javaClass}")
    }

    fun run(op: JSONObject): Any? = when (op["op"]) {
        "set" -> { apply(op["fields"] as JSONObject); null }
        "tick" -> { tick = n(op["value"]); null }
        "define" -> { ConsumableListener().defineListeners(); null }
        "handle" -> if (op["slot"] is Number && player.inventory.items[n(op["slot"])] == null) "empty slot" else handler!!.invoke(player, node(op))
        "consume" -> { row(op).consume(node(op) as Item, player); null }
        "health" -> row(op).getHealthEffectValue(player)
        "formatted" -> row(op).getFormattedName(node(op) as Item)
        "dose" -> (row(op) as Potion).getDose(node(op) as Item)
        "lookup" -> Consumables.getConsumableById(n(op["id"]))?.name
        "table" -> table()
        "potions" -> Consumables.potions.toList()
        "effect" -> { effect(op["effect"]).activate(player); null }
        "effectHealth" -> effect(op["effect"]).getHealthEffectValue(player)
        "describe" -> describeEffect(effect(op["effect"]))
        "script" -> {
            @Suppress("UNCHECKED_CAST")
            val s = queued[n(op["index"])] as Script<(Int) -> Boolean>
            s.execution.invoke(n(op["stage"] ?: 0L))
        }
        "pulse" -> pulses[n(op["index"])].pulse()
        else -> throw IllegalStateException("unknown op ${op["op"]}")
    }

    fun item(a: Any?, slot: Int): Item {
        val r = a as JSONArray
        val i = Item(n(r[0]), n(r[1]))
        i.index = slot
        return i
    }

    fun apply(m: JSONObject) {
        val s = player.skills
        for ((k, v) in m) {
            when (k as String) {
                "static" -> for ((i, l) in v as JSONObject) s.staticLevels[(i as String).toInt()] = n(l)
                "dynamic" -> for ((i, l) in v as JSONObject) s.dynamicLevels[(i as String).toInt()] = n(l)
                "lp" -> s.lifepoints = n(v)
                "prayer" -> s.setPrayer(d(v))
                "energy" -> player.settings.setRunEnergy(d(v))
                "clocks" -> for ((i, c) in v as JSONObject) player.clocks[(i as String).toInt()] = n(c)
                "attrs" -> for ((a, x) in v as JSONObject) {
                    if (x == null) player.attributes.remove(a as String) else player.attributes[a as String] = argument(x)
                }
                "inventory" -> for (r in v as JSONArray) {
                    val slot = n((r as JSONArray)[0])
                    player.inventory.items[slot] = if (r[1] == null) null else item(JSONArray().also { it.add(r[1]); it.add(r[2]) }, slot)
                }
                "equipment" -> for (r in v as JSONArray) {
                    val slot = n((r as JSONArray)[0])
                    player.equipment.items[slot] = if (r[1] == null) null else item(JSONArray().also { it.add(r[1]); it.add(r[2]) }, slot)
                }
                "timers" -> for (r in v as JSONArray) {
                    val a = r as JSONArray
                    val t = if (a[0] == "disease") Disease().also { if (a.size > 1) it.hitsLeft = n(a[1]) } else RSTimer(a[0] as String, emptyList())
                    label(t)
                    timers.add(t)
                }
                else -> spec[k] = v
            }
        }
    }

    fun state(): String {
        val m = JSONObject()
        val s = player.skills
        m["lp"] = s.lifepoints
        m["lpIncrease"] = s.lifepointsIncrease
        m["lpUpdate"] = s.lifepointsUpdate
        m["dynamic"] = s.dynamicLevels.toList()
        m["prayer"] = s.prayerPoints
        m["energy"] = player.settings.energy
        m["clocks"] = player.clocks.toList()
        val attrs = JSONObject()
        for ((k, v) in player.attributes) attrs[k] = value(v)
        m["attrs"] = attrs
        for (c in listOf(player.inventory, player.equipment)) {
            val rows = JSONArray()
            for ((i, item) in c.items.withIndex()) if (item != null) rows.add(listOf(i, item.id, item.amount, item.slot))
            m[c.label] = rows
        }
        m["timers"] = timers.map { listOf(label(it), it.identifier, it.args, (it as? Disease)?.hitsLeft) }
        m["queued"] = queued.map { listOf((it as QueuedScript).strength.name, it.nextExecution, it.persist) }
        m["pulses"] = pulses.map { listOf(pulseLabel(it), it.delay, it.isRunning) }
        return m.toJSONString()
    }

    fun evaluate(case: JSONObject): List<Any?> {
        input = case
        spec = JSONObject()
        events = ArrayList(); calls = 0; failAt = -1
        timers.clear(); timerLabels.clear(); pulses.clear(); queued.clear(); handler = null
        tick = (case["tick"] as? Number)?.toInt() ?: 0
        RandomFunction.RANDOM = Rng((case["seed"] as? Number)?.toLong() ?: 0L)
        player = Player()
        for (i in 0 until 24) { player.skills.staticLevels[i] = if (i == 3) 10 else 1; player.skills.dynamicLevels[i] = player.skills.staticLevels[i] }
        apply(case["player"] as? JSONObject ?: JSONObject())
        ConsumableListener().defineListeners()
        events = ArrayList(); calls = 0
        val steps = ArrayList<Any?>()
        var previous = ""
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
            val text = state()
            step["player"] = if (text == previous) null else JSONParser().parse(text)
            previous = text
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
