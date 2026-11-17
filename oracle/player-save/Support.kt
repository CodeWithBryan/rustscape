// Harness for the unmodified PlayerSaver.kt and PlayerSaveParser.kt. It only
// scripts inputs, calls the original entry points and observes state through
// the original accessors; it contains no model of the expected save format.
package oracle

import content.global.skill.construction.Hotspot
import content.global.skill.construction.Room
import content.global.skill.construction.Servant
import content.global.skill.summoning.familiar.BurdenBeast
import content.global.skill.summoning.familiar.Familiar
import content.global.skill.summoning.pet.Pet
import content.global.skill.summoning.pet.PetDetails
import core.ServerConstants
import core.api.PersistPlayer
import core.cache.def.impl.ItemDefinition
import core.game.container.Container
import core.game.node.entity.combat.spell.CombatSpell
import core.game.node.entity.combat.spell.MagicSpell
import core.game.node.entity.player.Player
import core.game.node.entity.player.info.login.PlayerSaveParser
import core.game.node.entity.player.info.login.PlayerSaver
import core.game.node.entity.player.link.BankPinManager
import core.game.node.entity.player.link.IronmanMode
import core.game.node.entity.player.link.SpellBookManager.SpellBook
import core.game.node.entity.player.link.appearance.BodyPart
import core.game.node.entity.player.link.appearance.Gender
import core.game.node.entity.player.link.diary.AchievementDiary
import core.game.node.entity.player.link.diary.DiaryType
import core.game.node.entity.player.link.emote.Emotes
import core.game.node.entity.player.link.music.MusicEntry
import core.game.node.item.Item
import core.game.world.map.Location
import org.json.simple.JSONArray
import org.json.simple.JSONObject
import org.json.simple.JSONValue
import org.json.simple.parser.JSONParser
import org.json.simple.parser.ParseException
import java.io.File
import java.lang.reflect.Field
import java.lang.reflect.Modifier
import java.util.TreeMap

/** Recording and scripting boundary shared by every adapter of an unported system. */
object Rec {
    val trace = arrayListOf<Any?>()
    val counts = HashMap<String, Int>()
    val failures = HashMap<String, Int>()
    var clock = 0L
    var step = 0L
    var pouches: List<Pair<String, String>> = emptyList()
    var poh = "{}"

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

object Profile {
    var base = File(".")
    var index = 0
    var parser: PlayerSaveParser? = null
    private var quiet = false
    private val last = HashMap<String, String>()
    val player get() = Model.entity as Player

    private fun dir(kind: String) = File(base, "$index/$kind").path + File.separator
    private fun int(value: Any?) = (value as Number).toInt()
    private fun long(value: Any?) = (value as Number).toLong()
    private fun bools(value: Any?) = BooleanArray((value as JSONArray).size) { value[it] as Boolean }
    private fun bits(value: Double) = String.format("%016x", java.lang.Double.doubleToLongBits(value))

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

