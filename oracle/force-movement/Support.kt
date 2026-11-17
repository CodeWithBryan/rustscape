package oracle

import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.impl.Animator.Priority
import core.game.node.entity.impl.ForceMovement
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.game.system.task.Pulse
import core.game.world.map.Direction
import core.game.world.map.Location
import core.game.world.update.flag.context.Animation
import core.game.world.update.flag.context.ForceMoveCtx
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStream
import java.io.PrintStream
import java.nio.charset.StandardCharsets
import java.util.IdentityHashMap
import org.json.simple.JSONArray
import org.json.simple.JSONObject
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser

/** Records every stack trace Pulse.update prints (its first line is the exception). */
class Traces : PrintStream(object : OutputStream() { override fun write(b: Int) {} }) {
    override fun println(x: Any?) {
        if (x is Throwable) Support.note("exception", Support.errorName(x))
    }
}

object Support {
    class Failure : RuntimeException()

    lateinit var input: JSONObject
    var events = ArrayList<Any?>()
    var calls = 0
    var failAt = -1
    val entities = ArrayList<Entity>()
    val specs = IdentityHashMap<Any, JSONObject>()
    val anims = ArrayList<Animation>()
    val movements = ArrayList<ForceMovement>()

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

    @JvmStatic fun flag(e: Any?, key: String, fallback: Boolean): Boolean = specs[e]!![key] as? Boolean ?: fallback

    @JvmStatic fun location(e: Any?): Location? = place(specs[e]!!["location"])

    fun place(v: Any?): Location? {
        val a = v as? JSONArray ?: return null
        return Location(n(a[0]), n(a[1]), n(a[2]))
    }

    @JvmStatic fun loc(l: Location?): Any? = l?.let { listOf(it.x, it.y, it.z) }

    fun animLabel(a: Animation?): String? = when {
        a == null -> null
        a === ForceMovement.WALK_ANIMATION -> "walk"
        else -> anims.indexOfFirst { it === a }.let { if (it < 0) throw IllegalStateException("unknown animation") else "a$it" }
    }

    @JvmStatic fun anim(a: Animation?): Any? = a?.let { listOf(animLabel(it), it.id, it.priority.name) }

    @JvmStatic fun context(c: Any?): Any? {
        val ctx = c as ForceMoveCtx
        return listOf(loc(ctx.start), loc(ctx.dest), ctx.startArrive, ctx.destArrive, ctx.direction.name)
    }

    /** A movement is labelled at its first sight (a static run submits it before returning it). */
    @JvmStatic fun label(p: Pulse): String {
        if (movements.none { it === p }) movements.add(p as ForceMovement)
        return "f" + movements.indexOfFirst { it === p }
    }

    fun n(o: Any?): Int = (o as Number).toInt()

    fun errorName(t: Throwable): String = when (t) {
        is Failure -> "Binding"
        is NullPointerException -> "Null"
        is ClassCastException -> "Cast"
        is IndexOutOfBoundsException -> "Index"
        else -> t.javaClass.name + ": " + t.message
    }

    fun entity(op: JSONObject, key: String = "e"): Entity? = (op[key] as? Number)?.let { entities[it.toInt()] }
    fun animation(v: Any?): Animation? = when (v) {
        null -> null
        "walk" -> ForceMovement.WALK_ANIMATION
        else -> anims[(v as String).substring(1).toInt()]
    }
    fun direction(v: Any?): Direction? = (v as? String)?.let { Direction.valueOf(it) }
    fun movement(op: JSONObject): ForceMovement = movements[n(op["fm"])]

    fun keep(fm: ForceMovement): String {
        if (movements.none { it === fm }) movements.add(fm)
        return label(fm)
    }

    fun construct(op: JSONObject): ForceMovement {
        val e = entity(op)
        val start = place(op["start"])
        val dest = place(op["dest"])
        val startAnim = animation(op["startAnim"])
        val anim = animation(op["anim"])
        val dir = direction(op["direction"])
        return when (op["form"]) {
            "full" -> ForceMovement(e, start, dest, startAnim, anim, dir, n(op["commence"]), n(op["path"]), op["unlock"] as Boolean)
            "speed" -> ForceMovement(e, start, dest, anim, n(op["speed"]))
            "speeds" -> ForceMovement(e, dest, n(op["commence"]), n(op["path"]))
            "animation" -> ForceMovement(e, start, dest, anim)
            "detached" -> ForceMovement(start, dest, anim)
            else -> throw IllegalStateException("form ${op["form"]}")
        }
    }

    fun runStatic(op: JSONObject): ForceMovement {
        val e = entity(op)
        val start = place(op["start"])
        val dest = place(op["dest"])
        val startAnim = animation(op["startAnim"])
        val anim = animation(op["anim"])
        val dir = direction(op["direction"])
        // The movement is labelled when it is submitted (or here, when run returns).
        return when (op["form"]) {
            "r2" -> ForceMovement.run(e, dest)
            "r3" -> ForceMovement.run(e, start, dest)
            "r4" -> ForceMovement.run(e, start, dest, anim)
            "r5" -> ForceMovement.run(e, start, dest, anim, n(op["speed"]))
            "r5a" -> ForceMovement.run(e, start, dest, startAnim, anim)
            "r6" -> ForceMovement.run(e, start, dest, startAnim, anim, dir)
            "r7" -> ForceMovement.run(e, start, dest, startAnim, anim, dir, n(op["path"]))
            "r8" -> ForceMovement.run(e, start, dest, startAnim, anim, dir, n(op["commence"]), n(op["path"]))
            "r9" -> ForceMovement.run(e, start, dest, startAnim, anim, dir, n(op["commence"]), n(op["path"]), op["unlock"] as Boolean)
            "rSpeeds" -> ForceMovement.run(e, dest, n(op["commence"]), n(op["path"]))
            else -> throw IllegalStateException("form ${op["form"]}")
        }
    }

