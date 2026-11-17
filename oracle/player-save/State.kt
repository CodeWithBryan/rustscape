// Harness for the persisted state of the systems behind save::Host. It scripts
// inputs, calls the unmodified original classes and reads their state back
// through their own accessors; it holds no model of what they should contain.
package oracle

import content.data.Quests
import content.global.skill.construction.CrestType
import content.global.skill.construction.Hotspot
import content.global.skill.construction.HouseLocation
import content.global.skill.construction.HousingStyle
import content.global.skill.construction.Room
import content.global.skill.construction.Servant
import content.global.skill.construction.decoration.pohstorage.StorableFamily
import content.global.skill.summoning.familiar.BurdenBeast
import content.global.skill.summoning.familiar.Familiar
import content.global.skill.summoning.pet.Pet
import content.global.skill.summoning.pet.PetDetails
import core.ServerConstants
import core.api.PersistPlayer
import core.cache.def.impl.ItemDefinition
import core.game.container.Container
import core.game.event.AttributeRemoveEvent
import core.game.event.AttributeSetEvent
import core.game.event.Event
import core.game.node.entity.combat.graves.GraveController
import core.game.node.entity.combat.graves.GraveType
import core.game.node.entity.combat.spell.CombatSpell
import core.game.node.entity.combat.spell.MagicSpell
import core.game.node.entity.player.Player
import core.game.node.entity.player.info.login.PlayerSaveParser
import core.game.node.entity.player.info.login.PlayerSaver
import core.game.node.entity.player.link.SpellBookManager.SpellBook
import core.game.node.entity.player.link.diary.AchievementDiary
import core.game.node.entity.player.link.diary.DiaryType
import core.game.node.entity.player.link.emote.Emotes
import core.game.node.entity.player.link.music.MusicEntry
import core.game.node.entity.player.link.quest.Quest
import core.game.node.entity.player.link.quest.QuestRepository
import core.game.node.item.Item
import core.game.world.map.Location
import org.json.simple.JSONArray
import org.json.simple.JSONObject
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser
import org.json.simple.parser.ParseException
import java.io.File
import java.lang.reflect.Field

/** Recording and failure-injection boundary shared by every stand-in. */
object Rec {
    val trace = arrayListOf<Any?>()
    val counts = HashMap<String, Int>()
    val failures = HashMap<String, Int>()
    var clock = 0L
    var step = 0L

    @JvmStatic
    fun call(tag: String, vararg values: Any?) {
        trace.add(listOf(tag, *values))
        val count = (counts[tag] ?: 0) + 1
        counts[tag] = count
        if (failures[tag] == count) throw Failure()
    }

    @JvmStatic
    fun now(): Long {
        val value = clock
        clock += step
        call("now", value)
        return value
    }

    @JvmStatic
    fun json(text: String): Any? = JSONParser().parse(text)

    @JvmStatic
    fun event(event: Event) {
        when (event) {
            is AttributeSetEvent -> call("event", "AttributeSet", event.attribute, event.value.toString())
            is AttributeRemoveEvent -> call("event", "AttributeRemove", event.attribute)
            else -> call("event", event.javaClass.simpleName)
        }
    }
}

class Hook(private val index: Int, private val key: String, private val value: String) : PersistPlayer {
    override fun savePlayer(player: Player, save: JSONObject) {
        Rec.call("hook.save", index)
        save[key] = Rec.json(value)
    }

    override fun parsePlayer(player: Player, data: JSONObject) {
        Rec.call("hook.parse", index, if (data.containsKey(key)) JSONValue.toJSONString(data[key]) else null)
    }
}

/** Scripted registration input. It adds nothing to the inherited original Quest. */
class ScriptedQuest(quest: Quests, index: Int, button: Int, points: Int) : Quest(quest, index, button, points, 0, 0, 1, 2) {
    override fun newInstance(`object`: Any?): Quest = this
}

object State {
    var base = File(".")
    var index = 0
    lateinit var player: Player
    var parser: PlayerSaveParser? = null
    private var world = JSONObject()
    private val last = HashMap<String, String>()

