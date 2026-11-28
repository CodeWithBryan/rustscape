"""Boot the whole unmodified original server and list what it registers ahead of DoorManagingPlugin.

Java/Kotlin are the test oracle only. --check never rewrites the fixture.
The compiled server, jars and world template are world-trace's (runs/world-trace-oracle); this adds runs/door-claims-oracle.
See docs/live-world-doors.md.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / "runs/door-claims-oracle"
SERVER = ROOT / "reference/server/Server/src/main"
FIXTURE = ROOT / "fixtures/door-claims"
spec = importlib.util.spec_from_file_location("worldtrace", ROOT / "oracle/world-trace/generate.py")
w = importlib.util.module_from_spec(spec)
spec.loader.exec_module(w)
c = w.c
read, run = c.read, c.run

# Read for provenance: the registries the harness lists and the dispatch order that makes them come first.
STUDIED = [
    "content/global/handlers/scenery/DoorManagingPlugin.java",
    "core/cache/def/impl/SceneryDefinition.java",
    "core/cache/def/Definition.java",
    "core/game/interaction/InteractionListeners.kt",
    "core/game/interaction/InteractionListener.kt",
    "core/game/interaction/IntType.kt",
    "core/game/interaction/PluginInteractionManager.java",
    "core/game/interaction/OptionHandler.java",
    "core/game/system/config/ServerConfigParser.kt",
    "core/game/world/GameWorld.kt",
    "core/plugin/ClassScanner.kt",
]
JVM_FLAGS = ["-Xmx4g", "-Xss4m", "-Djava.awt.headless=true", "-Dfile.encoding=UTF-8", "-Duser.language=en", "-Duser.country=US",
             "-Duser.variant=", "-Duser.timezone=UTC"]
SCOPE = (
    "The whole original server (every Kotlin and Java file under Server/src/main, compiled unchanged with Kotlin 1.8.20 "
    "and javac on JDK 11 against the dependencies its POM names, as oracle/world-trace compiles it) booted in one JVM by "
    "the original ServerConfigParser.parse and GameWorld.prompt(false) over world-trace's world template "
    "(worldprops/default.conf with enable_bots=false). After the boot the harness reads, by reflection, four "
    "registries for the option names DoorManagingPlugin registers as scenery defaults (open, close, shut, go-through): "
    "InteractionListeners.listeners (keys of the SCENERY type, by id and catch-all), InteractionListeners.interactions "
    "(defineInteraction metadata), PluginInteractionManager.objectInteractions (by scenery id, listed under all four "
    "options because the plugin is asked whatever the option) and each loaded SceneryDefinition's handlers "
    "('option:<name>' entries whose handler is not DoorManagingPlugin). Each row is 'id option kind class'; id -1 is "
    "a catch-all. The four comment lines name the default handler of each option. Established: which scenery id and "
    "option pairs the original registered other content for at start-up on this build. Not established: what that "
    "content does, registrations made after start-up, scenery that reaches an option through a morph (child id) and "
    "whether a listed scenery offers the option at all. Class names of Kotlin lambdas depend on the compiler version. "
    "Two fresh JVM runs must agree byte for byte before the fixture is written or accepted."
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def dump(name, classpath, template):
    folder = RUN / name
    if folder.exists():
        shutil.rmtree(folder)
    world = folder / "world"
    shutil.copytree(template, world)
    (folder / "tmp").mkdir()
    out = folder / "claims.txt"
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    command = [exe, *JVM_FLAGS, f"-Djava.io.tmpdir={folder / 'tmp'}", "-cp", classpath, "oracle.doorclaims.DoorClaims", out]
    with (folder / "stdout.log").open("wb") as stdout, (folder / "stderr.log").open("wb") as stderr:
        p = subprocess.run(list(map(str, command)), cwd=world, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, timeout=1800)
    if p.returncode or not out.is_file():
        tail = (folder / "stderr.log").read_bytes()[-4000:].decode(errors="replace")
        raise RuntimeError(f"{name} failed ({p.returncode}); see {folder}\n{tail}")
    return out.read_bytes()


def generate(check):
    lock = json.loads(read(ROOT / "upstream.lock.json"))
    for part in ["server", "client"]:
        path = ROOT / "reference" / part
        if run(["git", "-C", path, "rev-parse", "HEAD"]).stdout.decode().strip() != lock[part]["commit"]:
            raise RuntimeError("Reference pin differs")
        run(["git", "-C", path, "diff", "--quiet", "HEAD", "--"])
    RUN.mkdir(parents=True, exist_ok=True)
    w.RUN.mkdir(parents=True, exist_ok=True)
    compiler = c.artifacts()
    jars = w.artifacts()
    paths, tree_digest, count = w.source_tree()
    for name in STUDIED:
        read(SERVER / name)
    classes, library, version = w.compile_server(jars, compiler, tree_digest, paths)
    harness = RUN / "harness"
    if harness.exists():
        shutil.rmtree(harness)
    harness.mkdir(parents=True)
    sources = sorted((HERE / "harness").rglob("*.java"))
    for path in sources:
        read(path)
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", os.pathsep.join(map(str, [classes, *library])), "-d", harness,
         *sources])
    classpath = os.pathsep.join(map(str, [classes, harness, *library]))
    template, data_files, config = w.world_template(lock)
    first = dump("pass-1", classpath, template)
    second = dump("pass-2", classpath, template)
    if first != second:
        raise RuntimeError(f"two fresh runs differ; see {RUN / 'pass-1'} and {RUN / 'pass-2'}")
    rows = [line.split(" ") for line in first.decode().splitlines() if line and not line.startswith("#")]
    kinds = {}
    for row in rows:
        kinds[row[2]] = kinds.get(row[2], 0) + 1
    read(pathlib.Path(__file__).resolve())
    read(ROOT / "oracle/world-trace/generate.py")
    read(ROOT / "oracle/world-trace/artifacts.lock.json")
    read(ROOT / "oracle/login/generate.py")
    manifest = hashlib.sha256()
    for name in sorted(data_files):
        manifest.update(f"{name} {data_files[name]}\n".encode())
    provenance = dict(
        format=1, upstream=lock, scope=SCOPE, sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm="sha256, CR removed",
        sourceTree=dict(root="reference/server/Server/src/main", files=count, manifestSha256=tree_digest,
                        manifest="sha256 over 'relative/path sha256\\n' for every .kt and .java file in path order, CR removed"),
        artifacts={name: sha(path.read_bytes()) for name, path in sorted(jars.items())},
        compilerArtifacts={path.name: sha(path.read_bytes()) for path in sorted(compiler)},
        kotlinCompiler="1.8.20", kotlincFlags=w.KOTLINC_FLAGS, javacFlags=w.JAVAC_FLAGS, javaVersion=version, jvmFlags=JVM_FLAGS,
        world=dict(files=len(data_files), manifestSha256=manifest.hexdigest(), manifest="sha256 over 'path sha256\\n' in path order",
                   configuration=config),
        rows=len(rows), kinds=dict(sorted(kinds.items())), sceneryIds=len({row[0] for row in rows}),
        fixtureSha256=sha(first),
    )
    if check:
        if (FIXTURE / "claims.txt").read_bytes() != first or json.loads((FIXTURE / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Door claims fixture/provenance differ; inspect before regeneration")
        print(f"{len(rows)} door option claims of the booted original reproduced.")
    else:
        FIXTURE.mkdir(parents=True, exist_ok=True)
        (FIXTURE / "claims.txt").write_bytes(first)
        (FIXTURE / "provenance.json").write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
        print(f"{len(rows)} door option claims recorded.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
