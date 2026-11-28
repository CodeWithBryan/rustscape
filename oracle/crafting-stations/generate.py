"""Unmodified crafting sources (spinning and silver casting) over a recording world.

Java/Kotlin are the test oracle only. --check never rewrites the fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / "runs/crafting-stations-oracle"
SERVER = ROOT / "reference/server/Server/src/main"
spec = importlib.util.spec_from_file_location("compiler", ROOT / "oracle/login/generate.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
read, run = c.read, c.run
spec = importlib.util.spec_from_file_location("cases", HERE / "cases.py")
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)

CRAFTING = "content/global/skill/crafting/"
UNMODIFIED = [
    "content/global/handlers/scenery/SpinSpinningWheelPlugin.java",
    "content/global/handlers/iface/SpinningInterface.java",
    CRAFTING + "spinning/SpinningItem.java",
    CRAFTING + "spinning/SpinningPulse.java",
    CRAFTING + "silver/SilverCraftingListener.kt",
    CRAFTING + "silver/SilverCraftingPulse.kt",
    CRAFTING + "silver/SilverProduct.kt",
    "core/api/Container.kt",
    "core/api/InputType.kt",
    "core/game/event/Event.kt",
    "core/game/system/task/Pulse.java",
    "core/game/node/entity/skill/SkillPulse.java",
    "core/game/interaction/IntType.kt",
    "core/plugin/Plugin.java",
    "core/plugin/Initializable.java",
]
# Read for provenance only: adapters stand in for these, or pinned text is spliced from them.
OBSERVED = [
    "core/api/ContentAPI.kt",
    "core/cache/def/impl/ItemDefinition.java",
    "core/cache/def/impl/SceneryDefinition.java",
    "core/game/component/Component.java",
    "core/game/component/ComponentDefinition.java",
    "core/game/component/ComponentPlugin.java",
    "core/game/container/impl/EquipmentContainer.java",
    "core/game/event/Events.kt",
    "core/game/interaction/InteractionListener.kt",
    "core/game/interaction/InteractionListeners.kt",
    "core/game/interaction/InterfaceListener.kt",
    "core/game/interaction/OptionHandler.java",
    "core/game/node/entity/Entity.java",
    "core/game/node/entity/skill/Skills.java",
    "core/game/node/entity/impl/Animator.java",
    "core/game/node/entity/impl/PulseManager.java",
    "core/game/node/entity/player/link/diary/DiaryType.java",
    "core/game/node/entity/player/link/diary/AchievementDiary.java",
    "core/game/node/entity/player/link/diary/AchievementDiaryManager.java",
    "core/game/node/scenery/Scenery.java",
    "core/game/world/map/Location.kt",
    "core/game/world/update/flag/context/Animation.java",
    "core/game/container/Container.java",
]
JVM_FLAGS = ["-Xmx512m", "-Dfile.encoding=UTF-8", "-Duser.language=en", "-Duser.country=US", "-Duser.variant="]
SCOPE = (
    "Complete unchanged SpinSpinningWheelPlugin, SpinningInterface, SpinningItem, SpinningPulse, SilverCraftingListener, "
    "SilverCraftingPulse, SilverProduct, core.api.Container, InputType, Event, Pulse, SkillPulse, IntType, Plugin and "
    "Initializable, compiled with Kotlin 1.8.20/JDK 11 against the pinned ConstLib 1.4.28 artifact. ContentAPI's "
    "hasLevelDyn, rewardXP, submitIndividualPulse, itemDefinition, getAttribute, setAttribute and the Boolean "
    "sendInputDialogue, the ResourceProducedEvent declaration, EquipmentContainer's slot constants and Animation's two "
    "short constructors and create are pinned text spliced verbatim. Player, Item, the inventory (a map of amounts: "
    "contains and inInventory compare the amount held, a removal succeeds when at least the amount is held, "
    "Inventory.add always succeeds and addItem succeeds unless the case declares the inventory full), the level, the "
    "interface manager's open and close, sendMessage, animate, playAudio, openInterface, closeInterface, "
    "sendItemOnInterface, the InputType sendInputDialogue (which records the type and the prompt and keeps the handler), "
    "Skills.addExperience, the diary's two isComplete answers and finishTask, the hat, Location.withinDistance and "
    "getRegionId, Entity.dispatch, the attribute store (a map that drops a key's /save: prefix, as GameAttributes does) and "
    "PulseManager.run (which records the pulse's fields as constructed, calls its start() as the original manager does and "
    "keeps it) are recording adapters whose results come from declared case state; they are service inputs, not ports. "
    "ItemDefinition is an adapter whose name is a declared text of the id, not the cache's name. SceneryDefinition's "
    "handlers map and ComponentDefinition.put record what is put. OptionHandler, ComponentPlugin, InteractionListener and "
    "InterfaceListener are adapters that keep what is registered. Scenery is an adapter of an id with coordinates or with "
    "a type and a rotation. DiaryType is an adapter enum of the pinned constants' names. The Skills constants are pinned "
    "text spliced into an adapter shell. The driver calls SpinSpinningWheelPlugin.handle and getDestination with a scenery "
    "node it names, SpinningInterface.handle with a component 459, an opcode and a button it names and slot and item 0, "
    "the use-with lambda through a declared lookup of the pair as used with an Item and a Scenery of declared "
    "coordinates, the onOpen and on lambdas with a component, the last amount prompt's handler with an Integer or a "
    "String, and pulse() and stop() of the pulse the last run kept, not Pulse.update(): nothing here establishes on which "
    "ticks a pulse runs, that a finished pulse is stopped, or how PulseManager replaces a running pulse. Integer.parseInt "
    "is covered for the ASCII texts of the corpus only. No live world, original Container/Skills arithmetic, interface "
    "or option dispatch, input prompt, diary, attribute persistence or item definition behaviour is established by this "
    "corpus."
)


def artifact(lock, name, candidates):
    target = RUN / name
    def valid(path):
        return path.is_file() and path.stat().st_size == lock["size"] and hashlib.sha256(path.read_bytes()).hexdigest() == lock["sha256"]
    if not valid(target):
        for other in candidates:
            if valid(other):
                shutil.copyfile(other, target)
                break
        else:
            with urllib.request.urlopen(lock["url"], timeout=60) as response:
                data = response.read(lock["size"] + 1)
            if len(data) != lock["size"] or hashlib.sha256(data).hexdigest() != lock["sha256"]:
                raise RuntimeError(f"{name} differs from its pin")
            target.write_bytes(data)
    return target


def splices():
    api = read(SERVER / "core/api/ContentAPI.kt")
    animation = read(SERVER / "core/game/world/update/flag/context/Animation.java")
    diaries = re.findall(r"^    ([A-Z_]+)\(\"", read(SERVER / "core/game/node/entity/player/link/diary/DiaryType.java"), re.M)
    if len(diaries) != 6 or "SEERS_VILLAGE" not in diaries:
        raise RuntimeError("DiaryType differs from the six constants this adapter expects")
    produced = re.search(r"^data class ResourceProducedEvent\(.*$", read(SERVER / "core/game/event/Events.kt"), re.M)
    slots = re.search(r"public static final int SLOT_HAT = 0,[^;]*;", read(SERVER / "core/game/container/impl/EquipmentContainer.java"))
    return {
        "/* SKILL_CONSTANTS */": re.search(r"public static final int ATTACK = 0,[^;]*;", read(SERVER / "core/game/node/entity/skill/Skills.java")).group(),
        "/* SLOT_HAT */": slots.group(),
        "/* HAS_LEVEL_DYN */": c.method(api, "fun hasLevelDyn(player: Player, skill: Int, level: Int): Boolean"),
        "/* REWARD_XP */": c.method(api, "fun rewardXP(player: Player, skill: Int, amount: Double)"),
        "/* SUBMIT_INDIVIDUAL_PULSE */": c.method(api, "fun submitIndividualPulse(entity: Entity, pulse: Pulse)"),
        "/* SEND_INPUT_BOOLEAN */": c.method(api, "fun sendInputDialogue(player: Player, numeric: Boolean, prompt: String, handler: (value: Any) -> Unit)"),
        "/* ITEM_DEFINITION */": c.method(api, "fun itemDefinition(id: Int): ItemDefinition"),
        "/* GET_ATTRIBUTE */": c.method(api, "fun <T> getAttribute(entity: Entity, attribute: String, default: T): T"),
        "/* SET_ATTRIBUTE */": c.method(api, "fun <T> setAttribute(entity: Entity, attribute: String, value: T)"),
        "/* RESOURCE_PRODUCED_EVENT */": produced.group(),
        "/* DIARY_TYPES */": ", ".join(diaries),
        "/* ANIMATION_ID */": c.method(animation, "public Animation(int id)"),
        "/* ANIMATION_CREATE */": c.method(animation, "public static Animation create(int id)"),
        "/* ANIMATION_PRIORITY */": c.method(animation, "public Animation(int id, Priority priority)"),
    }


def generate(check):
    lock = json.loads(read(ROOT / "upstream.lock.json"))
    for part in ["server", "client"]:
        path = ROOT / "reference" / part
        if run(["git", "-C", path, "rev-parse", "HEAD"]).stdout.decode().strip() != lock[part]["commit"]:
            raise RuntimeError("Reference pin differs")
        run(["git", "-C", path, "diff", "--quiet", "HEAD", "--"])
    RUN.mkdir(parents=True, exist_ok=True)
    classes = RUN / "classes"
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    jars = c.artifacts()
    std = next(p for p in jars if p.name.startswith("kotlin-stdlib-"))
    simple = artifact(json.loads(read(ROOT / "oracle/auth/json.lock.json")), "json-simple-1.1.1.jar",
                      [ROOT / "runs/herblore-oracle/json-simple-1.1.1.jar", ROOT / "runs/gathering-oracle/json-simple-1.1.1.jar"])
    constants = json.loads(read(ROOT / "oracle/bank/constants.lock.json"))
    consts = artifact(constants, "ConstLib-1.4.28.jar",
                      [ROOT / "runs/herblore-oracle/ConstLib-1.4.28.jar", ROOT / "runs/gathering-oracle/ConstLib-1.4.28.jar"])
    java, kotlin = [], []
    for name in UNMODIFIED:
        path = SERVER / name
        read(path)
        (java if name.endswith(".java") else kotlin).append(path)
    for name in OBSERVED:
        read(SERVER / name)
    spliced = splices()
    staged = RUN / "adapters"
    if staged.exists():
        shutil.rmtree(staged)
    for path in sorted((HERE / "adapters").rglob("*")):
        if not path.is_file():
            continue
        text = read(path)
        for token, value in spliced.items():
            text = text.replace(token, value)
        target = staged / path.relative_to(HERE / "adapters")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8", newline="\n")
        (java if target.suffix == ".java" else kotlin).append(target)
    compile_cp = os.pathsep.join(map(str, [std, simple, consts]))
    cp = os.pathsep.join(map(str, [classes, std, simple, consts]))
    run(["java", "-Xmx1g", "-cp", os.pathsep.join(map(str, jars)), "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler", "-no-stdlib",
         "-no-reflect", "-nowarn", "-jvm-target", "11", "-classpath", compile_cp, "-d", classes, *kotlin, *java])
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", cp, "-d", classes, *java])
    cases = corpus.requests()
    source = RUN / "input.jsonl"
    source.write_text("".join(json.dumps(x) + "\n" for x in cases), encoding="utf-8", newline="\n")
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    with source.open("rb") as stdin:
        p = subprocess.run(list(map(str, [exe, *JVM_FLAGS, "-cp", cp, "oracle.Oracle"])), cwd=ROOT, stdin=stdin, capture_output=True, timeout=600)
    if p.returncode:
        raise RuntimeError(p.stderr.decode(errors="replace"))
    out = [json.loads(line) for line in p.stdout.splitlines() if line]
    assert len(out) == len(cases)
    data = "".join(json.dumps(x, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n" for x in out).encode()
    for module in sorted(HERE.glob("*.py")):
        read(module)
    read(ROOT / "oracle/login/generate.py")
    # The JVM may print option notices first; the version line is the one that names it.
    version = next(line for line in run(["java", "-version"]).stderr.decode().splitlines() if " version " in line)
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(cases),
                      fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm="sha256, CR removed",
                      kotlinCompiler="1.8.20", javaVersion=version, jvmFlags=JVM_FLAGS,
                      constants=dict(version=constants["version"], sha256=constants["sha256"]),
                      kinds=corpus.census(out), scope=SCOPE)
    folder = ROOT / "fixtures/crafting-stations"
    if check:
        if (folder / "oracle.jsonl").read_bytes() != data or json.loads((folder / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Crafting stations fixtures/provenance differ; inspect before regeneration")
        print(f"{len(out)} original crafting stations cases reproduced.")
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "oracle.jsonl").write_bytes(data)
        (folder / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated {len(out)} original crafting stations cases.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