    private fun dir(kind: String) = File(base, "$index/$kind").path + File.separator
    private fun int(value: Any?) = (value as Number).toInt()
    private fun long(value: Any?) = (value as Number).toLong()
    private fun bits(value: Double) = String.format("%016x", java.lang.Double.doubleToLongBits(value))
    private fun flags(values: BooleanArray) = values.joinToString("") { if (it) "1" else "0" }

    fun field(obj: Any, name: String): Field {
        var type: Class<*>? = obj.javaClass
        while (type != null) {
            try {
                return type.getDeclaredField(name).apply { isAccessible = true }
            } catch (e: NoSuchFieldException) {
                type = type.superclass
            }
        }
        error(name)
    }

    private fun define(id: Int, stackable: Boolean) {
        val d = ItemDefinition()
        field(d, "id").set(d, id); field(d, "name").set(d, "item$id"); field(d, "value").set(d, id * 11)
        field(d, "stackable").setBoolean(d, stackable)
        ItemDefinition.getDefinitions()[id] = d
    }

    /** The server-wide registries the persisted state is read against. */
    private fun registry(w: JSONObject) {
        for (raw in w["quests"] as? JSONArray ?: JSONArray()) {
            val row = raw as JSONArray
            QuestRepository.register(ScriptedQuest(Quests.values()[int(row[0])], int(row[1]), int(row[2]), int(row[3])))
        }
        for (raw in w["songs"] as? JSONArray ?: JSONArray()) {
            val row = raw as JSONArray
            MusicEntry.getSongs()[int(row[0])] = MusicEntry(int(row[0]), row[1] as String, int(row[2]))
        }
        for (raw in w["spells"] as? JSONArray ?: JSONArray()) {
            val row = raw as JSONArray
            val own = SpellBook.values()[int(row[3])]
            SpellBook.values()[int(row[0])].register(int(row[1]), if (row[2] == true) CombatSpell(own) else MagicSpell(own))
        }
        for (raw in w["items"] as? JSONArray ?: JSONArray()) {
            val row = raw as JSONArray
            define(int(row[0]), row[1] == true)
        }
        for (raw in w["hooks"] as? JSONArray ?: JSONArray()) {
            val row = raw as JSONArray
            val hook = Hook(PlayerSaver.contentHooks.size, row[0] as String, row[1] as String)
            PlayerSaver.contentHooks.add(hook)
            PlayerSaveParser.contentHooks.add(hook)
        }
    }

    private fun newPlayer(): Player {
        val p = Player()
        p.location = Location(3222, 3218, 0)
        (world["name"] as? String)?.let { p.name = it }
        (world["version"] as? Number)?.let { p.version = it.toInt() }
        Model.entity = p
        parser = null
        return p
    }

    fun setup(input: JSONObject) {
        world = input["world"] as? JSONObject ?: JSONObject()
        ItemDefinition.getDefinitions().clear()
        for (i in 0..7) define(i, false)
        MusicEntry.getSongs().clear()
        for (book in SpellBook.values()) (field(book, "spells").get(book) as MutableMap<*, *>).clear()
        QuestRepository.getQuests().clear()
        AchievementDiary.completedLevels.clear()
        PlayerSaver.contentHooks.clear()
        PlayerSaveParser.contentHooks.clear()
        Rec.failures.clear()
        Rec.clock = (world["clock"] as? Number)?.toLong() ?: 0L
        Rec.step = (world["clockStep"] as? Number)?.toLong() ?: 0L
        ServerConstants.PLAYER_SAVE_PATH = dir("s")
        ServerConstants.PLAYER_ATTRIBUTE_PATH = dir("a")
        ServerConstants.JAVA_VERSION = 11
        ServerConstants.CURRENT_SAVEFILE_VERSION = 6
        ServerConstants.BANK_SIZE = 496
        Model.request = JSONObject()
        Model.events.clear()
        registry(world)
        player = newPlayer()
        Rec.trace.clear(); Rec.counts.clear()
        last.clear()
        if (input["full"] != true) for ((key, value) in sections()) last[key] = JSONValue.toJSONString(value)
    }

    private fun container(rows: JSONArray): Container {
        val c = Container(30)
        for (raw in rows) {
            val row = raw as JSONArray
            val item = Item(int(row[1]), int(row[2]), int(row[3]))
            item.index = int(row[0])
            c.toArray()[int(row[0])] = item
        }
        return c
    }