    fun run(op: JSONObject): Any? = when (op["op"]) {
        "set" -> { apply(entities[n(op["e"])], op["fields"] as JSONObject); null }
        "construct" -> keep(construct(op))
        "run" -> keep(runStatic(op))
        "runWith" -> { movement(op).run(entity(op), n(op["speed"])); null }
        "runEntity" -> { movement(op).run(entity(op)); null }
        "runSelf" -> { movement(op).run(); null }
        "start" -> { movement(op).start(); null }
        "pulse" -> movement(op).pulse()
        "update" -> movement(op).update()
        "updates" -> (1..n(op["n"])).map { movement(op).update() }
        "stop" -> { movement(op).stop(); null }
        "direction" -> ForceMovement.direction(place(op["start"]), place(op["dest"])).name
        "setStart" -> { movement(op).start = place(op["value"]); null }
        "setDestination" -> { movement(op).destination = place(op["value"]); null }
        "setDirection" -> { movement(op).direction = direction(op["value"]); null }
        "setCommenceSpeed" -> { movement(op).commenceSpeed = n(op["value"]); null }
        "setPathSpeed" -> { movement(op).pathSpeed = n(op["value"]); null }
        "setEntity" -> { movement(op).setEntity(entity(op, "value")); null }
        "setStartAnim" -> { movement(op).startAnim = animation(op["value"]); null }
        "setAnimation" -> { movement(op).animation = animation(op["value"]); null }
        "setEndAnimation" -> { movement(op).endAnimation = animation(op["value"]); null }
        "get" -> {
            val fm = movement(op)
            when (op["field"]) {
                "start" -> loc(fm.start)
                "destination" -> loc(fm.destination)
                "direction" -> fm.direction?.name
                "commenceSpeed" -> fm.commenceSpeed
                "pathSpeed" -> fm.pathSpeed
                "entity" -> ix(fm.entity)
                "startAnim" -> animLabel(fm.startAnim)
                "animation" -> animLabel(fm.animation)
                "endAnimation" -> animLabel(fm.endAnimation)
                else -> throw IllegalStateException("field ${op["field"]}")
            }
        }
        else -> throw IllegalStateException("unknown op ${op["op"]}")
    }

    fun apply(e: Any, m: JSONObject) {
        val spec = specs[e]!!
        for ((k, v) in m) spec[k] = v
    }

    fun field(o: Any, cls: Class<*>, name: String): Any? {
        val f = cls.getDeclaredField(name)
        f.isAccessible = true
        return f.get(o)
    }

    fun describe(fm: ForceMovement): Any {
        val c = ForceMovement::class.java
        @Suppress("UNCHECKED_CAST")
        val checks = field(fm, Pulse::class.java, "checks") as Array<Node?>?
        return listOf(ix(fm.entity), loc(fm.start), loc(fm.destination), animLabel(fm.startAnim), animLabel(fm.animation),
            animLabel(fm.endAnimation), fm.direction?.name, fm.commenceSpeed, fm.pathSpeed, field(fm, c, "unlockAfter"), fm.delay,
            fm.ticksPassed, fm.isRunning, checks?.map { ix(it) })
    }

    fun state(): String {
        val m = JSONObject()
        val a = JSONObject()
        a["walk"] = listOf(ForceMovement.WALK_ANIMATION.id, ForceMovement.WALK_ANIMATION.priority.name)
        for ((i, x) in anims.withIndex()) a["a$i"] = listOf(x.id, x.priority.name)
        m["anims"] = a
        m["movements"] = movements.map { describe(it) }
        return m.toJSONString()
    }

    fun evaluate(case: JSONObject): List<Any?> {
        input = case
        events = ArrayList(); calls = 0; failAt = -1
        entities.clear(); specs.clear(); anims.clear(); movements.clear()
        // WALK_ANIMATION is one static object: each case starts from its initialiser's priority.
        ForceMovement.WALK_ANIMATION.priority = Priority.MID
        for (raw in case["entities"] as JSONArray) {
            val spec = raw as JSONObject
            val e: Entity = if (spec["kind"] == "npc") NPC() else Player()
            entities.add(e)
            specs[e] = JSONObject(spec)
        }
        for (raw in (case["anims"] as? JSONArray) ?: JSONArray()) {
            val a = raw as JSONArray
            anims.add(Animation(n(a[0]), Priority.valueOf(a[1] as String)))
        }
        val steps = ArrayList<Any?>()
        var previous = ""
        for (raw in case["ops"] as JSONArray) {
            val op = raw as JSONObject
            events = ArrayList(); calls = 0
            failAt = (op["failAt"] as? Number)?.toInt() ?: -1
            var value: Any? = null
            var error: String? = null
            try {
                value = run(op)
            } catch (t: Throwable) {
                error = errorName(t)
            }
            failAt = -1
            val step = JSONObject()
            step["result"] = value
            step["error"] = error
            step["events"] = events
            val text = state()
            step["state"] = if (text == previous) null else JSONParser().parse(text)
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
        System.setErr(Traces())
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

