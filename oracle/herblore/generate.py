"""Unmodified herblore sources (cleaning herbs, mixing potions, the potion pulse) over a recording world.

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
RUN = ROOT / "runs/herblore-oracle"
SERVER = ROOT / "reference/server/Server/src/main"
spec = importlib.util.spec_from_file_location("compiler", ROOT / "oracle/login/generate.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
read, run = c.read, c.run
spec = importlib.util.spec_from_file_location("cases", HERE / "cases.py")
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)

HERBLORE = "content/global/skill/herblore/"
UNMODIFIED = [
    HERBLORE + "HerbCleanListener.kt",
    HERBLORE + "PotionMixingListener.kt",
    HERBLORE + "UnfinishedPotion.kt",
    HERBLORE + "FinishedPotion.kt",
    HERBLORE + "Herbs.java",
    HERBLORE + "GenericPotion.java",
    HERBLORE + "HerblorePulse.java",
    HERBLORE + "GrindingItem.java",
    "core/game/node/entity/skill/SkillPulse.java",
    "core/game/system/task/Pulse.java",
    "content/data/Quests.kt",
    "core/game/interaction/IntType.kt",
]
# Read for provenance only: adapters stand in for these, or pinned text is spliced from them.
OBSERVED = [
    "core/api/ContentAPI.kt",
    "core/game/dialogue/SkillDialogueHandler.kt",
    "core/game/interaction/InteractionListener.kt",
    "core/game/interaction/InteractionListeners.kt",
    "core/game/node/entity/skill/Skills.java",
    "core/game/node/entity/player/link/quest/QuestRepository.java",
    "core/game/node/entity/impl/PulseManager.java",
    "core/game/node/entity/impl/Animator.java",
    "core/game/container/Container.java",
    "core/tools/StringUtils.java",
    "core/tools/RandomFunction.java",
    "content/global/skill/skillcapeperks/SkillcapePerks.kt",
    "content/data/consumables/Consumables.java",
]
JVM_FLAGS = ["-Xmx512m", "-Dfile.encoding=UTF-8", "-Duser.language=en", "-Duser.country=US", "-Duser.variant="]
SCOPE = (
    "Complete unchanged HerbCleanListener, PotionMixingListener, UnfinishedPotion, FinishedPotion, Herbs, GenericPotion, "
    "HerblorePulse, GrindingItem, SkillPulse, Pulse, Quests and IntType, compiled with Kotlin 1.8.20/JDK 11 against the "
    "pinned ConstLib 1.4.28 artifact. Player, Item (with declared names and a slot the driver sets), the inventory (a map "
    "of amounts: containsItem compares the amount held, a removal succeeds when at least the amount is held, an addition "
    "always succeeds), the level, quest stages, attributes, lock, sendMessage, the packet dispatch's sendMessage, "
    "replaceSlot, rewardXP, addExperience, playAudio, animate, hasRequirement (true for the quests a case lists), "
    "Consumables.getConsumableById (a case's declared ids) and PulseManager.run (which records the pulse's potion and "
    "amount, calls its start() and keeps it) are recording adapters whose results come from declared case state; they "
    "are service inputs, not ports. QuestRepository.isComplete is the adapter's 'stage >= 100', the original's "
    "comparison for a registered quest. isQuestComplete, requireQuest, sendSkillDialogue, SkillDialogueBuilder, "
    "StringUtils.formatDisplayName, the Skills constants, the SkillDialogue constants with forLength, the BREWMASTER "
    "constant and SkillcapePerks.isActive are pinned text spliced into adapter shells; RandomFunction's two random "
    "methods are adapters with the original bodies whose RANDOM.nextInt is the case's next declared draw reduced into "
    "the bound. SkillDialogueHandler is an adapter that records open() with its type and items and keeps create/getAll "
    "overridable: its display, buttons and amount prompts are not covered. The driver calls the one 'clean' handler "
    "with an item of the id and slot an operation names, a use-with handler through a declared lookup (the pair as "
    "used, then flipped), not InteractionListeners, and SkillPulse.pulse() and stop() of the pulse the last run kept, "
    "not Pulse.update(): nothing here establishes on which ticks the pulse runs, only the delay it leaves. No live "
    "world, original Container/Skills arithmetic, option dispatch, dialogue interpreter or item definition behaviour "
    "is established by this corpus."
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
    dialogue = read(SERVER / "core/game/dialogue/SkillDialogueHandler.kt")
    perks = read(SERVER / "content/global/skill/skillcapeperks/SkillcapePerks.kt")
    constants = re.findall(r"^\s+([A-Z_]+\(\d+, \d+, \d+\)) \{$", dialogue, re.M)
    if len(constants) != 6:
        raise RuntimeError("SkillDialogue constants differ from the six this adapter expects")
    brewmaster = re.search(r'^\s+(BREWMASTER\("[^"]*"\)),$', perks, re.M)
    if not brewmaster:
        raise RuntimeError("SkillcapePerks.BREWMASTER differs from the constant this adapter expects")
    return {
        "/* SKILL_CONSTANTS */": re.search(r"public static final int ATTACK = 0,[^;]*;", read(SERVER / "core/game/node/entity/skill/Skills.java")).group(),
        "/* IS_QUEST_COMPLETE */": c.method(api, "fun isQuestComplete(player: Player, quest: Quests): Boolean"),
        "/* REQUIRE_QUEST */": c.method(api, "fun requireQuest(player: Player, quest: Quests, message: String) : Boolean"),
        "/* SEND_SKILL_DIALOGUE */": c.method(api, "fun sendSkillDialogue(player: Player, init: SkillDialogueBuilder.() -> Unit)"),
        "/* SKILL_DIALOGUE_BUILDER */": c.method(api, "class SkillDialogueBuilder"),
        "/* FORMAT_DISPLAY_NAME */": c.method(read(SERVER / "core/tools/StringUtils.java"), "public static String formatDisplayName(String name)"),
        "/* SKILL_DIALOGUE_CONSTANTS */": ", ".join(constants),
        "/* FOR_LENGTH */": c.method(dialogue, "fun forLength(length2: Int): SkillDialogue?"),
        "/* BREWMASTER */": brewmaster.group(1),
        "/* IS_ACTIVE */": c.method(perks, "fun isActive(perk: SkillcapePerks, player: Player): Boolean"),
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
    folder = ROOT / "fixtures/herblore"
    if check:
        if (folder / "oracle.jsonl").read_bytes() != data or json.loads((folder / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Herblore fixtures/provenance differ; inspect before regeneration")
        print(f"{len(out)} original herblore cases reproduced.")
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / "oracle.jsonl").write_bytes(data)
        (folder / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated {len(out)} original herblore cases.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