    private fun errorName(e: Exception): String = when (e) {
        is NullPointerException -> "Null"
        is NumberFormatException -> "Number"
        is ClassCastException -> "Type"
        is IndexOutOfBoundsException -> "Index"
        is NegativeArraySizeException -> "NegativeSize"
        is ConcurrentModificationException -> "ConcurrentModification"
        is Failure -> "Binding"
        is ParseException -> "Parse"
        is IllegalArgumentException -> "Argument"
        is java.io.IOException -> "Io"
        else -> throw e
    }

    fun action(a: JSONObject): Any? {
        val out = linkedMapOf<String, Any?>("error" to null)
        try {
            run(a, out)
        } catch (e: Exception) {
            out["error"] = errorName(e)
        }
        out["trace"] = ArrayList(Rec.trace)
        Rec.trace.clear()
        val changed = linkedMapOf<String, Any?>()
        for ((key, value) in sections()) {
            val text = JSONValue.toJSONString(value)
            if (last[key] != text) {
                last[key] = text
                changed[key] = value
            }
        }
        out["state"] = changed
        return out
    }

    private fun saveFile() = File(ServerConstants.PLAYER_SAVE_PATH + player.name + ".json")

    private fun parseSection(ps: PlayerSaveParser, name: String) = when (name) {
        "parseData" -> ps.parseData()
        "parseQuests" -> ps.parseQuests()
        "parseGrave" -> ps.parseGrave()
        "parseSpellbook" -> ps.parseSpellbook()
        "parseAutocastSpell" -> ps.parseAutocastSpell()
        "parseMusic" -> ps.parseMusic()
        "parseFamiliars" -> ps.parseFamiliars()
        "parseHouse" -> ps.parseHouse()
        "parseEmoteManager" -> ps.parseEmoteManager()
        "parseAchievements" -> ps.parseAchievements()
        "parsePouches" -> ps.parsePouches()
        "parsePOHStorable" -> ps.parsePOHStorable()
        "parseAttributes" -> ps.parseAttributes()
        else -> error("unknown parser section $name")
    }

    private fun saveSection(s: PlayerSaver, name: String, root: JSONObject) = when (name) {
        "saveQuests" -> s.saveQuests(root)
        "saveSpellbook" -> s.saveSpellbook(root)
        "saveAutocast" -> s.saveAutocast(root)
        "saveMusicPlayer" -> s.saveMusicPlayer(root)
        "saveFamiliarManager" -> s.saveFamiliarManager(root)
        "saveHouseData" -> s.saveHouseData(root)
        "saveAchievementData" -> s.saveAchievementData(root)
        "saveEmoteData" -> s.saveEmoteData(root)
        "saveAttributes" -> s.saveAttributes(root)
        "savePouches" -> s.savePouches(root)
        "savePOHStorageRoom" -> s.savePOHStorageRoom(root)
        else -> error("unknown saver section $name")
    }

    private fun coreContainer(name: String): Container = when (name) {
        "bank" -> player.bankPrimary
        "secondary" -> player.bankSecondary
        "equipment" -> player.equipment
        "inventory" -> player.inventory
        else -> error("unknown container $name")
    }

    private fun quest(a: JSONObject): Quest = player.questRepository.getQuest(Quests.values()[int(a["quest"])])!!

