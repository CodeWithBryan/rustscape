"""Record which original call queued every packet of the world-trace login scenarios.

Builds on oracle/world-trace as oracle/startup-loaders and oracle/canonical-player do: the same unchanged compiled
server (3,487 files), the same pinned call sites, the same world directory and scenario script. The harness
oracle.worldtrace.LoginOrigins runs each login scenario statement for statement as WorldTrace.main does; after the
boot record it replaces the two lists of the original's PacketWriteQueue with lists that remember the caller's stack
when a packet is queued. Every entry a session is sent is then attributed to a stack (or to "direct" / "pump").

A run is accepted only if (a) two fresh JVM passes agree byte for byte and (b) every record it writes for
new-character, two-characters and returning-character is byte-identical to the same record of
fixtures/world-trace/trace.jsonl (the boot records by digest). The world-trace generator, harness and fixture are
unchanged. Java/Kotlin are the test oracle only. --check never rewrites the fixture. See docs/login.md.
"""
import argparse
import concurrent.futures
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / "runs/world-login-oracle"
FIXTURE = ROOT / "fixtures/world-login"
WORLD_TRACE = ROOT / "fixtures/world-trace/trace.jsonl"
spec = importlib.util.spec_from_file_location("worldtrace", ROOT / "oracle/world-trace/generate.py")
wt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wt)
read, run, c = wt.read, wt.run, wt.c

SCHEMA = 1
SCENARIOS = ["new-character", "two-characters", "returning-character"]
STUDIED = [
    "core/net/packet/PacketWriteQueue.kt",
    "core/net/packet/PacketRepository.java",
    "core/net/IoSession.java",
    "core/net/event/LoginReadEvent.kt",
    "core/net/packet/in/Login.kt",
    "core/game/node/entity/player/info/login/LoginParser.kt",
    "core/game/node/entity/player/info/login/PlayerParser.java",
    "core/game/node/entity/player/info/login/LoginConfiguration.java",
    "core/game/node/entity/player/Player.java",
    "core/game/node/entity/Entity.java",
]
SCOPE = (
    "The whole original server compiled unchanged (as oracle/world-trace), booted and driven by the world-trace "
    "script of new-character, two-characters and returning-character, statement for statement as WorldTrace.main. "
    "After the boot record PacketWriteQueue's two LinkedLists are replaced by LinkedLists that remember the stack of "
    "the caller of each add and attribute the session entries written during a flush to the packet polled before "
    "them. Every record must be byte-identical to the world-trace fixture's. Recorded: for each record and session, "
    "one origin per entry (a stack index, 'direct' for entries the original writes without the queue, 'pump' for "
    "entries written while hand-offs are pumped), and the table of distinct stacks (class.method:line from the "
    "caller of PacketRepository.send or PacketWriteQueue.push down to the harness). Not covered: the stack of direct "
    "writes, other JDKs and operating systems."
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


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


def play(passname, scenario, classpath, template, seed_files):
    """One scenario in one fresh JVM and world directory: (trace records, origin lines)."""
    folder = RUN / passname / scenario["name"]
    if folder.exists():
        shutil.rmtree(folder)
    world = folder / "world"
    shutil.copytree(template, world)
    (folder / "tmp").mkdir()
    for relative, data in seed_files.items():
        target = world / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    script = folder / "scenario.json"
    script.write_text(json.dumps(scenario), encoding="utf-8", newline="\n")
    trace, origins = folder / "trace.jsonl", folder / "origins.jsonl"
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    command = [exe, *wt.JVM_FLAGS, f"-Djava.io.tmpdir={folder / 'tmp'}", "-cp", classpath, "oracle.worldtrace.LoginOrigins", script, trace, origins]
    with (folder / "stdout.log").open("wb") as out, (folder / "stderr.log").open("wb") as err:
        p = subprocess.run(list(map(str, command)), cwd=world, stdin=subprocess.DEVNULL, stdout=out, stderr=err, timeout=1800)
    if p.returncode or not trace.is_file() or not origins.is_file():
        tail = (folder / "stderr.log").read_bytes()[-4000:].decode(errors="replace")
        raise RuntimeError(f"scenario {scenario['name']} failed ({p.returncode}); see {folder}\n{tail}")
    records = [json.loads(line) for line in trace.read_text(encoding="utf-8").splitlines() if line]
    return records, [json.loads(line) for line in origins.read_text(encoding="utf-8").splitlines() if line]


def canonical(name, records, plan):
    """The scenario's records as wt.one_pass writes them into the fixture (the boot record by digest)."""
    s = next(s for s in plan if s["name"] == name)
    lines = []
    for record in records:
        if record["t"] == "scenario":
            record.update(description=s["description"], filesFrom=s.get("filesFrom"), steps=len(s["steps"]))
        elif record["t"] == "boot":
            digest = sha(json.dumps({k: v for k, v in record.items() if k != "i"}, sort_keys=True, separators=(",", ":")).encode())
            record = dict(t="boot", i=record["i"], sameAsFirstScenario=True, sha256=digest)
        lines.append(json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=True))
    return lines


def fixture_scenarios():
    out, current = {}, None
    for line in WORLD_TRACE.read_bytes().decode().splitlines():
        record = json.loads(line)
        if record["t"] == "scenario":
            current = record["name"]
            out[current] = []
        out[current].append(line)
    return out


