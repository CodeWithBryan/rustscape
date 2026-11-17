// Scripted content for the dialogue DSL oracle, written against the unmodified
// DialogueLabeller, DialogueBuilderFile/DialogueBuilder and SkillDialogueHandler.
// Each class interprets a JSON script by calling only the original DSL members;
// the native replay (native.rs) interprets the same scripts against the port.
//// oracle/Script.kt
package oracle

import core.api.InputType
import core.game.component.Component
import core.game.dialogue.*
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.game.node.item.Item

@Suppress("UNCHECKED_CAST")
fun ops(raw: Any?): List<List<Any?>> = (raw as List<Any?>).map { it as List<Any?> }
fun int(raw: Any?): Int = (raw as Number).toInt()
fun strs(raw: Any?): Array<String> = (raw as List<*>).map { Oracle.text(it) }.toTypedArray()
fun anim(raw: Any?): FacialExpression = FacialExpression.valueOf(raw as String)
fun flag(o: List<Any?>, i: Int): Boolean = o.size > i && o[i] == true
fun value(raw: Any?): Any? = if (raw is Number) raw.toInt() else if (raw is List<*>) Oracle.text(raw) else raw

/** Conditions: a constant, a player attribute, the NPC's id or a negation. */
fun cond(raw: Any?, player: Player, npc: NPC?): Boolean {
    if (raw is Boolean) return raw
    val m = raw as Map<*, *>
    if (m.containsKey("attr")) return player.getAttribute<Boolean>(m["attr"] as String, false)
    if (m.containsKey("npc")) return npc!!.id == int(m["npc"])
    if (m.containsKey("not")) return !cond(m["not"], player, npc)
    throw AssertionError(m)
}

class ScriptLabeller(val spec: List<Any?>) : DialogueLabeller() {
    override fun addConversation() { run(spec) }

    fun target(raw: Any?): Any {
        val m = raw as Map<*, *>
        if (m.containsKey("key")) return int(m["key"])
        return Oracle.newFile(int(m["file"]))
    }

    fun sub(list: Any?, player: Player, npc: NPC?): Component? {
        var component: Component? = null
        for (o in ops(list)) {
            when (o[0] as String) {
                "loadLabel" -> loadLabel(player, o[1] as String)
                "goto" -> goto(o[1] as String)
                "emit" -> Rec.emit("cb", o[1], player.oid, npc?.oid)
                "setAttr" -> player.setAttribute(o[1] as String, value(o[2]))
                "if" -> component = if (cond(o[1], player, npc)) sub(o[2], player, npc) else sub(o[3], player, npc)
                "fail" -> throw Rec.Failure()
                "openDialogue" -> openDialogue(player, target(o[1]))
                "send" -> component = interpreter!!.sendDialogue(*strs(o[1]))
                "emitInput" -> Rec.emit("input", Describe.value(optInput), optButton)
                else -> throw AssertionError(o)
            }
        }
        return component
    }

    fun option(raw: Any?): DialogueOption {
        val m = raw as Map<*, *>
        val text = Oracle.text(m["option"])
        val spoken = if (m["spoken"] != null) Oracle.text(m["spoken"]) else text
        val expression = if (m["expr"] != null) anim(m["expr"]) else ChatAnim.NEUTRAL
        val condition = m["if"]
        return DialogueOption(m["goto"] as String, text, spoken, expression, m["skip"] == true,
            if (condition == null) null else { p: Player, n: NPC -> cond(condition, p, n) })
    }

    fun run(list: Any?) {
        for (o in ops(list)) {
            when (o[0] as String) {
                "label" -> label(*strs(o[1])) { if (o.size > 2) run(o[2]) }
                "goto" -> goto(o[1] as String)
                "assign" -> assignToIds(int(o[1]))
                "npc" -> npc(anim(o[1]), *strs(o[2]), unclosable = flag(o, 3))
                "npcPlain" -> npc(*strs(o[1]))
                "npcAs" -> npc(anim(o[1]), int(o[2]), *strs(o[3]), unclosable = flag(o, 4))
                "npcId" -> npc(int(o[1]), *strs(o[2]), unclosable = flag(o, 3))
                "player" -> player(anim(o[1]), *strs(o[2]), unclosable = flag(o, 3))
                "playerPlain" -> player(*strs(o[1]))
                "item" -> item(Item.all[int(o[1])], *strs(o[2]), message = if (o[3] == null) "" else Oracle.text(o[3]), unclosable = flag(o, 4))
                "item2" -> item(Item.all[int(o[1])], Item.all[int(o[2])], *strs(o[3]), message = if (o[4] == null) "" else Oracle.text(o[4]), unclosable = flag(o, 5))
                "line" -> line(*strs(o[1]), unclosable = flag(o, 2))
                "options" -> {
                    val list = (o[1] as List<*>).map { option(it) }.toTypedArray()
                    if (o[2] == null) options(*list, unclosable = flag(o, 3)) else options(*list, title = Oracle.text(o[2]), unclosable = flag(o, 3))
                }
                "input" -> if (o[2] == null) input(InputType.valueOf(o[1] as String)) else input(InputType.valueOf(o[1] as String), Oracle.text(o[2]))
                "inputBool" -> if (o[2] == null) input(o[1] == true) else input(o[1] == true, Oracle.text(o[2]))
                "exec" -> exec { p, n -> sub(o[1], p, n) }
                "manual" -> manual(flag(o, 1)) { p, n -> sub(o[2], p, n) }
                "afterClose" -> afterClose { p -> sub(o[1], p, null) }
                "open" -> {
                    val t = o[1] as Map<*, *>
                    if (t["npc"] == true) open(player!!, target(t), npc!!) else open(player!!, target(t))
                }
                else -> throw AssertionError(o)
            }
        }
    }
}