    private fun run(a: JSONObject, out: MutableMap<String, Any?>) {
        val p = player
        when (a["op"]) {
            // Whole profile.
            "populate" -> out["text"] = PlayerSaver(p).populate().toJSONString()
            "save" -> {
                val root = JSONObject()
                try {
                    saveSection(PlayerSaver(p), a["name"] as String, root)
                } finally {
                    out["text"] = root.toJSONString()
                }
            }
            "parse" -> {
                val ps = PlayerSaveParser(p)
                val text = a["text"] as String?
                ps.saveFile = if (text == null) null else ps.parser.parse(text) as JSONObject
                parseSection(ps, a["name"] as String)
            }
            "parseData" -> {
                val ps = PlayerSaveParser(p)
                parser = ps
                ps.saveFile = ps.parser.parse(a["text"] as String) as JSONObject
                ps.parseData()
            }
            "profileParse" -> {
                val ps = PlayerSaveParser(p)
                parser = ps
                try {
                    ps.parse()
                } finally {
                    // The original never closes its reader; the harness does, so later steps can replace the file on Windows.
                    ps.reader?.close()
                    out["read"] = ps.read
                    out["loaded"] = ps.saveFile != null
                }
            }
            "contentHooks" -> (if (a["fresh"] == true) PlayerSaveParser(p) else parser!!).runContentHooks()
            "profileSave" -> {
                val file = saveFile()
                out["existed"] = file.exists()
                PlayerSaver(p).save()
                out["file"] = if (file.exists()) String(file.readBytes(), Charsets.UTF_8) else null
            }
            "newPlayer" -> player = newPlayer()
            "registry" -> registry(a["world"] as JSONObject)
            "failAt" -> Rec.failures[a["tag"] as String] = (Rec.counts[a["tag"] as String] ?: 0) + ((a["at"] as? Number)?.toInt() ?: 1)
            "version" -> p.version = int(a["value"])
            "saveParsed" -> p.details.saveParsed = a["value"] as Boolean
            // Quests.
            "quest.parse" -> p.questRepository.parse(Rec.json(a["text"] as String) as JSONObject)
            "quest.put" -> p.questRepository.questList[int(a["index"])] = int(a["stage"])
            "quest.stage" -> if (a["any"] == true) p.questRepository.setStageNonmonotonic(quest(a), int(a["stage"])) else p.questRepository.setStage(quest(a), int(a["stage"]))
            "quest.points" -> if (a["dock"] == true) p.questRepository.dockPoints(int(a["value"])) else p.questRepository.incrementPoints(int(a["value"]))
            "quest.sync" -> p.questRepository.syncPoints()
            "quest.getStage" -> out["value"] = p.questRepository.getStage(Quests.values()[int(a["quest"])])
            "quest.isComplete" -> out["value"] = p.questRepository.isComplete(Quests.values()[int(a["quest"])])
            "quest.hasStarted" -> out["value"] = p.questRepository.hasStarted(Quests.values()[int(a["quest"])])
            "quest.totals" -> out["value"] = listOf(p.questRepository.availablePoints, p.questRepository.hasCompletedAll())
            // Music.
            "music.put" -> { val entry = MusicEntry.forId(int(a["id"])); p.musicPlayer.unlocked[entry.index] = entry }
            "music.clear" -> p.musicPlayer.clearUnlocked()
            "music.query" -> out["value"] = listOf(p.musicPlayer.hasUnlocked(int(a["id"])), p.musicPlayer.hasUnlockedIndex(int(a["id"])), p.musicPlayer.hasAirGuitar())
            // Emotes.
            "emote.add" -> p.emoteManager.emotes.add(Emotes.values()[int(a["ordinal"])])
            "emote.remove" -> out["value"] = p.emoteManager.emotes.remove(Emotes.values()[int(a["ordinal"])])
            "emote.clear" -> p.emoteManager.emotes.clear()
            "emote.isUnlocked" -> out["value"] = p.emoteManager.isUnlocked(Emotes.values()[int(a["ordinal"])])
            // Achievement diaries.
            "diary.parse" -> p.achievementDiaryManager.parse(Rec.json(a["text"] as String) as JSONArray)
            "diary.raw" -> {
                val d = p.achievementDiaryManager.diarys[int(a["type"])]
                (a["started"] as JSONArray?)?.forEachIndexed { i, v -> d.levelStarted[i] = v as Boolean }
                (a["rewarded"] as JSONArray?)?.forEachIndexed { i, v -> d.levelRewarded[i] = v as Boolean }
                for (raw in a["tasks"] as? JSONArray ?: JSONArray()) { val row = raw as JSONArray; d.taskCompleted[int(row[0])][int(row[1])] = row[2] as Boolean }
            }
            "diary.reset" -> p.achievementDiaryManager.resetRewards()
            "diary.table" -> out["value"] = DiaryType.values().map { t ->
                listOf(t.name, t.getName(), t.achievements.size, t.rewards.map { level -> level.map { listOf(it.id, it.amount, it.charge) } })
            }
            // Spells.
            "autocast" -> {
                val spell = a["spell"] as JSONArray?
                p.properties.autocastSpell = spell?.let { SpellBook.values()[int(it[0])].getSpell(int(it[1])) as CombatSpell }
            }
            "spellbook" -> field(p.spellBookManager, "spellBook").setInt(p.spellBookManager, int(a["value"]))
            // Grave type.
            "grave" -> GraveController.updateGraveType(p, GraveType.values()[int(a["ordinal"])])
            "grave.get" -> out["value"] = GraveController.getGraveType(p).ordinal
            // Runecrafting pouches.
            "pouch.parse" -> p.pouchManager.parse(Rec.json(a["text"] as String) as JSONArray)
            "pouch.set" -> {
                val pouch = p.pouchManager.pouches[int(a["id"])]!!
                (a["charges"] as? Number)?.let { pouch.charges = it.toInt() }
                (a["cap"] as? Number)?.let { pouch.currentCap = it.toInt() }
                if (a["remake"] == true) pouch.remakeContainer()
                for (raw in a["items"] as? JSONArray ?: JSONArray()) { val row = raw as JSONArray; out["added"] = pouch.container.add(Item(int(row[0]), int(row[1]))) }
            }
            // POH storage.
            "poh.add" -> for (id in a["ids"] as JSONArray) p.getPOHStorageState().getContainer(StorableFamily.values()[int(a["family"])]).addItem(int(id))
            "poh.withdraw" -> out["value"] = p.getPOHStorageState().getContainer(StorableFamily.values()[int(a["family"])]).withdraw(int(a["id"]))
            "poh.page" -> p.getPOHStorageState().getContainer(StorableFamily.values()[int(a["family"])]).nextPage(int(a["total"]), int(a["size"]))
            "poh.read" -> p.getPOHStorageState().readJson(Rec.json(a["text"] as String) as JSONObject)
            "poh.clear" -> p.getPOHStorageState().clear()
            // House and familiars: scripted values for the saving side only.
            "house" -> {
                val m = p.houseManager
                m.location = HouseLocation.values()[int(a["location"])]
                m.style = HousingStyle.values()[int(a["style"])]
                m.crest = CrestType.values()[int(a["crest"])]
                val s = a["servant"] as JSONObject?
                m.servant = s?.let { Servant(int(it["type"]), int(it["uses"]), (it["item"] as JSONArray?)?.let { i -> Item(int(i[0]), int(i[1])) }, it["greet"] == true) }
                for (z in 0..3) for (x in 0..7) for (y in 0..7) m.rooms[z][x][y] = null
                for (raw in a["rooms"] as? JSONArray ?: JSONArray()) {
                    val r = raw as JSONObject
                    m.rooms[int(r["z"])][int(r["x"])][int(r["y"])] = Room(int(r["properties"]), int(r["rotation"]), (r["hotspots"] as JSONArray).map { Hotspot(int(it)) }.toTypedArray())
                }
            }
            "pets" -> for (raw in a["pets"] as JSONArray) {
                val row = raw as JSONArray
                val list = ArrayList<PetDetails>()
                for (d in row[1] as JSONArray) {
                    val pair = d as JSONArray
                    list.add(PetDetails((pair[1] as String).toDouble()).apply { hunger = (pair[0] as String).toDouble() })
                }
                p.familiarManager.petDetails[int(row[0])] = list
            }
            "familiar" -> {
                val d = a["familiar"] as JSONObject?
                p.familiarManager.script(when {
                    d == null -> null
                    d.containsKey("pet") -> Pet(int(d["pet"]))
                    else -> {
                        val f = if (d.containsKey("burden")) BurdenBeast(container(d["burden"] as JSONArray)) else Familiar()
                        f.script(int(d["originalId"]), int(d["ticks"]), int(d["specialPoints"]))
                        field(f.skills, "lifepoints").setInt(f.skills, int(d["lifepoints"]))
                        f
                    }
                })
            }
            // The profile state the ported systems reach.
            "item" -> {
                val c = coreContainer(a["container"] as String)
                val item = if (a["null"] == true) null else Item(int(a["id"]), int(a["amount"]), (a["charge"] as? Number)?.toInt() ?: 1000)
                if (item != null) item.index = int(a["slot"])
                c.toArray()[int(a["slot"])] = item
            }
            "secondary" -> p.useSecondaryBank = a["value"] as Boolean
            "attr" -> {
                val value: Any? = when (a["kind"]) {
                    "int" -> int(a["value"])
                    "long" -> long(a["value"])
                    "bool" -> a["value"] as Boolean
                    "str" -> a["value"] as String
                    else -> error("unknown attribute kind")
                }
                p.gameAttributes.setAttribute(a["key"] as String, value)
            }
            else -> error("unknown state action $a")
        }
    }