def one_pass(passname, classpath, template, plan):
    by_name = {s["name"]: s for s in plan}
    results = {}
    independent = [by_name[n] for n in SCENARIOS if not by_name[n].get("filesFrom")]
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        for s, result in zip(independent, pool.map(lambda s: play(passname, s, classpath, template, {}), independent)):
            results[s["name"]] = result
    for name in SCENARIOS:
        s = by_name[name]
        if s.get("filesFrom"):
            seed = {f["path"]: bytes.fromhex(f["hex"]) for record in results[s["filesFrom"]][0] for f in record.get("files", []) if "hex" in f}
            if not seed:
                raise RuntimeError(f"{s['filesFrom']} wrote no character save for {name} to start from")
            results[name] = play(passname, s, classpath, template, seed)
    fixture = fixture_scenarios()
    lines = []
    for name in SCENARIOS:
        records, origins = results[name]
        mine = canonical(name, records, plan)
        theirs = fixture[name]
        if mine != theirs:
            where = next((i for i, (x, y) in enumerate(zip(mine, theirs)) if x != y), min(len(mine), len(theirs)))
            raise RuntimeError(f"{name}: record {where} differs from the world-trace fixture; see {RUN / passname / name}")
        table = origins[-1]
        stacks = table["stacks"]
        lines.append(json.dumps(dict(t="scenario", name=name, captured=table["captured"]), sort_keys=True, separators=(",", ":")))
        for row in origins[:-1]:
            # Stack indices are made global: each is replaced by its frames' position in a scenario-independent table.
            lines.append(json.dumps(dict(t="record", i=row["i"], origins={k: [o if isinstance(o, str) else stacks[o] for o in v] for k, v in row["origins"].items()}),
                                    sort_keys=True, separators=(",", ":")))
    # Distinct stacks across all scenarios, in first-use order; records refer to them by index.
    table, ids, body = [], {}, []
    for line in lines:
        record = json.loads(line)
        if record["t"] == "record":
            for k, v in record["origins"].items():
                refs = []
                for o in v:
                    if isinstance(o, str):
                        refs.append(o)
                        continue
                    key = "\n".join(o)
                    if key not in ids:
                        ids[key] = len(table)
                        table.append(o)
                    refs.append(ids[key])
                record["origins"][k] = refs
        body.append(json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n")
    body.append(json.dumps(dict(t="stacks", stacks=table), sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n")
    return "".join(body).encode()


def generate(check):
    lock = json.loads(read(ROOT / "upstream.lock.json"))
    for part in ["server", "client"]:
        path = ROOT / "reference" / part
        if run(["git", "-C", path, "rev-parse", "HEAD"]).stdout.decode().strip() != lock[part]["commit"]:
            raise RuntimeError("Reference pin differs")
        run(["git", "-C", path, "diff", "--quiet", "HEAD", "--"])
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
    plan = wt.scenarios.build(read(wt.SERVER / "core/ServerConstants.kt"), read(ROOT / "reference/client/client/src/main/java/rt4/GlobalConfig.java"))
    first = one_pass("pass-1", classpath, template, plan)
    second = one_pass("pass-2", classpath, template, plan)
    if first != second:
        a, b = first.decode().splitlines(), second.decode().splitlines()
        where = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
        raise RuntimeError(f"two fresh passes differ at line {where + 1}; see {RUN / 'pass-1'} and {RUN / 'pass-2'}")
    for module in sorted(HERE.glob("*.py")):
        read(module)
    for module in sorted(wt.HERE.glob("*.py")):
        read(module)
    read(ROOT / "oracle/login/generate.py")
    records = [json.loads(l) for l in first.decode().splitlines()]
    entries = sum(len(v) for r in records if r["t"] == "record" for v in r["origins"].values())
    provenance = dict(
        format=1, schema=SCHEMA, upstream=lock, scope=SCOPE,
        sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm="sha256, CR removed",
        sourceTree=dict(root="reference/server/Server/src/main", files=count, manifestSha256=tree_digest),
        artifacts={name: sha(path.read_bytes()) for name, path in sorted(jars.items())},
        compilerArtifacts={path.name: sha(path.read_bytes()) for path in sorted(compiler)},
        kotlinCompiler="1.8.20", javaVersion=version, jvmFlags=wt.JVM_FLAGS, redirectedCallSites=sites, config=config,
        worldTrace=dict(fixtureSha256=sha(WORLD_TRACE.read_bytes()), scenarios=SCENARIOS, recordsIdentical=True),
        counts=dict(entries=entries, stacks=len(records[-1]["stacks"]),
                    direct=sum(o == "direct" for r in records if r["t"] == "record" for v in r["origins"].values() for o in v),
                    pump=sum(o == "pump" for r in records if r["t"] == "record" for v in r["origins"].values() for o in v)),
        determinism=dict(freshJvmRuns=2, identical=True),
        fixtureSha256=sha(first),
    )
    if check:
        same = (FIXTURE / "origins.jsonl").read_bytes() == first
        if not same or json.loads((FIXTURE / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("World-login fixture/provenance differ; inspect before regeneration")
        print(f"Packet origins of {len(SCENARIOS)} world-trace login scenarios reproduced ({entries} entries, "
              f"{provenance['counts']['stacks']} distinct stacks); records identical to the world-trace fixture; two fresh passes identical.")
    else:
        FIXTURE.mkdir(parents=True, exist_ok=True)
        (FIXTURE / "origins.jsonl").write_bytes(first)
        (FIXTURE / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated packet origins of {len(SCENARIOS)} world-trace login scenarios ({entries} entries, "
              f"{provenance['counts']['stacks']} distinct stacks); records identical to the world-trace fixture; two fresh passes identical.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