class ScriptBuilder : DialogueBuilderFile() {
    companion object {
        /** The spec create() reads: the base constructor runs it before any subclass field exists. */
        @JvmField var pending: Map<*, *>? = null
    }

    override fun create(b: DialogueBuilder) {
        val spec = pending!!
        val holders = HashMap<String, PlaceholderNode>()
        if (spec["bare"] != null) chain(b, spec["bare"], holders)
        for (raw in spec["clauses"] as List<*>) {
            val clause = raw as Map<*, *>
            val w = clause["when"]
            val builder = if (w == "default") b.defaultDialogue()
                else if (w is Map<*, *> && w.containsKey("quest")) b.onQuestStages(content.data.Quests.valueOf(w["quest"] as String), *(w["stages"] as List<*>).map { int(it) }.toIntArray())
                else b.onPredicate { p -> cond((w as Map<*, *>)["cond"], p, null) }
            chain(builder, clause["chain"], holders)
        }
    }

    fun callback(list: Any?, df: DialogueFile, player: Player, cid: Int?, bid: Int?) {
        for (o in ops(list)) {
            when (o[0] as String) {
                "emit" -> Rec.emit("cb", o[1], player.oid, cid, bid)
                "stage" -> df.stage = int(o[1])
                "npcl" -> df.npcl(anim(o[1]), Oracle.text(o[2]))
                "send" -> df.interpreter!!.sendDialogue(*strs(o[1]))
                "end" -> df.end()
                "setAttr" -> player.setAttribute(o[1] as String, value(o[2]))
                "fail" -> throw Rec.Failure()
                else -> throw AssertionError(o)
            }
        }
    }

    fun chain(start: DialogueBuilder, list: Any?, holders: HashMap<String, PlaceholderNode>) {
        var b = start
        for (o in ops(list)) {
            when (o[0] as String) {
                "npcl" -> b = if (o[1] == null) b.npcl(Oracle.text(o[2])) else b.npcl(anim(o[1]), Oracle.text(o[2]))
                "npc" -> b = if (o[1] == null) b.npc(*strs(o[2])) else b.npc(anim(o[1]), *strs(o[2]))
                "playerl" -> b = if (o[1] == null) b.playerl(Oracle.text(o[2])) else b.playerl(anim(o[1]), Oracle.text(o[2]))
                "player" -> b = if (o[1] == null) b.player(*strs(o[2])) else b.player(anim(o[1]), *strs(o[2]))
                "item" -> b = b.item(int(o[1]), *strs(o[2]))
                "iteml" -> b = b.iteml(int(o[1]), Oracle.text(o[2]))
                "line" -> b = b.line(*strs(o[1]))
                "linel" -> b = b.linel(Oracle.text(o[1]))
                "between" -> b = b.betweenStage { df, p, c, i -> callback(o[1], df, p, c, i) }
                "manual" -> b = b.manualStage { df, p, c, i -> callback(o[1], df, p, c, i) }
                "manualGoto" -> b = b.manualStageWithGoto({ df, p, c, i, s ->
                    callback(o[1], df, p, c, i)
                    val r = o[2]
                    if (r is Map<*, *>) s + int(r["plus"]) else int(r)
                }, { it })
                "end" -> b.end()
                "endWith" -> b.endWith { df, p -> callback(o[1], df, p, null, null) }
                "options" -> {
                    var ob = if (o[1] == null) b.options() else b.options(Oracle.text(o[1]))
                    if (o[2] != null) ob = ob.recordAttribute(o[2] as String)
                    for (raw in o[3] as List<*>) {
                        val e = raw as List<*>
                        val text = Oracle.text(e[1])
                        val next = when (e[0] as String) {
                            "option" -> ob.option(text)
                            "optionIf" -> ob.optionIf(text) { p -> cond(e[2], p, null) }
                            "playerl" -> ob.option_playerl(text)
                            else -> throw AssertionError(e)
                        }
                        chain(next, e[3], holders)
                    }
                }
                "placeholder" -> holders[o[1] as String] = b.placeholder()
                "builder" -> chain(holders[o[1] as String]!!.builder(), o[2], holders)
                "goto" -> b = b.goto(holders[o[1] as String]!!)
                "branch" -> {
                    val key = (o[1] as Map<*, *>)["attrInt"] as String
                    val bb = b.branch { p -> p.getAttribute<Int>(key, 0) }
                    for (raw in o[2] as List<*>) { val e = raw as List<*>; chain(bb.onValue(int(e[0])), e[1], holders) }
                }
                "branchBool" -> {
                    val bb = b.branchBoolAttribute(o[1] as String, o[2] == true)
                    for (raw in o[3] as List<*>) { val e = raw as List<*>; chain(bb.onValue(e[0] == true), e[1], holders) }
                }
                else -> throw AssertionError(o)
            }
        }
    }
}

/** A SkillDialogueHandler subclass: create always records; getAll and getName optionally overridden. */
class ScriptSkill(player: Player, type: SkillDialogueHandler.SkillDialogue?, val spec: Map<*, *>, val label: String, vararg items: Item) :
    SkillDialogueHandler(player, type, *items) {
    override fun create(amount: Int, index: Int) {
        Rec.emit("create", label, amount, index)
        if (spec["createFails"] == true) throw Rec.Failure()
    }
    override fun getAll(index: Int): Int {
        if (spec["getAll"] == null) return super.getAll(index)
        Rec.emit("getAll", label, index)
        return int(spec["getAll"])
    }
    override fun getName(item: Item): String {
        if (spec["getName"] != true) return super.getName(item)
        Rec.emit("getName", label, item.ref)
        return "Custom " + item.id
    }
}