    private fun items(c: Container) = c.toArray().mapIndexedNotNull { i, t -> t?.let { listOf(i, it.id, it.amount, it.charge) } }

    private fun attribute(value: Any?): List<Any?> = when (value) {
        null -> listOf("null", null)
        is Int -> listOf("int", value.toString())
        is Long -> listOf("long", value.toString())
        is Short -> listOf("short", value.toString())
        is Byte -> listOf("byte", value.toString())
        is Boolean -> listOf("bool", value.toString())
        is String -> listOf("str", value)
        is Location -> listOf("location", value.toString())
        else -> listOf("object", value.toString())
    }

    /** Everything is read through the original accessors, in the original collections' own order. */
    private fun sections(): Map<String, Any?> {
        val p = player
        val out = linkedMapOf<String, Any?>()
        out["quests"] = listOf(p.questRepository.points, p.questRepository.questList.entries.map { listOf(it.key, it.value) })
        out["music"] = p.musicPlayer.unlocked.entries.map { listOf(it.key, it.value.id) }
        out["emotes"] = listOf(p.emoteManager.isSaveRequired, p.emoteManager.emotes.map { it.ordinal })
        out["diaries"] = p.achievementDiaryManager.diarys.map { d -> listOf(d.type.name, flags(d.levelStarted), d.taskCompleted.map { flags(it) }, flags(d.levelRewarded)) }
        out["diaryLevels"] = ArrayList(AchievementDiary.completedLevels)
        out["autocast"] = p.properties.autocastSpell?.let { listOf(it.book.ordinal, it.spellId) }
        out["spellbook"] = p.spellBookManager.spellBook
        val attributes = p.gameAttributes
        out["attributes"] = linkedMapOf(
            "saved" to ArrayList(attributes.savedAttributes),
            "values" to attributes.attributes.entries.sortedBy { it.key }.map { listOf(it.key) + attribute(it.value) })
        out["pouches"] = p.pouchManager.pouches.map { (id, pouch) -> listOf(id, pouch.charges, pouch.currentCap, pouch.container.toArray().size, items(pouch.container)) }
        out["poh"] = StorableFamily.values().map { family -> p.getPOHStorageState().getContainer(family).let { listOf(it.getItems(), it.getPageIndex()) } }
        val house = p.houseManager
        out["house"] = listOf(house.location.ordinal, house.style.ordinal, house.crest.ordinal, house.hasServant(), house.rooms.sumOf { plane -> plane.sumOf { row -> row.count { it != null } } })
        val familiars = p.familiarManager
        out["pets"] = familiars.petDetails.entries.map { listOf(it.key, it.value.map { d -> listOf(bits(d.hunger), bits(d.growth)) }) }
        out["familiar"] = when {
            familiars.hasPet() -> listOf("pet", (familiars.familiar as Pet).getItemId())
            familiars.hasFamiliar() -> listOf("familiar", familiars.familiar.originalId)
            else -> null
        }
        out["containers"] = linkedMapOf("inventory" to items(p.inventory), "bank" to items(p.bankPrimary), "secondary" to items(p.bankSecondary), "equipment" to items(p.equipment), "useSecondary" to p.useSecondaryBank)
        out["version"] = p.version
        out["saveParsed"] = p.details.saveParsed
        return out
    }
}

fun main(args: Array<String>) {
    if (System.getProperty("java.specification.version") != "11") throw IllegalStateException("Use JDK 11 for the player save oracle")
    State.base = File(args[0])
    for (line in generateSequence(::readLine)) {
        val input = JSONParser().parse(line) as JSONObject
        State.index++
        State.setup(input)
        input["expected"] = (input["actions"] as JSONArray).map { State.action(it as JSONObject) }
        println(JSONValue.toJSONString(input))
    }
}
