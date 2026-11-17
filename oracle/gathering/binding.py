"""Second gathering corpus: the same unmodified listeners and pulses over the ORIGINAL Skills, Container and Item.

The adapter Skills/Container/Item models of the first corpus are replaced by the pinned classes, and the ContentAPI
functions that reach them are the pinned function texts. Everything else stays a recording adapter. The output is
fixtures/gathering/binding.jsonl; it is produced and checked by generate.py.
"""
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SERVER = ROOT / "reference/server/Server/src/main"
ORIGINAL = [
    "core/game/node/entity/skill/Skills.java",
    "content/global/handlers/item/equipment/brawling_gloves/BrawlingGloves.java",
    "core/cache/def/Definition.java",
    "core/game/node/item/Item.java",
    "core/game/container/Container.java",
    "core/game/container/ContainerEvent.java",
    "core/game/container/ContainerListener.java",
    "core/game/container/ContainerType.java",
    "core/game/container/SortType.java",
]
# First-corpus adapters that the pinned classes or the overlay replace.
REPLACED = [
    "core/game/container/Container.java",
    "core/game/node/item/Item.java",
    "core/game/node/entity/skill/Skills.java",
    "core/cache/def/impl/ItemDefinition.java",
]
# Recording ContentAPI stand-ins dropped in favour of the pinned function texts below.
DROPPED = ["addItem", "addItemOrDrop", "freeSlots", "getDynLevel", "rewardXP", "getItemName", "getItemFromEquipment", "inEquipment",
           "removeItem", "inInventory", "hasLevelDyn", "hasSpaceFor"]
PINNED = ["fun inInventory(", "fun hasLevelDyn(", "fun hasSpaceFor(", "fun freeSlots(", "fun addItem(", "fun addItemOrDrop(", "fun <T> removeItem(",
          "fun rewardXP(", "fun getDynLevel(", "fun getItemFromEquipment(", "fun getItemName("]
DEFINITION_METHODS = ["public ItemDefinition()", "public static ItemDefinition forId(", "public static Map<Integer, ItemDefinition> getDefinitions()",
                      "public boolean isStackable()", "public int getValue()", "public int getAlchemyValue(", "public int getNoteId()",
                      "public boolean isUnnoted()", "public ItemPlugin getItemPlugin()", "public static OptionHandler getOptionHandler("]
NODE_METHODS = ["public Node(", "public String getName()", "public int getIndex()", "public void setIndex(", "public DestinationFlag getDestinationFlag()"]
SCOPE = (
    "The unmodified WoodcuttingListener, MiningListener, FishingListener and FishingPulse (with the tables and RandomFunction of "
    "the first corpus) run over the complete unchanged Skills, Container, ContainerEvent, ContainerListener, ContainerType, SortType, "
    "Item, Definition and BrawlingGloves, with verbatim selected ItemDefinition and Node methods and the verbatim ContentAPI "
    "functions inInventory, hasLevelDyn, hasSpaceFor, freeSlots, addItem, addItemOrDrop, removeItem, rewardXP, getDynLevel, "
    "getItemFromEquipment and getItemName. Equipment is the base Container with fourteen slots, not EquipmentContainer. "
    "Inventory and equipment carry one recording ContainerListener each, not InventoryListener/EquipmentListener. No assist "
    "session and no brawling gloves exist. inEquipment (item-configuration equip slot), packets, level-up, XP plugins/events, "
    "ground items, attributes, clocks, scripts, messages, familiars, perks, generators and every other system remain "
    "recording adapters. The per-tick driver is the modelled one of the first corpus."
)


def remove_functions(text, names):
    """Drops top-level Kotlin functions: a block ends at a column-0 brace, an expression body at the next column-0 line."""
    lines, out, index = text.split("\n"), [], 0
    while index < len(lines):
        line = lines[index]
        match = re.match(r"(?:inline )?fun (?:<[^>]*> )?(?:Int\.)?(\w+)\(", line)
        if match and match.group(1) in names:
            block = line.rstrip().endswith("{")
            index += 1
            while index < len(lines) and (lines[index] != "}" if block else lines[index].startswith((" ", "\t"))):
                index += 1
            index += 1 if block else 0
            continue
        out.append(line)
        index += 1
    return "\n".join(out)


