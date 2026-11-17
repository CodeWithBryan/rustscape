"""Construct original Players in a booted original world and record what they hold.

Builds on oracle/world-trace exactly as oracle/startup-loaders does: the same unchanged compiled server, the same
pinned call sites, the same world directory and boot inputs. oracle.worldtrace.FreshPlayer boots the world as
WorldTrace does, then constructs `new Player(PlayerDetails.getDetails(name))` and records every instance field of
Node, Entity, Player, Properties, Skills, PlayerDetails, UserAccountInfo, UIDInfo, CommunicationInfo,
SpellBookManager, FamiliarManager and Settings, the registries a player's construction and save read, and
`PlayerSaver.populate()` (with each content hook's own output). It then loads each distinct character file of the
world-trace fixture into a new Player with PlayerSaveParser.parse and runContentHooks and records the same again.

A run is accepted only if (a) two fresh JVM runs agree byte for byte and (b) its boot record is byte-identical to
the boot record of fixtures/world-trace/trace.jsonl. Java/Kotlin are the test oracle only. --check never rewrites
the fixture. See docs/canonical-player.md.
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
RUN = ROOT / "runs/canonical-player-oracle"
FIXTURE = ROOT / "fixtures/canonical-player"
WORLD_TRACE = ROOT / "fixtures/world-trace/trace.jsonl"
spec = importlib.util.spec_from_file_location("worldtrace", ROOT / "oracle/world-trace/generate.py")
wt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wt)
read, run, c = wt.read, wt.run, wt.c
spec = importlib.util.spec_from_file_location("startuploaders", ROOT / "oracle/startup-loaders/generate.py")
sl = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sl)

SCHEMA = 1
NAME = "traceone"
STUDIED = [
    "core/game/node/Node.java",
    "core/game/node/entity/Entity.java",
    "core/game/node/entity/player/Player.java",
    "core/game/node/entity/impl/Properties.java",
    "core/game/node/entity/skill/Skills.java",
    "core/game/node/entity/player/info/PlayerDetails.java",
    "core/game/node/entity/player/info/UIDInfo.java",
    "core/auth/UserAccountInfo.kt",
    "core/game/system/communication/CommunicationInfo.java",
    "core/game/node/entity/player/link/SpellBookManager.java",
    "core/game/node/entity/player/link/quest/QuestRepository.java",
    "core/game/node/entity/player/link/music/MusicEntry.java",
    "core/game/node/entity/player/info/login/PlayerSaver.kt",
    "core/game/node/entity/player/info/login/PlayerSaveParser.kt",
    "core/api/PersistPlayer.kt",
    "core/ServerConstants.kt",
]
SCOPE = (
    "The whole original server compiled unchanged (as oracle/world-trace), booted as WorldTrace boots it (boot record "
    "byte-identical to the world-trace fixture's). Then the original constructor builds Players: for a fresh one and "
    "for each distinct character file the world-trace fixture holds, loaded with PlayerSaveParser.parse and "
    "runContentHooks. Recorded by reflection: every instance field of Node, Entity, Player, Properties, Skills, "
    "PlayerDetails, UserAccountInfo, UIDInfo, CommunicationInfo, SpellBookManager, FamiliarManager and Settings, as "
    "a value or, for other objects, as their class; registered quests, songs, the start location, save version and "
    "content hooks; generator positions and clock reads; PlayerSaver.populate's text and what each content hook "
    "wrote into it. Not covered: the field values of the other objects a Player holds, Player.init, login, ticks, "
    "other JDKs and operating systems."
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def saves():
    """Distinct character files the world-trace fixture holds, in fixture order: (name, bytes)."""
    seen, out = set(), []
    for line in WORLD_TRACE.read_bytes().decode().splitlines():
        record = json.loads(line)
        for f in record.get("files", []):
            if "hex" not in f:
                continue
            data = bytes.fromhex(f["hex"])
            if sha(data) in seen:
                continue
            seen.add(sha(data))
            out.append((pathlib.PurePosixPath(f["path"]).stem, data))
    return out


def build(classes, library, compiler):
    """Pins the call sites into a copy of the class files and compiles the world-trace adapters with this harness."""
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
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", os.pathsep.join(map(str, [classes, *library])), "-d", harness, *sources])
    return os.pathsep.join(map(str, [pinned, harness, *library])), json.loads(census.read_text(encoding="utf-8"))


def play(passname, classpath, template, header, files):
    folder = RUN / passname
    if folder.exists():
        shutil.rmtree(folder)
    world = folder / "world"
    shutil.copytree(template, world)
    (folder / "tmp").mkdir()
    args = []
    for index, (name, data) in enumerate(files):
        path = folder / f"save-{index}.hex"
        path.write_bytes(data.hex().encode())
        args += [name, str(path)]
    out = folder / "players.jsonl"
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    command = [exe, *wt.JVM_FLAGS, f"-Djava.io.tmpdir={folder / 'tmp'}", "-cp", classpath, "oracle.worldtrace.FreshPlayer",
               str(header["clockStart"]), str(header["threadLocalSeed"]), NAME, out, *args]
    with (folder / "stdout.log").open("wb") as o, (folder / "stderr.log").open("wb") as e:
        p = subprocess.run(list(map(str, command)), cwd=world, stdin=subprocess.DEVNULL, stdout=o, stderr=e, timeout=1800)
    if p.returncode or not out.is_file():
        tail = (folder / "stderr.log").read_bytes()[-4000:].decode(errors="replace")
        raise RuntimeError(f"fresh-player run failed ({p.returncode}); see {folder}\n{tail}")
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
    RUN.mkdir(parents=True, exist_ok=True)
    compiler = c.artifacts()
    jars = wt.artifacts()
    paths, tree_digest, count = wt.source_tree()
    for name in STUDIED:
        read(wt.SERVER / name)
    # The compiled classes are shared with world-trace-oracle (same inputs, same cache key).
    classes, library, version = wt.compile_server(jars, compiler, tree_digest, paths)
    classpath, sites = build(classes, library, compiler)
    saved = wt.RUN
    wt.RUN = RUN
    try:
        template, data_files, config = wt.world_template(lock)
    finally:
        wt.RUN = saved
    files = saves()
    first = play("pass-1", classpath, template, header, files)
    second = play("pass-2", classpath, template, header, files)
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
        raise RuntimeError(f"the boot record differs from the world-trace fixture in {keys}")
    end = json.loads(lines[-1])
    if end.get("t") != "end" or end["refused"] or end["unpinned"] or end["foreign"]:
        raise RuntimeError(f"refused, unpinned or foreign host access: {end}")
    body = ("\n".join(lines[1:]) + "\n").encode()
    for module in sorted(HERE.glob("*.py")):
        read(module)
    for module in sorted(wt.HERE.glob("*.py")):
        read(module)
    read(ROOT / "oracle/startup-loaders/generate.py")
    read(ROOT / "oracle/login/generate.py")
    records = [json.loads(l) for l in lines[1:]]
    provenance = dict(
        format=1, schema=SCHEMA, upstream=lock, scope=SCOPE,
        sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm="sha256, CR removed",
        sourceTree=dict(root="reference/server/Server/src/main", files=count, manifestSha256=tree_digest),
        artifacts={name: sha(path.read_bytes()) for name, path in sorted(jars.items())},
        compilerArtifacts={path.name: sha(path.read_bytes()) for path in sorted(compiler)},
        kotlinCompiler="1.8.20", javaVersion=version, jvmFlags=wt.JVM_FLAGS, redirectedCallSites=sites, config=config,
        bootRecord=dict(sha256=digest, identicalToWorldTrace=True, worldTraceFixtureSha256=sha(WORLD_TRACE.read_bytes())),
        clockStart=header["clockStart"], threadLocalSeed=header["threadLocalSeed"], name=NAME,
        saves=[dict(name=name, sha256=sha(data)) for name, data in files],
        counts=dict(records=len(records), fresh=sum(r["t"] == "fresh" for r in records), loads=sum(r["t"] == "load" for r in records)),
        determinism=dict(freshJvmRuns=2, identical=True),
        fixtureSha256=sha(body),
    )
    if check:
        same = (FIXTURE / "oracle.jsonl").read_bytes() == body
        if not same or json.loads((FIXTURE / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Canonical-player fixture/provenance differ; inspect before regeneration")
        print(f"Original fresh and loaded Players reproduced ({provenance['counts']['fresh']} fresh, "
              f"{provenance['counts']['loads']} loaded); boot record identical to the world-trace fixture; two fresh runs identical.")
    else:
        FIXTURE.mkdir(parents=True, exist_ok=True)
        (FIXTURE / "oracle.jsonl").write_bytes(body)
        (FIXTURE / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated original fresh and loaded Players ({provenance['counts']['fresh']} fresh, "
              f"{provenance['counts']['loads']} loaded); boot record identical to the world-trace fixture; two fresh runs identical.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
