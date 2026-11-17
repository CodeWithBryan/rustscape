"""Unmodified gathering sources (woodcutting, mining, fishing, tools, tables) over a recording world.

A second corpus (binding.py) runs the same sources over the original Skills, Container and Item.

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
import sys
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / "runs/gathering-oracle"
SERVER = ROOT / "reference/server/Server/src/main"
spec = importlib.util.spec_from_file_location("compiler", ROOT / "oracle/login/generate.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
read, run = c.read, c.run
spec = importlib.util.spec_from_file_location("corpus", HERE / "corpus.py")
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)

UNMODIFIED = [
    "content/data/skill/SkillingTool.java",
    "content/global/skill/gather/SkillingResource.java",
    "content/global/skill/gather/woodcutting/WoodcuttingNode.java",
    "content/global/skill/gather/woodcutting/WoodcuttingListener.kt",
    "content/global/skill/gather/woodcutting/WoodcuttingSkillPulse.java",
    "content/global/skill/gather/mining/MiningNode.java",
    "content/global/skill/gather/mining/MiningListener.kt",
    "content/global/skill/gather/mining/MiningSkillPulse.kt",
    "content/global/skill/gather/mining/ProspectListener.kt",
    "core/tools/RandomFunction.java",
    "core/tools/Globals.kt",
    "core/game/node/item/ChanceItem.java",
    "core/game/node/item/WeightedChanceItem.java",
    "core/game/node/entity/npc/drop/DropFrequency.java",
    "core/game/system/task/Pulse.java",
    "core/game/interaction/Clocks.kt",
    "core/game/interaction/IntType.kt",
    "core/game/system/command/sets/StatAttributeKeys.kt",
    "core/api/EquipmentSlot.kt",
    "core/api/Container.kt",
    # Fishing (appended; the earlier entries and their cases are unchanged).
    "content/global/skill/fishing/Fish.kt",
    "content/global/skill/fishing/FishingOption.kt",
    "content/global/skill/fishing/FishingSpot.kt",
    "content/global/skill/gather/fishing/FishingListener.kt",
    "content/global/skill/gather/fishing/FishingPulse.kt",
    "core/game/node/entity/skill/SkillPulse.java",
    "core/game/node/entity/player/link/skillertasks/SkillTasks.java",
    "core/game/node/entity/player/link/skillertasks/Difficulty.java",
]
# Read for provenance only: adapters stand in for these, or the driver is modelled on them.
OBSERVED = [
    "core/game/node/entity/skill/Skills.java",
    "core/game/container/impl/EquipmentContainer.java",
    "core/game/interaction/ScriptProcessor.kt",
    "core/game/interaction/InteractionListener.kt",
    "core/api/ContentAPI.kt",
    "core/ServerConstants.kt",
    "core/game/world/map/Location.kt",
    "core/game/world/GameWorld.kt",
    "core/game/world/map/path/Pathfinder.java",
    "content/region/fremennik/diary/FremennikAchievementDiary.kt",
    "content/region/kandarin/quest/barbariantraining/BarbarianTraining.kt",
    "content/global/handlers/item/equipment/fistofguthixgloves/FOGGlovesManager.kt",
]
JVM_FLAGS = ["-Xmx512m", "-Dfile.encoding=UTF-8", "-Duser.language=en", "-Duser.country=US", "-Duser.variant="]
SCOPE = (
    "Complete unchanged WoodcuttingNode, WoodcuttingListener, WoodcuttingSkillPulse, MiningNode, MiningListener, "
    "MiningSkillPulse, ProspectListener, SkillingResource, SkillingTool, Fish, FishingOption, FishingSpot, FishingListener, "
    "FishingPulse, SkillPulse, SkillTasks, Difficulty, RandomFunction, ChanceItem, WeightedChanceItem, "
    "DropFrequency, Pulse, Clocks, IntType, StatAttributeKeys, EquipmentSlot, Container enum and Globals.kt, compiled "
    "with Kotlin 1.8.20/JDK 11 against the pinned ConstLib 1.4.28 artifact. Player, Skills, containers, item names, "
    "ContentAPI functions, script queue/clocks, scenery replacement, animation, audio, dialogue, projectile, diary, "
    "familiar, perk, timer, farming, bird-nest, firemaking, quest, zone and repository calls are recording adapters "
    "whose results come from declared case state; they are service inputs, not ports. RandomFunction.RANDOM, "
    "Math.random's generator and Collections.shuffle's generator are replaced by recording java.util.Random "
    "subclasses before first use; the thread-local generator's seed is set and read through Unsafe. The per-tick "
    "driver is modelled on ScriptProcessor.processInteractScript and is not the original processor. Default locale "
    "is pinned to en-US. For fishing, the spot NPC, familiar/Forager, Pathfinder, GameWorld (settings, ticks, Pulser), "
    "skill tasks, diary task, Fist of Guthix gloves and player direction/clocks are recording adapters; Direction is a "
    "declared step vector; Location.withinMaxnormDistance and transform(Direction) are the pinned expressions spliced into "
    "an adapter; the Fremennik task index and the barbarian-training key are spliced constants. Spot movement, barbarian "
    "fishing and the option handlers that start FishingPulse are not covered. No live world, original Container/Skills "
    "arithmetic or content registry behaviour is established by this corpus; see the binding corpus for Skills/Container."
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
                      [ROOT / "runs/doors-oracle/json-simple-1.1.1.jar"])
    constants = json.loads(read(ROOT / "oracle/bank/constants.lock.json"))
    consts = artifact(constants, "ConstLib-1.4.28.jar",
                      [ROOT / "runs/bank-constants-investigation/ConstLib-1.4.28.jar", ROOT / "runs/bank-oracle/ConstLib-1.4.28.jar"])
    java, kotlin = [], []
    for name in UNMODIFIED:
        path = SERVER / name
        read(path)
        (java if name.endswith(".java") else kotlin).append(path)
    for name in OBSERVED:
        read(SERVER / name)
    skills = re.search(r"public static final int ATTACK = 0,[^;]*;", read(SERVER / "core/game/node/entity/skill/Skills.java")).group()
    slots = re.search(r"public static final int SLOT_HAT = 0,[^;]*;", read(SERVER / "core/game/container/impl/EquipmentContainer.java")).group()
    # Fishing: constants and Location expressions taken from the pinned sources, never typed.
    location = read(SERVER / "core/game/world/map/Location.kt")
    splices = {
        "/* SKILL_CONSTANTS */": skills,
        "/* SLOT_CONSTANTS */": slots,
        "/* CATCH_TUNA_WITHOUT_HARPOON */": re.search(r"const val CATCH_TUNA_WITHOUT_HARPOON = \d+", read(SERVER / "content/region/fremennik/diary/FremennikAchievementDiary.kt")).group(),
        "/* ATTRIBUTE_FIST */": re.search(r"const val attributeFist\s*= \"[^\"]*\"", read(SERVER / "content/region/kandarin/quest/barbariantraining/BarbarianTraining.kt")).group(),
        "/* WITHIN_MAXNORM_DISTANCE */": re.search(r"fun withinMaxnormDistance\(other: Location, dist: Int\): Boolean = (.*)", location).group(1),
        "/* TRANSFORM_DIRECTION */": re.search(r"@JvmOverloads fun transform\(dir: Direction, steps: Int = 1\): Location = (.*)", location).group(1),
    }
    staged = RUN / "adapters"
    if staged.exists():
        shutil.rmtree(staged)
    for path in sorted((HERE / "adapters").rglob("*")):
        if not path.is_file():
            continue
        text = read(path)
        for token, value in splices.items():
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
    version = run(["java", "-version"]).stderr.decode().strip().splitlines()[0]
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(cases),
                      fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm="sha256, CR removed",
                      kotlinCompiler="1.8.20", javaVersion=version, jvmFlags=JVM_FLAGS, constants=dict(version=constants["version"], sha256=constants["sha256"]),
                      kinds=corpus.census(out), itemNames=corpus.NAMES, scope=SCOPE)
    folder = ROOT / "fixtures/gathering"
    if check:
        if (folder / "oracle.jsonl").read_bytes() != data or json.loads((folder / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Gathering fixtures/provenance differ; inspect before regeneration")
        print(f"{len(out)} original gathering cases reproduced.")
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "oracle.jsonl").write_bytes(data)
        (folder / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated {len(out)} original gathering cases.")
    # The second corpus: the same sources over the original Skills, Container and Item.
    spec = importlib.util.spec_from_file_location("binding", HERE / "binding.py")
    binding = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(binding)
    binding.generate(c, lock, check, UNMODIFIED, splices, jars, RUN, exe, JVM_FLAGS, corpus)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