def generate(c, lock, check, unmodified, splices, jars, run_dir, exe, jvm_flags, corpus):
    read, run = c.read, c.run
    c.SOURCES.clear()
    work = run_dir / "binding"
    if work.exists():
        shutil.rmtree(work)
    classes, staged = work / "classes", work / "adapters"
    classes.mkdir(parents=True)
    std = next(p for p in jars if p.name.startswith("kotlin-stdlib-"))
    simple, consts = run_dir / "json-simple-1.1.1.jar", run_dir / "ConstLib-1.4.28.jar"
    java, kotlin = [], []
    for name in unmodified + ORIGINAL:
        path = SERVER / name
        read(path)
        (java if name.endswith(".java") else kotlin).append(path)
    api = read(SERVER / "core/api/ContentAPI.kt")
    definition = read(SERVER / "core/cache/def/impl/ItemDefinition.java")
    node = read(SERVER / "core/game/node/Node.java")
    config = read(SERVER / "core/game/system/config/ItemConfigParser.kt")
    extra = dict(splices)
    extra["/* PINNED_CONTENT_API */"] = "\n\n".join(c.method(api, signature) for signature in PINNED)
    extra["/* ITEM_DEFINITION_MEMBERS */"] = (
        definition[definition.index("private static final Map<Integer, ItemDefinition> DEFINITIONS"):definition.index("public ItemDefinition()")]
        + "\n".join(c.method(definition, m) for m in DEFINITION_METHODS))
    extra["/* NODE_METHODS */"] = "\n".join(c.method(node, m) for m in NODE_METHODS)
    extra["/* ALCHEMY_KEYS */"] = "".join(
        "public static String " + name + "=" + re.search(r"const val " + name + r' = ("[^"]+")', config).group(1) + ";" for name in ["HIGH_ALCHEMY", "LOW_ALCHEMY"])
    files = {}
    for path in sorted((HERE / "adapters").rglob("*")):
        if path.is_file() and path.relative_to(HERE / "adapters").as_posix() not in REPLACED:
            files[path.relative_to(HERE / "adapters").as_posix()] = path
    for path in sorted((HERE / "binding/adapters").rglob("*")):
        if path.is_file():
            files[path.relative_to(HERE / "binding/adapters").as_posix()] = path
    for name, path in sorted(files.items()):
        text = read(path)
        if name == "core/api/ContentAPI.kt":
            text = remove_functions(text, DROPPED) + "\n" + read(HERE / "binding/ContentAPI.tail.kt")
            text = text.replace("import oracle.Support\n", "import oracle.Support\nimport core.cache.def.impl.ItemDefinition\nimport core.game.node.item.GroundItemManager\n"
                                "import content.global.skill.summoning.familiar.BurdenBeast\n", 1)
        for token, value in extra.items():
            text = text.replace(token, value)
        target = staged / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8", newline="\n")
        (java if target.suffix == ".java" else kotlin).append(target)
    compile_cp = os.pathsep.join(map(str, [std, simple, consts]))
    cp = os.pathsep.join(map(str, [classes, std, simple, consts]))
    run(["java", "-Xmx1g", "-cp", os.pathsep.join(map(str, jars)), "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler", "-no-stdlib",
         "-no-reflect", "-nowarn", "-jvm-target", "11", "-classpath", compile_cp, "-d", classes, *kotlin, *java])
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", cp, "-d", classes, *java])
    spec = importlib.util.spec_from_file_location("cases_binding", HERE / "cases_binding.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    cases = module.cases(corpus.Helpers)
    names = work / "names.json"
    names.write_text(json.dumps(corpus.NAMES, sort_keys=True), encoding="utf-8", newline="\n")
    source = work / "input.jsonl"
    source.write_text("".join(json.dumps(x) + "\n" for x in cases), encoding="utf-8", newline="\n")
    with source.open("rb") as stdin:
        p = subprocess.run(list(map(str, [exe, *jvm_flags, "-cp", cp, "oracle.Binding", names])), cwd=ROOT, stdin=stdin, capture_output=True, timeout=600)
    if p.returncode:
        raise RuntimeError(p.stderr.decode(errors="replace"))
    out = [json.loads(line) for line in p.stdout.splitlines() if line]
    assert len(out) == len(cases)
    data = "".join(json.dumps(x, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n" for x in out).encode()
    for module_path in [HERE / "binding.py", HERE / "cases_binding.py", HERE / "corpus.py", HERE / "generate.py", ROOT / "oracle/login/generate.py"]:
        read(module_path)
    version = run(["java", "-version"]).stderr.decode().strip().splitlines()[0]
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(cases),
                      fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm="sha256, CR removed", kotlinCompiler="1.8.20",
                      javaVersion=version, jvmFlags=jvm_flags, kinds=corpus.census(out), itemNames=corpus.NAMES, scope=SCOPE)
    folder = ROOT / "fixtures/gathering"
    if check:
        if (folder / "binding.jsonl").read_bytes() != data or json.loads((folder / "binding.provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Gathering binding fixtures/provenance differ; inspect before regeneration")
        print(f"{len(out)} original gathering binding cases reproduced.")
    else:
        (folder / "binding.jsonl").write_bytes(data)
        (folder / "binding.provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated {len(out)} original gathering binding cases.")
