"""The command registry the original boot leaves behind (part of commands-oracle).

Builds on oracle/world-trace: the same unchanged compiled server, the same pinned call sites, the same world
directory and boot inputs. oracle.worldtrace.CommandRegistry runs WorldTrace.main's statements up to and including
the boot record, then writes CommandMapping (getNames() order, each command's fields and the class of its handler),
the legacy CommandSet plugin lists and getPageIndices(0..3). A run is accepted only if two fresh JVM runs agree byte
for byte and its boot record is byte-identical to the boot record of fixtures/world-trace/trace.jsonl, so nothing
here changed the start-up. Java/Kotlin are the test oracle only.
"""
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / "runs/commands-oracle/registry"
WORLD_TRACE = ROOT / "fixtures/world-trace/trace.jsonl"
spec = importlib.util.spec_from_file_location("worldtrace", ROOT / "oracle/world-trace/generate.py")
wt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wt)
read, run, c = wt.read, wt.run, wt.c

STUDIED = [
    "core/plugin/ClassScanner.kt",
    "core/game/system/command/Command.kt",
    "core/game/system/command/CommandSet.java",
    "core/game/system/command/CommandPlugin.kt",
    "core/game/system/command/sets/CommandSet.kt",
    "core/api/Commands.kt",
]


def build(classes, library, compiler):
    tool, pinned, harness = RUN / "tool", RUN / "pinned", RUN / "harness"
    for folder in [tool, pinned, harness]:
        if folder.exists():
            shutil.rmtree(folder)
        folder.mkdir(parents=True)
    embeddable = next(p for p in compiler if p.name.startswith("kotlin-compiler-embeddable-"))
    source = wt.HERE / "tool/Rewrite.java"
    read(source)
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", embeddable, "-d", tool, source])
    census = RUN / "census.json"
    run(["java", "-cp", os.pathsep.join(map(str, [tool, embeddable])), "Rewrite", classes, pinned, census])
    sources = sorted((wt.HERE / "harness").rglob("*.java")) + sorted((HERE / "harness").rglob("*.java"))
    for path in sources:
        read(path)
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", os.pathsep.join(map(str, [classes, *library])), "-d", harness,
         *sources])
    return os.pathsep.join(map(str, [pinned, harness, *library])), json.loads(census.read_text(encoding="utf-8"))


def play(passname, classpath, template, header):
    folder = RUN / passname
    if folder.exists():
        shutil.rmtree(folder)
    world = folder / "world"
    shutil.copytree(template, world)
    (folder / "tmp").mkdir()
    script = folder / "scenario.json"
    script.write_text(json.dumps(dict(name="commands-registry", clockStart=header["clockStart"], threadLocalSeed=header["threadLocalSeed"],
                                      steps=[])), encoding="utf-8", newline="\n")
    out = folder / "registry.jsonl"
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    command = [exe, *wt.JVM_FLAGS, f"-Djava.io.tmpdir={folder / 'tmp'}", "-cp", classpath, "oracle.worldtrace.CommandRegistry", script, out]
    with (folder / "stdout.log").open("wb") as o, (folder / "stderr.log").open("wb") as e:
        p = subprocess.run(list(map(str, command)), cwd=world, stdin=subprocess.DEVNULL, stdout=o, stderr=e, timeout=1800)
    if p.returncode or not out.is_file():
        tail = (folder / "stderr.log").read_bytes()[-4000:].decode(errors="replace")
        raise RuntimeError(f"command registry boot failed ({p.returncode}); see {folder}\n{tail}")
    return out.read_bytes()


def boot_digest(record):
    return wt.sha(json.dumps({k: v for k, v in record.items() if k != "i"}, sort_keys=True, separators=(",", ":")).encode())


def dump(lock):
    """The registry record, after checking the boot against the world-trace fixture. Returns (record, provenance part)."""
    trace = WORLD_TRACE.read_bytes().decode().splitlines()
    header, fixture_boot = json.loads(trace[0]), json.loads(trace[1])
    if header.get("name") != "boot" or fixture_boot.get("t") != "boot":
        raise RuntimeError("fixtures/world-trace/trace.jsonl does not start with the boot scenario")
    RUN.mkdir(parents=True, exist_ok=True)
    compiler = c.artifacts()
    jars = wt.artifacts()
    paths, tree_digest, count = wt.source_tree()
    for name in STUDIED:
        read(wt.SERVER / name)
    classes, library, version = wt.compile_server(jars, compiler, tree_digest, paths)
    classpath, sites = build(classes, library, compiler)
    saved = wt.RUN
    wt.RUN = RUN
    try:
        template, _, config = wt.world_template(lock)
    finally:
        wt.RUN = saved
    first = play("pass-1", classpath, template, header)
    second = play("pass-2", classpath, template, header)
    if first != second:
        raise RuntimeError(f"two fresh runs differ; see {RUN / 'pass-1'} and {RUN / 'pass-2'}")
    lines = [json.loads(line) for line in first.decode().splitlines() if line]
    boot = next(r for r in lines if r.get("t") == "boot")
    digest = boot_digest(boot)
    if digest != fixture_boot["sha256"]:
        theirs = {k: v for k, v in fixture_boot.items() if k not in ("i", "sha256")}
        keys = sorted(k for k in set(theirs) | set(boot) if k != "i" and boot.get(k) != theirs.get(k))
        raise RuntimeError(f"the boot record differs from the world-trace fixture in {keys}")
    record = next(r for r in lines if r.get("t") == "commands")
    for module in sorted(wt.HERE.glob("*.py")):
        read(module)
    provenance = dict(
        sourceTree=dict(root="reference/server/Server/src/main", files=count, manifestSha256=tree_digest),
        artifacts={name: wt.sha(path.read_bytes()) for name, path in sorted(jars.items())},
        javaVersion=version, jvmFlags=wt.JVM_FLAGS, redirectedCallSites=sites, config=config,
        bootRecord=dict(sha256=digest, identicalToWorldTrace=True, worldTraceFixtureSha256=wt.sha(WORLD_TRACE.read_bytes())),
        determinism=dict(freshJvmRuns=2, identical=True),
    )
    return record, provenance
