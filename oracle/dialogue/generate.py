"""Original DialogueInterpreter/DialoguePlugin/DialogueFile differential corpus.

Compiles the unmodified dialogue classes with JDK 11 and Kotlin 1.8.20 against the
recording stand-ins in Standins.java, and drives the scenarios from cases.py
through them. --check never writes fixtures. Java/Kotlin are test oracles only.
"""
import argparse, hashlib, importlib.util, json, os, pathlib, re, subprocess

HERE = pathlib.Path(__file__).resolve().parent
ROOT = next(p for p in HERE.parents if (p / "upstream.lock.json").is_file())
RUN = ROOT / "runs/dialogue-oracle"
SERVER = ROOT / "reference/server/Server/src/main"

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

c = load("compiler", ROOT / "oracle/login/generate.py")

# Compiled unchanged, straight from the pinned reference checkout.
ORIGINAL = [
    "core/game/dialogue/DialogueInterpreter.java",
    "core/game/dialogue/DialoguePlugin.java",
    "core/game/dialogue/DialogueFile.kt",
    "core/game/dialogue/EmptyPlugin.kt",
    "core/game/dialogue/DialogueTopic.kt",
    "core/game/dialogue/FacialExpression.java",
    "core/game/dialogue/DialogueAction.java",
    "core/game/dialogue/SimpleDialoguePlugin.java",
    "core/game/event/Event.kt",
    "core/tools/DialogueConst.kt",
    "core/api/DialUtils.kt",
    "core/plugin/Plugin.java",
    "core/plugin/PluginManifest.java",
    "core/plugin/PluginType.java",
    "core/plugin/Initializable.java",
]
SCOPE = (
    "Complete unchanged DialogueInterpreter.java, DialoguePlugin.java, DialogueFile.kt, EmptyPlugin.kt, "
    "DialogueTopic.kt, FacialExpression.java, DialogueAction.java, SimpleDialoguePlugin.java, DialUtils.kt, "
    "DialogueConst.kt, Event.kt and the plugin annotations/interface, plus the three dialogue event classes "
    "copied verbatim from Events.kt at generation time. Player, NPC, Entity, Component, InterfaceManager, "
    "PacketDispatch, Item, ItemDefinition, NPCDefinition, GameWorld settings, PacketRepository, logging and "
    "System.err are recording stand-ins; scripted plugin/file content calls only inherited original methods. "
    "Reflection observes private fields and empties the static plugin registry between scenarios. Not packet "
    "bytes, real definitions, real interface lifecycle, concrete NPC/quest content, DialogueBuilder, "
    "DialogueLabeller, SkillDialogueHandler or a live session."
)

def sources():
    java, kotlin = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (java if name.endswith(".java") else kotlin).append(path)
    adapters = RUN / "adapters"
    def write(name, text):
        target = adapters / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8", newline="\n")
        (java if name.endswith(".java") else kotlin).append(target)
    for bundle in ["Standins.java", "Harness.java"]:
        parts = re.split(r"(?m)^//// (\S+)\n", c.read(HERE / bundle))
        for name, text in zip(parts[1::2], parts[2::2]):
            write(name, text)
    events = c.read(SERVER / "core/game/event/Events.kt")
    selected = re.findall(r"(?m)^data class Dialogue\w+Event\(.*\) : Event$", events)
    assert len(selected) == 3, selected
    write("core/game/event/Events.kt", "package core.game.event\nimport core.game.dialogue.DialoguePlugin\n" + "\n".join(selected) + "\n")
    return java, kotlin

def generate(check):
    lock = json.loads(c.read(ROOT / "upstream.lock.json"))
    actual = c.run(["git", "-C", ROOT / "reference/server", "rev-parse", "HEAD"]).stdout.decode().strip()
    assert actual == lock["server"]["commit"], "server reference differs from pin"
    c.run(["git", "-C", ROOT / "reference/server", "diff", "--quiet", "HEAD", "--"])
    classes = RUN / "classes"
    if classes.exists():
        for path in sorted(classes.rglob("*"), reverse=True):
            path.unlink() if path.is_file() else path.rmdir()
    classes.mkdir(parents=True, exist_ok=True)
    java, kotlin = sources()
    jars = c.artifacts()
    stdlib = next(p for p in jars if p.name.startswith("kotlin-stdlib-"))
    json_lock = json.loads(c.read(ROOT / "oracle/auth/json.lock.json"))
    json_jar = ROOT / "runs/interfaces-oracle/json-simple-1.1.1.jar"
    assert hashlib.sha256(json_jar.read_bytes()).hexdigest() == json_lock["sha256"], "json-simple differs from pin"
    cp = os.pathsep.join(map(str, [classes, stdlib, json_jar]))
    c.run(["java", "-Xmx768m", "-cp", os.pathsep.join(map(str, jars)), "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",
           "-no-stdlib", "-no-reflect", "-jvm-target", "11", "-nowarn", "-classpath", cp, "-d", classes, *kotlin, *java])
    c.run(["javac", "--release", "11", "-encoding", "UTF-8", "-nowarn", "-cp", cp, "-d", classes, *java])
    cases = load("cases", HERE / "cases.py")
    c.read(HERE / "cases.py")
    rows = cases.requests()
    data = "".join(json.dumps(row, ensure_ascii=True) + "\n" for row in rows).encode()
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    result = subprocess.run(list(map(str, [exe, "-Xmx768m", "-Xss2m", "-Dfile.encoding=UTF-8", "-cp", cp, "oracle.Oracle"])),
                            cwd=ROOT, input=data, capture_output=True, timeout=600)
    assert result.returncode == 0, result.stderr.decode(errors="replace") + result.stdout.decode(errors="replace")[-2000:]
    out = [json.loads(line) for line in result.stdout.splitlines()]
    assert len(out) == len(rows), (len(out), len(rows))
    fixture = "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in out).encode()
    c.read(pathlib.Path(__file__))
    initial = json.loads(c.read(HERE / "initial.lock.json"))
    prefix = b"".join(fixture.splitlines(keepends=True)[:initial["caseCount"]])
    assert hashlib.sha256(prefix).hexdigest() == initial["sha256"], "initial observations changed"
    provenance = dict(format=1, initialBaseline=initial, upstream=lock, sources=dict(sorted(c.SOURCES.items())),
                      caseCount=len(rows), snapshotCount=sum(len(row["expected"]) for row in out),
                      fixtureSha256=hashlib.sha256(fixture).hexdigest(), sourceHashAlgorithm="sha256, CR removed", scope=SCOPE)
    folder = ROOT / "fixtures/dialogue"
    if check:
        assert (folder / "oracle.jsonl").read_bytes() == fixture, "JVM output differs from the committed dialogue fixture"
        assert json.loads((folder / "provenance.json").read_text(encoding="utf-8")) == provenance, "dialogue fixture provenance is stale"
    else:
        folder.mkdir(exist_ok=True)
        (folder / "oracle.jsonl").write_bytes(fixture)
        (folder / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n", encoding="utf-8")
    print(len(rows), "original dialogue scenarios reproduced" if check else "original dialogue scenarios generated", flush=True)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    values = parser.parse_args()
    c.JAVA_HOME = values.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(values.check)
