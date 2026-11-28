"""Unmodified fletching sources (logs, stringing, arrows) over a recording world.

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
RUN = ROOT / "runs/fletching-oracle"
SERVER = ROOT / "reference/server/Server/src/main"
spec = importlib.util.spec_from_file_location("compiler", ROOT / "oracle/login/generate.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
read, run = c.read, c.run
spec = importlib.util.spec_from_file_location("cases", HERE / "cases.py")
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)

FLETCHING = "content/global/skill/fletching/"
UNMODIFIED = [
    FLETCHING + "log/LogCraftableListeners.kt",
    FLETCHING + "log/CraftItemWithLogScript.kt",
    FLETCHING + "log/LogCraftInfo.kt",
    FLETCHING + "log/GrammarHelpers.kt",
    FLETCHING + "stringing/StringingListeners.kt",
    FLETCHING + "stringing/StringItemScript.kt",
    FLETCHING + "stringing/StringableCraftInfo.kt",
    FLETCHING + "arrow/ArrowListeners.kt",
    FLETCHING + "arrow/ArrowCraftInfo.kt",
    FLETCHING + "arrow/HeadlessArrowCraftScript.kt",
    FLETCHING + "arrow/TippedArrowCraftScript.kt",
    FLETCHING + "arrow/FlightedOgreArrowCraftScript.kt",
    FLETCHING + "bolts/BoltListeners.kt",
    FLETCHING + "bolts/BoltCraftScript.kt",
    FLETCHING + "bolts/BoltCraftInfo.kt",
    FLETCHING + "darts/DartListeners.kt",
    FLETCHING + "darts/DartCraftScript.kt",
    FLETCHING + "darts/DartCraftInfo.kt",
    FLETCHING + "crossbow/LimbingListener.kt",
    FLETCHING + "crossbow/LimbingScript.kt",
    FLETCHING + "crossbow/UnfinishedCrossbowCraftInfo.kt",
    FLETCHING + "gem/GemBoltListeners.kt",
    FLETCHING + "gem/AttachGemTipToBoltScript.kt",
    FLETCHING + "gem/CutGemsIntoBoltTipsScript.kt",
    FLETCHING + "gem/GemBoltsCraftInfo.kt",
    FLETCHING + "grapple/GrappleListeners.kt",
    FLETCHING + "Zones.kt",
    FLETCHING + "Feathers.kt",
    FLETCHING + "AchievementDiaryAttributeKeys.kt",
    "content/data/Quests.kt",
    "core/game/interaction/Clocks.kt",
    "core/game/interaction/IntType.kt",
    "core/api/Container.kt",
]
# Read for provenance only: adapters stand in for these, or pinned text is spliced from them.
OBSERVED = [
    "core/api/ContentAPI.kt",
    "core/api/ApiExtensions.kt",
    "core/game/dialogue/SkillDialogueHandler.kt",
    "core/game/interaction/InteractionListener.kt",
    "core/game/interaction/InteractionListeners.kt",
    "core/game/interaction/Script.kt",
    "core/game/node/entity/skill/Skills.java",
    "core/game/world/map/zone/ZoneBorders.java",
    "core/tools/StringUtils.java",
    "core/tools/RandomFunction.java",
    "content/global/skill/slayer/SlayerFlags.kt",
    "core/net/packet/context/ChildPositionContext.java",
]
JVM_FLAGS = ["-Xmx512m", "-Dfile.encoding=UTF-8", "-Duser.language=en", "-Duser.country=US", "-Duser.variant="]
SCOPE = (
    "Complete unchanged LogCraftableListeners, CraftItemWithLogScript, LogCraftInfo, GrammarHelpers, StringingListeners, "
    "StringItemScript, StringableCraftInfo, ArrowListeners, ArrowCraftInfo, HeadlessArrowCraftScript, "
    "TippedArrowCraftScript, FlightedOgreArrowCraftScript, BoltListeners, BoltCraftScript, BoltCraftInfo, DartListeners, "
    "DartCraftScript, DartCraftInfo, LimbingListener, LimbingScript, UnfinishedCrossbowCraftInfo, GemBoltListeners, "
    "AttachGemTipToBoltScript, CutGemsIntoBoltTipsScript, GemBoltsCraftInfo, GrappleListeners, Zones, Feathers, AchievementDiaryAttributeKeys, Quests, Clocks, "
    "IntType and the Container enum, compiled with Kotlin 1.8.20/JDK 11 against the pinned ConstLib 1.4.28 artifact. "
    "Player, Item (with declared names), the inventory (a map of amounts: a removal succeeds when at least the amount is "
    "held, an addition always succeeds), hasSpaceFor, levels, quest stages, the skilling clock, attributes (one map, the "
    "/save: prefix dropped), the diary manager, SlayerFlags, sendDialogue, sendMessage, rewardXP, animate, queueScript, "
    "keepRunning, stopExecuting, delayClock, delayScript, the animate function and PacketRepository are recording adapters whose results come from declared "
    "case state; they are service inputs, not ports. isQuestStarted, isQuestComplete, removeItemsIfPlayerHasEnough, StringUtils.isPlusN, "
    "RandomFunction.random(int,int) and random(int), SkillDialogue.forLength, the SkillDialogue constants and "
    "ZoneBorders' constructors and insideBorder methods are pinned text spliced into adapter shells; RandomFunction.RANDOM "
    "returns the case's declared draws. SkillDialogueHandler is an adapter that records open() with its type and items "
    "and keeps create/getAll overridable: its display, buttons and amount prompts are not covered. The driver calls a "
    "use-with handler through a declared lookup (the pair as used, then flipped), not InteractionListeners, and runs a "
    "queued script (the one a case names by its place in the queueing order) with the stage the case names, not "
    "ScriptProcessor: nothing here establishes when or in what order the processor runs the gem cutting loops. "
    "No live world, original Container/Skills arithmetic, dialogue interpreter or item definition behaviour is established by this corpus."
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
    zone = read(SERVER / "core/game/world/map/zone/ZoneBorders.java")
    random = read(SERVER / "core/tools/RandomFunction.java")
    dialogue = read(SERVER / "core/game/dialogue/SkillDialogueHandler.kt")
    constants = re.findall(r"^\s+([A-Z_]+\(\d+, \d+, \d+\)) \{$", dialogue, re.M)
    if len(constants) != 6:
        raise RuntimeError("SkillDialogue constants differ from the six this adapter expects")
    borders = [
        "public ZoneBorders(int x1, int y1, int x2, int y2) {",
        "public ZoneBorders(int x1, int y1, int x2, int y2, int plane) {",
        "public boolean insideBorder(Location location) {",
        "public boolean insideBorder(Node node) {",
        "public boolean insideBorder(int x, int y, int z) {",
        "public void setPlane(int plane) {",
    ]
    return {
        "/* SKILL_CONSTANTS */": re.search(r"public static final int ATTACK = 0,[^;]*;", read(SERVER / "core/game/node/entity/skill/Skills.java")).group(),
        "/* IS_QUEST_STARTED */": c.method(api, "fun isQuestStarted(player: Player, quest: Quests): Boolean"),
        "/* IS_QUEST_COMPLETE */": c.method(api, "fun isQuestComplete(player: Player, quest: Quests): Boolean"),
        "/* REMOVE_ITEMS_IF_PLAYER_HAS_ENOUGH */": c.method(api, "fun removeItemsIfPlayerHasEnough(player: Player, vararg items: Item): Boolean"),
        "/* IS_PLUS_N */": c.method(read(SERVER / "core/tools/StringUtils.java"), "public static boolean isPlusN(String word)"),
        "/* RANDOM_AB */": c.method(random, "public static final int random(int a, int b)"),
        "/* RANDOM_MAX */": c.method(random, "public static final int random(int maxValue)"),
        "/* SKILL_DIALOGUE_CONSTANTS */": ", ".join(constants),
        "/* FOR_LENGTH */": c.method(dialogue, "fun forLength(length2: Int): SkillDialogue?"),
        "/* ZONE_BORDERS_METHODS */": "\n    ".join(c.method(zone, signature) for signature in borders),
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
                      [ROOT / "runs/gathering-oracle/json-simple-1.1.1.jar", ROOT / "runs/doors-oracle/json-simple-1.1.1.jar"])
    constants = json.loads(read(ROOT / "oracle/bank/constants.lock.json"))
    consts = artifact(constants, "ConstLib-1.4.28.jar",
                      [ROOT / "runs/gathering-oracle/ConstLib-1.4.28.jar", ROOT / "runs/bank-oracle/ConstLib-1.4.28.jar"])
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
    names = RUN / "names.json"
    names.write_text(json.dumps(corpus.NAMES, sort_keys=True), encoding="utf-8", newline="\n")
    source = RUN / "input.jsonl"
    source.write_text("".join(json.dumps(x) + "\n" for x in cases), encoding="utf-8", newline="\n")
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    with source.open("rb") as stdin:
        p = subprocess.run(list(map(str, [exe, *JVM_FLAGS, "-cp", cp, "oracle.Oracle", names])), cwd=ROOT, stdin=stdin, capture_output=True, timeout=600)
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
                      kinds=corpus.census(out), itemNames=corpus.NAMES, scope=SCOPE)
    folder = ROOT / "fixtures/fletching"
    if check:
        if (folder / "oracle.jsonl").read_bytes() != data or json.loads((folder / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Fletching fixtures/provenance differ; inspect before regeneration")
        print(f"{len(out)} original fletching cases reproduced.")
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "oracle.jsonl").write_bytes(data)
        (folder / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated {len(out)} original fletching cases.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
