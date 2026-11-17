"""Record who draws from the original's shared generators during start-up, and what each start-up phase adds.

Builds on oracle/startup-loaders (and through it oracle/world-trace): the same unchanged compiled server, pinned call
sites, world directory and boot inputs. oracle.worldtrace.StartupDraws runs the statements of
StartupPhases.startup with the three counting generators replaced by counting generators that also record the call
site of every next(bits) call, and reads after each phase which NPCs it added to the repository, the region cache,
the definition tables, the ground items and the chunk dirty set (read only).

A run is accepted only if (a) two fresh JVM runs agree byte for byte, (b) its phase and table lines are identical,
line for line, to those of fixtures/startup-loaders/oracle.jsonl (so recording changed no draw, log line or table),
and (c) its boot record is byte-identical to the boot record of fixtures/world-trace/trace.jsonl.
Java/Kotlin are the test oracle only. --check never rewrites the fixture. See docs/startup.md.
"""
import argparse
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / "runs/startup-draws-oracle"
FIXTURE = ROOT / "fixtures/startup-draws"
LOADERS_FIXTURE = ROOT / "fixtures/startup-loaders/oracle.jsonl"
WORLD_TRACE = ROOT / "fixtures/world-trace/trace.jsonl"
spec = importlib.util.spec_from_file_location("startuploaders", ROOT / "oracle/startup-loaders/generate.py")
sl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sl)
wt, c = sl.wt, sl.c
read, run, sha = c.read, c.run, sl.sha

SCHEMA = 1
# The original sources whose draws and registrations this corpus attributes; read for provenance.
STUDIED = [
    "core/game/world/GameWorld.kt",
    "core/plugin/ClassScanner.kt",
    "core/game/system/config/NPCSpawner.kt",
    "core/game/system/config/GroundSpawnLoader.kt",
    "core/game/node/entity/npc/NPC.java",
    "core/game/node/entity/npc/AbstractNPC.java",
    "core/game/node/entity/Entity.java",
    "core/game/system/timer/TimerRegistry.kt",
    "core/game/system/timer/TimerManager.kt",
    "core/game/system/timer/RSTimer.kt",
    "core/game/system/timer/impl/AntiMacro.kt",
    "core/game/system/timer/impl/SkillRestore.kt",
    "core/game/ge/GrandExchangeTimer.kt",
    "core/game/worldevents/holiday/HolidayRandoms.kt",
    "core/game/node/item/GroundItemManager.java",
    "core/game/world/map/RegionChunk.java",
    "core/game/world/update/ChunkDirtyListener.kt",
    "core/tools/RandomFunction.java",
]
SCOPE = (
    "The original start-up of oracle/startup-loaders, statement by statement, with RandomFunction.RANDOM, Math.random "
    "and Collections.shuffle replaced by seeded counting generators that also record, per next(bits) call, the "
    "java.util.Random/Math/Collections method that called it, the RandomFunction method, the first non-library "
    "frame, the innermost content.* frame, the phase's own entry frame and the frame that entry called. Per phase, "
    "read only: the NPC slots added to Repository.npcs (ranges), NPCs whose class is not the base NPC, shared draws "
    "by Repository.npcs.size() at draw time, the region cache size and key digest, NPC definition count and key "
    "digest, scenery definition count, GroundItemManager's list size, ChunkUpdateTracker's dirty set size, the "
    "renderable NPC count and the class of RegionChunk.dirtyListener. Accepted only if its phase and table lines equal "
    "fixtures/startup-loaders/oracle.jsonl line for line and its boot record is byte-identical to the world-trace "
    "fixture's. Not covered: thread-local draws (ThreadLocalRandom cannot be intercepted; its seed per phase is in the "
    "startup-loaders phases), the order of draws inside a phase beyond the per-size counts, other JDKs/operating systems."
)


def build(classes, library, compiler):
    """Pins the call sites into a copy of the class files and compiles all three harnesses together."""
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
    sources = sorted((wt.HERE / "harness").rglob("*.java")) + sorted((sl.HERE / "harness").rglob("*.java")) + sorted((HERE / "harness").rglob("*.java"))
    for path in sources:
        read(path)
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", os.pathsep.join(map(str, [classes, *library])), "-d", harness, *sources])
    return os.pathsep.join(map(str, [pinned, harness, *library])), json.loads(census.read_text(encoding="utf-8"))


def play(passname, classpath, template, header):
    folder = RUN / passname
    if folder.exists():
        shutil.rmtree(folder)
    world = folder / "world"
    shutil.copytree(template, world)
    (folder / "tmp").mkdir()
    out = folder / "draws.jsonl"
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    command = [exe, *wt.JVM_FLAGS, f"-Djava.io.tmpdir={folder / 'tmp'}", "-cp", classpath, "oracle.worldtrace.StartupDraws",
               out, str(header["clockStart"]), str(header["threadLocalSeed"])]
    with (folder / "stdout.log").open("wb") as o, (folder / "stderr.log").open("wb") as e:
        p = subprocess.run(list(map(str, command)), cwd=world, stdin=subprocess.DEVNULL, stdout=o, stderr=e, timeout=1800)
    if p.returncode or not out.is_file():
        tail = (folder / "stderr.log").read_bytes()[-4000:].decode(errors="replace")
        raise RuntimeError(f"start-up draw attribution failed ({p.returncode}); see {folder}\n{tail}")
    return out.read_bytes()