    fun setup(input: JSONObject): Player {
        ItemDefinition.getDefinitions().clear()
        for (i in 0..7) {
            val d = ItemDefinition()
            field(d, "id").set(d, i); field(d, "name").set(d, "item$i"); field(d, "value").set(d, i * 11)
            ItemDefinition.getDefinitions()[i] = d
        }
        MusicEntry.getSongs().clear()
        for (book in SpellBook.values()) (field(book, "spells").get(book) as MutableMap<*, *>).clear()
        PlayerSaver.contentHooks.clear()
        PlayerSaveParser.contentHooks.clear()
        val config = input["config"] as? JSONObject ?: JSONObject()
        Rec.failures.clear(); Rec.pouches = listOf("pouches" to "[]"); Rec.poh = "{\"containers\":{}}"
        Rec.clock = (config["clock"] as? Number)?.toLong() ?: 0L
        Rec.step = (config["clockStep"] as? Number)?.toLong() ?: 0L
        ServerConstants.PLAYER_SAVE_PATH = dir("s")
        ServerConstants.PLAYER_ATTRIBUTE_PATH = dir("a")
        ServerConstants.JAVA_VERSION = 11
        ServerConstants.CURRENT_SAVEFILE_VERSION = 6
        ServerConstants.BANK_SIZE = 496
        parser = null
        val p = Player()
        if (config["location"] != false) p.location = Location(3222, 3218, 0)
        (config["name"] as? String)?.let { p.name = it }
        (config["version"] as? Number)?.let { p.version = it.toInt() }
        world(p, config["world"] as? JSONObject)
        for ((tag, at) in config["fail"] as? JSONObject ?: JSONObject()) Rec.failures[tag as String] = int(at)
        Rec.trace.clear(); Rec.counts.clear()
        // Later observations record only the sections that changed. Unless the case asks for a full first
        // observation, the state right after setup is the unrecorded starting point.
        last.clear()
        if (config["full"] != true) for ((key, value) in sections(p)) last[key] = JSONValue.toJSONString(value)
        return p
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

    fun world(p: Player, w: JSONObject?) {
        w ?: return
        (w["quests"] as? JSONObject)?.let { q ->
            p.questRepository.script(int(q["points"]))
            for (raw in q["list"] as JSONArray) { val row = raw as JSONArray; p.questRepository.questList[int(row[0])] = int(row[1]) }
        }
        (w["spells"] as? JSONArray)?.let {
            for (raw in it) {
                val row = raw as JSONArray
                val book = SpellBook.values()[int(row[0])]
                book.register(int(row[1]), if (row[2] == true) CombatSpell(book) else MagicSpell(book))
            }
        }
        if (w.containsKey("autocast")) {
            val a = w["autocast"] as JSONArray?
            p.properties.autocastSpell = a?.let { CombatSpell(SpellBook.values()[int(it[0])]).apply { spellId = int(it[1]) } }
        }
        (w["songs"] as? JSONArray)?.let {
            for (raw in it) { val row = raw as JSONArray; MusicEntry.getSongs()[int(row[0])] = MusicEntry(int(row[0]), "song", int(row[1])) }
        }
        (w["unlocked"] as? JSONArray)?.let {
            for (id in it) { val entry = MusicEntry.forId(int(id)); p.musicPlayer.unlocked[entry.index] = entry }
        }
        (w["emotes"] as? JSONArray)?.let {
            p.emoteManager.emotes.clear()
            for (ordinal in it) p.emoteManager.emotes.add(Emotes.values()[int(ordinal)])
        }
        (w["diaries"] as? JSONArray)?.let {
            p.achievementDiaryManager.script(it.map { raw ->
                val d = raw as JSONObject
                AchievementDiary(DiaryType.values()[int(d["type"])], bools(d["started"]), (d["completed"] as JSONArray).map { level -> bools(level) }.toTypedArray(), bools(d["rewarded"]))
            }.toTypedArray())
        }
        (w["house"] as? JSONObject)?.let { h ->
            val m = p.houseManager
            m.script(int(h["location"]), int(h["style"]), int(h["crest"]))
            val s = h["servant"] as JSONObject?
            m.servant = s?.let { Servant(int(it["type"]), int(it["uses"]), (it["item"] as JSONArray?)?.let { i -> Item(int(i[0]), int(i[1])) }, it["greet"] == true) }
            for (z in 0..3) for (x in 0..7) for (y in 0..7) m.rooms[z][x][y] = null
            for (raw in h["rooms"] as? JSONArray ?: JSONArray()) {
                val r = raw as JSONObject
                m.rooms[int(r["z"])][int(r["x"])][int(r["y"])] = Room(int(r["properties"]), int(r["rotation"]), (r["hotspots"] as JSONArray).map { Hotspot(int(it)) }.toTypedArray())
            }
        }
        (w["pets"] as? JSONArray)?.let {
            for (raw in it) {
                val row = raw as JSONArray
                val list = ArrayList<PetDetails>()
                for (d in row[1] as JSONArray) {
                    val pair = d as JSONArray
                    list.add(PetDetails((pair[1] as String).toDouble()).apply { hunger = (pair[0] as String).toDouble() })
                }
                p.familiarManager.petDetails[int(row[0])] = list
            }
        }
        if (w.containsKey("familiar")) {
            val d = w["familiar"] as JSONObject?
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
        (w["pouches"] as? JSONArray)?.let { Rec.pouches = it.map { raw -> val row = raw as JSONArray; (row[0] as String) to (row[1] as String) } }
        (w["poh"] as? String)?.let { Rec.poh = it }
        (w["hooks"] as? JSONArray)?.let {
            for (raw in it) {
                val row = raw as JSONArray
                val hook = Hook(PlayerSaver.contentHooks.size, row[0] as String, row[1] as String)
                PlayerSaver.contentHooks.add(hook)
                PlayerSaveParser.contentHooks.add(hook)
            }
        }
    }

    private fun errorName(e: Exception): String = when (e) {
        is NullPointerException -> "Null"
        is NumberFormatException -> "Number"
        is ClassCastException -> "Type"
        is IndexOutOfBoundsException -> "Index"
        is ConcurrentModificationException -> "ConcurrentModification"
        is Failure -> "Binding"
        is ParseException -> "Parse"
        is IllegalArgumentException -> "Argument"
        is java.io.IOException -> "Io"
        else -> throw e
    }

    fun action(a: JSONObject): Any? {
        val out = linkedMapOf<String, Any?>("error" to null)
        quiet = a["quiet"] == true
        try {
            run(a, out)
        } catch (e: Exception) {
            out["error"] = errorName(e)
        }
        return out
    }

    private fun write(file: File, a: JSONObject) {
        if (a["delete"] == true) {
            file.delete()
            return
        }
        if (a["directory"] == true) {
            file.mkdirs()
            return
        }
        file.parentFile.mkdirs()
        val hex = a["hex"] as String?
        file.writeBytes(if (hex != null) ByteArray(hex.length / 2) { hex.substring(it * 2, it * 2 + 2).toInt(16).toByte() } else (a["text"] as String).toByteArray(Charsets.UTF_8))
    }

    private fun saveFile() = File(ServerConstants.PLAYER_SAVE_PATH + player.name + ".json")

    private fun parseSection(ps: PlayerSaveParser, name: String) = when (name) {
        "parseData" -> ps.parseData()
        "parseVersion" -> ps.parseVersion()
        "parseCore" -> ps.parseCore()
        "parseAttributes" -> ps.parseAttributes()
        "parseSkills" -> ps.parseSkills()
        "parseSettings" -> ps.parseSettings()
        "parseQuests" -> ps.parseQuests()
        "parseAppearance" -> ps.parseAppearance()
        "parseGrave" -> ps.parseGrave()
        "parseVarps" -> ps.parseVarps()
        "parseSpellbook" -> ps.parseSpellbook()
        "parseSavedData" -> ps.parseSavedData()
        "parseAutocastSpell" -> ps.parseAutocastSpell()
        "parseFarming" -> ps.parseFarming()
        "parseConfigs" -> ps.parseConfigs()
        "parseMonitor" -> ps.parseMonitor()
        "parseMusic" -> ps.parseMusic()
        "parseFamiliars" -> ps.parseFamiliars()
        "parseBankPin" -> ps.parseBankPin()
        "parseHouse" -> ps.parseHouse()
        "parseIronman" -> ps.parseIronman()
        "parseEmoteManager" -> ps.parseEmoteManager()
        "parseStatistics" -> ps.parseStatistics()
        "parseAchievements" -> ps.parseAchievements()
        "parsePouches" -> ps.parsePouches()
        "parsePOHStorable" -> ps.parsePOHStorable()
        else -> error("unknown parser section $name")
    }

    private fun saveSection(s: PlayerSaver, name: String, root: JSONObject) = when (name) {
        "saveVersion" -> s.saveVersion(root)
        "saveCoreData" -> s.saveCoreData(root)
        "saveSkills" -> s.saveSkills(root)
        "saveSettings" -> s.saveSettings(root)
        "saveQuests" -> s.saveQuests(root)
        "saveAppearance" -> s.saveAppearance(root)
        "saveSpellbook" -> s.saveSpellbook(root)
        "saveSavedData" -> s.saveSavedData(root)
        "saveActivityData" -> s.saveActivityData(root)
        "saveQuestData" -> s.saveQuestData(root)
        "saveGlobalData" -> s.saveGlobalData(root)
        "saveAutocast" -> s.saveAutocast(root)
        "savePlayerMonitor" -> s.savePlayerMonitor(root)
        "saveMusicPlayer" -> s.saveMusicPlayer(root)
        "saveFamiliarManager" -> s.saveFamiliarManager(root)
        "saveBankPinData" -> s.saveBankPinData(root)
        "saveHouseData" -> s.saveHouseData(root)
        "saveAchievementData" -> s.saveAchievementData(root)
        "saveIronManData" -> s.saveIronManData(root)
        "saveEmoteData" -> s.saveEmoteData(root)
        "saveStatManager" -> s.saveStatManager(root)
        "saveAttributes" -> s.saveAttributes(root)
        "savePouches" -> s.savePouches(root)
        "savePOHStorageRoom" -> s.savePOHStorageRoom(root)
        else -> error("unknown saver section $name")
    }

    private fun coreContainer(name: String): Container = when (name) {
        "bank" -> player.bankPrimary
        "secondary" -> player.bankSecondary
        "equipment" -> player.equipment
        else -> player.inventory
    }

    private fun number(a: JSONObject, key: String, fallback: Int = 0) = (a[key] as? Number)?.toInt() ?: fallback

    private fun run(a: JSONObject, out: MutableMap<String, Any?>) {
        val p = player
        when (a["op"]) {
            "saveFile" -> write(saveFile(), a)
            "blockSaveDirectory" -> {
                // A plain file where the save directory should be, so the saver cannot create its file.
                val directory = File(ServerConstants.PLAYER_SAVE_PATH!!.trimEnd(File.separatorChar))
                directory.parentFile.mkdirs()
                directory.writeText("not a directory")
            }
            "attributeFile" -> write(File(ServerConstants.PLAYER_ATTRIBUTE_PATH + p.name + ".xml"), a)
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
            "parseText" -> {
                val ps = PlayerSaveParser(p)
                parser = ps
                ps.saveFile = ps.parser.parse(a["text"] as String) as JSONObject
                ps.parseData()
            }
            "parseSection" -> {
                val ps = PlayerSaveParser(p)
                val text = a["text"] as String?
                ps.saveFile = if (text == null) null else ps.parser.parse(text) as JSONObject
                parseSection(ps, a["name"] as String)
            }
            "populate" -> out["text"] = PlayerSaver(p).populate().toJSONString()
            "profileSave" -> {
                val file = saveFile()
                out["existed"] = file.exists()
                PlayerSaver(p).save()
                out["saved"] = true
                out["file"] = if (file.exists()) String(file.readBytes(), Charsets.UTF_8) else null
            }
            "saveSection" -> saveSection(PlayerSaver(p), a["name"] as String, Model.root)
            "world" -> world(p, a["world"] as JSONObject)
            "failAt" -> Rec.failures[a["tag"] as String] = (Rec.counts[a["tag"] as String] ?: 0) + number(a, "at", 1)
            "clock" -> { Rec.clock = long(a["value"]); Rec.step = (a["step"] as? Number)?.toLong() ?: Rec.step }
            "name" -> p.name = a["value"] as String
            "version" -> p.version = int(a["value"])
            "saveParsed" -> p.details.saveParsed = a["value"] as Boolean
            "retaliate" -> p.properties.isRetaliating = a["value"] as Boolean
            "spellbook" -> field(p.spellBookManager, "spellBook").setInt(p.spellBookManager, int(a["value"]))
            "ironman" -> p.ironmanManager.mode = (a["mode"] as Number?)?.let { IronmanMode.values()[it.toInt()] }
            "gender" -> field(p.appearance, "gender").set(p.appearance, (a["value"] as Number?)?.let { Gender.values()[it.toInt()] })
            "part" -> p.appearance.appearanceCache[int(a["index"])] = if (a["null"] == true) null else BodyPart(int(a["look"]), int(a["color"]))
            "pin" -> {
                val m = p.bankPinManager
                if (a.containsKey("pin")) field(m, "pin").set(m, a["pin"] as String?)
                if (a.containsKey("status")) field(m, "status").set(m, (a["status"] as Number?)?.let { BankPinManager.PinStatus.values()[it.toInt()] })
                if (a.containsKey("longRecovery")) field(m, "longRecovery").setBoolean(m, a["longRecovery"] as Boolean)
                if (a.containsKey("pendingDelay")) field(m, "pendingDelay").setLong(m, long(a["pendingDelay"]))
                if (a.containsKey("tryDelay")) field(m, "tryDelay").setLong(m, long(a["tryDelay"]))
            }
            "settings" -> for ((key, value) in a["fields"] as JSONObject) {
                val f = field(p.settings, key as String)
                when (f.type) {
                    Double::class.javaPrimitiveType -> f.setDouble(p.settings, (value as String).toDouble())
                    Boolean::class.javaPrimitiveType -> f.setBoolean(p.settings, value as Boolean)
                    else -> f.setInt(p.settings, int(value))
                }
            }
            "skills" -> for ((key, value) in a["fields"] as JSONObject) {
                val f = field(p.skills, key as String)
                val target = f.get(p.skills)
                when {
                    target is DoubleArray -> (value as JSONArray).forEachIndexed { i, v -> target[i] = (v as String).toDouble() }
                    target is IntArray -> (value as JSONArray).forEachIndexed { i, v -> target[i] = int(v) }
                    f.type == Double::class.javaPrimitiveType -> f.setDouble(p.skills, (value as String).toDouble())
                    f.type == Boolean::class.javaPrimitiveType -> f.setBoolean(p.skills, value as Boolean)
                    else -> f.setInt(p.skills, int(value))
                }
            }
            "attr" -> {
                val key = a["key"] as String
                val value: Any? = when (a["kind"]) {
                    "int" -> int(a["value"])
                    "long" -> long(a["value"])
                    "short" -> long(a["value"]).toShort()
                    "byte" -> long(a["value"]).toByte()
                    "bool" -> a["value"] as Boolean
                    "str" -> a["value"] as String
                    "location" -> (a["value"] as JSONArray).let { Location(int(it[0]), int(it[1]), int(it[2])) }
                    "double" -> (a["value"] as String).toDouble()
                    "null" -> null
                    else -> error("unknown attribute kind")
                }
                p.gameAttributes.setAttribute(key, value)
                if (a.containsKey("expires")) p.gameAttributes.keyExpirations[key.removePrefix("/save:")] = (a["expires"] as Number?)?.toLong()
            }
            "questItem" -> p.savedData.questData.setDesertTreasureItem(int(a["index"]), if (a["null"] == true) null else Item(int(a["id"]), int(a["amount"])))
            "attrSaved" -> p.gameAttributes.savedAttributes.add(a["key"] as String)
            "coreVarp" -> { val i = number(a, "index"); p.varpMap[i] = number(a, "value"); if (a.containsKey("save")) p.saveVarp[i] = a["save"] as Boolean? }
            "coreSecondary" -> p.useSecondaryBank = a["value"] as Boolean
            "coreLocation" -> { val v = a["value"] as JSONArray?; p.location = if (v == null) null else Location(int(v[0]), int(v[1]), int(v[2])) }
            "coreTab" -> { val tabs = if (a["secondary"] == true) p.bankSecondary.tabStartSlot else p.bankPrimary.tabStartSlot; tabs[number(a, "index")] = number(a, "value") }
            "coreRaw" -> {
                val c = coreContainer(a["container"] as? String ?: "inventory")
                val item = if (a["null"] == true) null else Item(number(a, "id", 1), number(a, "amount", 1), number(a, "charge", 1000))
                if (item != null) item.index = number(a, "index", number(a, "slot"))
                c.toArray()[number(a, "slot")] = item
            }
            else -> error("unknown profile action $a")
        }
    }

    private fun snapshot(value: Any?): Any? {
        if (value == null || value is String || value is Boolean) return value
        if (value is Double) return bits(value)
        if (value is Number) return value
        if (value is Item) return mapOf("amount" to value.amount, "idHash" to value.idHash)
        if (value.javaClass.isArray) return (0 until java.lang.reflect.Array.getLength(value)).map { snapshot(java.lang.reflect.Array.get(value, it)) }
        val out = TreeMap<String, Any?>()
        for (f in value.javaClass.declaredFields) {
            if (Modifier.isStatic(f.modifiers) || f.name == "player" || f.name == "entity") continue
            f.isAccessible = true
            out[f.name] = snapshot(f.get(value))
        }
        return out
    }

    private fun observeContainer(c: Container): Any {
        val items = c.toArray().mapIndexedNotNull { i, t -> t?.let { listOf(i, it.id, it.amount, it.charge, it.index) } }
        return listOf(c.toArray().size, field(c, "type").get(c).toString(), field(c, "sortType").get(c).toString(), c.listeners.size, items, c.event.slots.toList(), c.event.isClear)
    }

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

    fun observe(): Any {
        val out = linkedMapOf<String, Any?>()
        out["trace"] = ArrayList(Rec.trace)
        Rec.trace.clear()
        // Setup steps may ask for the trace alone; their changes are recorded by the next full step.
        if (quiet) {
            quiet = false
            return out
        }
        for ((key, value) in sections(player)) {
            val text = JSONValue.toJSONString(value)
            if (last[key] != text) {
                last[key] = text
                out[key] = value
            }
        }
        return out
    }

    private fun sections(p: Player): Map<String, Any?> {
        val out = linkedMapOf<String, Any?>()
        val loc = p.location
        out["core"] = linkedMapOf(
            "inventory" to observeContainer(p.inventory), "bank" to observeContainer(p.bankPrimary), "secondary" to observeContainer(p.bankSecondary), "equipment" to observeContainer(p.equipment),
            "tabs" to p.bankPrimary.tabStartSlot.toList(), "tabsSecondary" to p.bankSecondary.tabStartSlot.toList(), "secondarySelected" to p.useSecondaryBank,
            "location" to loc?.let { listOf(it.x, it.y, it.z) }, "varps" to p.varpMap.entries.map { listOf(it.key, it.value) }, "saveVarps" to p.saveVarp.mapKeys { it.key.toString() })
        out["version"] = p.version
        out["saveParsed"] = p.details.saveParsed
        out["spellbook"] = p.spellBookManager.spellBook
        val attributes = p.gameAttributes
        out["attributes"] = linkedMapOf(
            "saved" to ArrayList(attributes.savedAttributes),
            "values" to attributes.attributes.entries.sortedBy { it.key }.map { listOf(it.key) + attribute(it.value) },
            "expirations" to attributes.keyExpirations.entries.sortedBy { it.key }.map { listOf(it.key, it.value) })
        out["skills"] = snapshot(p.skills)
        out["combat"] = listOf(p.properties.combatLevel, p.familiarManager.summoningCombatLevel)
        out["settings"] = snapshot(p.settings)
        out["retaliating"] = p.properties.isRetaliating
        out["activity"] = snapshot(p.savedData.activityData)
        out["quest"] = snapshot(p.savedData.questData)
        out["global"] = snapshot(p.savedData.globalData)
        out["appearance"] = listOf(p.appearance.gender?.ordinal, p.appearance.appearanceCache.map { part -> part?.let { listOf(it.look, it.color) } })
        val pin = p.bankPinManager
        out["pin"] = listOf(pin.pin, pin.isLongRecovery, pin.status?.ordinal, pin.pendingDelay, pin.tryDelay)
        out["ironman"] = p.ironmanManager.mode?.ordinal
        out["music"] = p.musicPlayer.unlocked.entries.sortedBy { it.key }.map { listOf(it.key, it.value.id) }
        out["emotes"] = p.emoteManager.emotes.map { it.ordinal }
        out["autocast"] = p.properties.autocastSpell?.let { listOf(it.book.ordinal, it.spellId) }
        return out
    }
}

fun main(args: Array<String>) {
    if (System.getProperty("java.specification.version") != "11") throw IllegalStateException("Use JDK 11 for the player save oracle")
    Profile.base = File(args[0])
    Model.entityFactory = Profile::setup
    Model.extraAction = Profile::action
    Model.extraObservation = Profile::observe
    for (line in generateSequence(::readLine)) {
        val input = JSONParser().parse(line) as JSONObject
        Profile.index++
        input["expected"] = Model.evaluate(input)
        println(JSONValue.toJSONString(input))
    }
}