def generate(check):
    lock = json.loads(read(ROOT / "upstream.lock.json"))
    for part in ["server", "client"]:
        path = ROOT / "reference" / part
        if run(["git", "-C", path, "rev-parse", "HEAD"]).stdout.decode().strip() != lock[part]["commit"]:
            raise RuntimeError("Reference pin differs")
        run(["git", "-C", path, "diff", "--quiet", "HEAD", "--"])
    trace = WORLD_TRACE.read_bytes().decode().splitlines()
    header, fixture_boot = json.loads(trace[0]), json.loads(trace[1])
    if header.get("name") != "boot" or fixture_boot.get("t") != "boot":
        raise RuntimeError("fixtures/world-trace/trace.jsonl does not start with the boot scenario")
    loader_lines = [l for l in LOADERS_FIXTURE.read_bytes().decode().splitlines() if json.loads(l)["t"] in ("phase", "table")]
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
        template, data_files, config = wt.world_template(lock)
    finally:
        wt.RUN = saved
    first = play("pass-1", classpath, template, header)
    second = play("pass-2", classpath, template, header)
    if first != second:
        a, b = first.decode().splitlines(), second.decode().splitlines()
        where = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
        raise RuntimeError(f"two fresh runs differ at line {where + 1}; see {RUN / 'pass-1'} and {RUN / 'pass-2'}")
    lines = first.decode().splitlines()
    boot = json.loads(lines[0])
    if boot.get("t") != "boot":
        raise RuntimeError("the harness did not write the boot record first")
    digest = sl.boot_digest(boot)
    if digest != fixture_boot["sha256"]:
        theirs = {k: v for k, v in fixture_boot.items() if k not in ("i", "sha256")}
        keys = sorted(k for k in set(theirs) | set(boot) if k != "i" and boot.get(k) != theirs.get(k))
        raise RuntimeError(f"the attributed start-up's boot record differs from the world-trace fixture in {keys}")
    marker = lines.index("DRAWS")
    phases = lines[lines.index("PHASES") + 1:marker]
    if phases != loader_lines:
        where = next((i for i, (x, y) in enumerate(zip(phases, loader_lines)) if x != y), min(len(phases), len(loader_lines)))
        raise RuntimeError(f"phase/table line {where + 1} differs from fixtures/startup-loaders/oracle.jsonl: recording changed the start-up")
    body = ("\n".join(lines[marker + 1:]) + "\n").encode()
    records = [json.loads(l) for l in lines[marker + 1:]]
    # Each phase's attributed draws must add up to what the startup-loaders fixture recorded for it.
    previous = {"shared": 0, "math": 0, "shuffle": 0}
    for record, line in zip(records, [l for l in loader_lines if json.loads(l)["t"] == "phase"]):
        phase = json.loads(line)
        if phase["name"] != record["name"]:
            raise RuntimeError(f"phase order differs at {record['name']}")
        for g in previous:
            drawn = sum(d["count"] for d in record["draws"] if d["generator"] == g)
            now = phase["generators"][g][0]
            if drawn != now - previous[g]:
                raise RuntimeError(f"{record['name']}: {drawn} attributed {g} draws, {now - previous[g]} counted")
            previous[g] = now
    for module in sorted(HERE.glob("*.py")) + sorted(sl.HERE.glob("*.py")) + sorted(wt.HERE.glob("*.py")):
        read(module)
    read(ROOT / "oracle/login/generate.py")
    provenance = dict(
        format=1, schema=SCHEMA, upstream=lock, scope=SCOPE,
        sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm="sha256, CR removed",
        sourceTree=dict(root="reference/server/Server/src/main", files=count, manifestSha256=tree_digest),
        artifacts={name: sha(path.read_bytes()) for name, path in sorted(jars.items())},
        compilerArtifacts={path.name: sha(path.read_bytes()) for path in sorted(compiler)},
        kotlinCompiler="1.8.20", javaVersion=version, jvmFlags=wt.JVM_FLAGS, redirectedCallSites=sites, config=config,
        bootRecord=dict(sha256=digest, identicalToWorldTrace=True, worldTraceFixtureSha256=sha(WORLD_TRACE.read_bytes())),
        startupLoaders=dict(phaseAndTableLinesIdentical=True, fixtureSha256=sha(LOADERS_FIXTURE.read_bytes())),
        counts=dict(phases=len(records), drawKeys=sum(len(r["draws"]) for r in records)),
        determinism=dict(freshJvmRuns=2, identical=True),
        fixtureSha256=sha(body),
    )
    if check:
        same = (FIXTURE / "draws.jsonl").read_bytes() == body
        if not same or json.loads((FIXTURE / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Startup-draws fixture/provenance differ; inspect before regeneration")
        print(f"Original start-up draws attributed in {len(records)} phases; phases identical to the startup-loaders fixture; "
              f"boot record identical to the world-trace fixture; two fresh runs identical.")
    else:
        FIXTURE.mkdir(parents=True, exist_ok=True)
        (FIXTURE / "draws.jsonl").write_bytes(body)
        (FIXTURE / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated the original start-up's draw attribution in {len(records)} phases; phases identical to the "
              f"startup-loaders fixture; boot record identical to the world-trace fixture; two fresh runs identical.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
