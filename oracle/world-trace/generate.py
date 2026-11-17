"""Boot the whole unmodified original server headless and record scripted world traces.

Java/Kotlin are the test oracle only. --check never rewrites the fixture.
Everything this builds, extracts or downloads lives under runs/world-trace-oracle.
See docs/world-trace.md.
"""
import argparse
import concurrent.futures
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
RUN = ROOT / "runs/world-trace-oracle"
REFERENCE = ROOT / "reference/server"
SERVER = REFERENCE / "Server/src/main"
FIXTURE = ROOT / "fixtures/world-trace"
spec = importlib.util.spec_from_file_location("compiler", ROOT / "oracle/login/generate.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
read, run = c.read, c.run
spec = importlib.util.spec_from_file_location("scenarios", HERE / "scenarios.py")
scenarios = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scenarios)

SCHEMA = 1
# Read for provenance: the start-up, tick, login and logout path the harness drives or stands in for.
STUDIED = [
    "core/Server.kt",
    "core/ServerConstants.kt",
    "core/game/world/GameWorld.kt",
    "core/worker/MajorUpdateWorker.kt",
    "core/worker/ManagementEvents.kt",
    "core/plugin/ClassScanner.kt",
    "core/game/system/SystemManager.java",
    "core/game/system/config/ConfigParser.kt",
    "core/game/system/config/ServerConfigParser.kt",
    "core/game/system/task/TaskExecutor.kt",
    "core/auth/Auth.kt",
    "core/auth/DevelopmentAuthenticator.kt",
    "core/storage/InMemoryStorageProvider.kt",
    "core/net/IoSession.java",
    "core/net/IoEventHandler.java",
    "core/net/IoReadEvent.java",
    "core/net/event/HSReadEvent.java",
    "core/net/event/HSWriteEvent.java",
    "core/net/event/LoginReadEvent.kt",
    "core/net/event/LoginWriteEvent.java",
    "core/net/event/GameReadEvent.java",
    "core/net/event/GameWriteEvent.java",
    "core/net/packet/PacketWriteQueue.kt",
    "core/net/packet/in/Login.kt",
    "core/game/node/entity/player/info/login/LoginParser.kt",
    "core/game/node/entity/player/info/login/PlayerParser.java",
    "core/game/node/entity/player/info/login/LoginConfiguration.java",
    "core/game/node/entity/player/info/login/PlayerSaver.kt",
    "core/game/world/repository/DisconnectionQueue.kt",
    "core/game/world/repository/Repository.kt",
    "core/tools/RandomFunction.java",
]
# default.conf is used as shipped except for these keys; each must occur exactly once.
CONFIG_OVERRIDES = {"enable_bots": ("true", "false")}
# hashCode=2 gives every object the same identity hash. The original's world depends on identity hashes (maps and sets
# keyed by objects without hashCode iterate in hash order), so with the JVM's default generator the trace depended on how
# many hashes the world thread had handed out before start-up, including to the harness. See docs/world-trace.md.
JVM_FLAGS = ["-Xmx4g", "-Xss4m", "-XX:+UseSerialGC", "-Xshare:off", "-XX:+UnlockExperimentalVMOptions", "-XX:hashCode=2", "-Djava.awt.headless=true",
             "-Dfile.encoding=UTF-8", "-Duser.language=en", "-Duser.country=US", "-Duser.variant=", "-Duser.timezone=UTC"]
KOTLINC_FLAGS = ["-no-stdlib", "-no-reflect", "-nowarn", "-jvm-target", "11"]
JAVAC_FLAGS = ["--release", "11", "-nowarn", "-encoding", "UTF-8", "-proc:none"]
SCOPE = (
    "The whole original server (every Kotlin and Java file under Server/src/main, compiled unchanged with Kotlin "
    "1.8.20 and javac on JDK 11 against the dependencies its POM names) booted in one JVM by the original "
    "ServerConfigParser.parse and GameWorld.prompt(false), then driven by a script: bytes handed to the session's own "
    "event producers, MajorUpdateWorker.handleTickActions(false) called once per tick, IoSession.disconnect for a "
    "closed connection. Recorded per step: bytes each session was sent, repository lists, tick counter, generator "
    "positions, files written. Not original: the socket (three IoSession members), the 600 ms worker loop and its "
    "ping-timeout, inactive-player and midnight-restart checks, Server.main's networking, console reader and "
    "watchdog, and the SystemManager.flag call that starts the worker. Pinned inputs: wall clock and unseeded "
    "java.util.Random constructions (call sites redirected after compilation; census in this file), "
    "RandomFunction.RANDOM, Math.random, Collections.shuffle and the world thread's ThreadLocalRandom (replaced or "
    "seeded), Dispatchers.Default and the management-event queue (run on the world thread when the harness pumps), "
    "identity hash codes (one constant for every object, -XX:hashCode=2, so hash collections keyed by identity "
    "iterate in an order fixed by their insertion history), JVM flags, locale and time zone. The configuration is "
    "worldprops/default.conf with enable_bots=false. Class-scan order, directory listing order and anything else "
    "that depends on the JDK build or operating system are not controlled individually: they are observed to "
    "repeat (two fresh JVM runs must agree byte for byte before a fixture is written or accepted) on this JDK "
    "build and operating system only. No native world replays this trace yet; it establishes nothing about the "
    "Rust server."
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git(*args):
    """Read-only git on the pinned server checkout. It is a partial clone, and a blob it lacks is never
    fetched unless RS09_WORLD_TRACE_FETCH_DATA=1 asks for git's own fetch of that pinned object."""
    if os.environ.get("RS09_WORLD_TRACE_FETCH_DATA") == "1":
        command, env = ["git", "-C", str(REFERENCE), *args], dict(os.environ)
    else:
        command, env = ["git", "-C", str(REFERENCE), "--no-lazy-fetch", *args], dict(os.environ, GIT_NO_LAZY_FETCH="1")
    result = subprocess.run(command, capture_output=True, env=env)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace") + "\nIf the pinned Server/data objects are absent from "
                           "reference/server (a fresh partial clone), set RS09_WORLD_TRACE_FETCH_DATA=1 to let git fetch them.")
    return result.stdout


def artifacts():
    """Every jar the original's POM needs, each verified against artifacts.lock.json."""
    lock = json.loads(read(HERE / "artifacts.lock.json"))
    folder = RUN / "jars"
    folder.mkdir(parents=True, exist_ok=True)
    out = {}
    for name, entry in sorted(lock.items()):
        target = folder / name
        def valid(path):
            return path.is_file() and path.stat().st_size == entry["size"] and sha(path.read_bytes()) == entry["sha256"]
        if not valid(target):
            for other in sorted((ROOT / "runs").glob("*/" + name)):
                if valid(other):
                    shutil.copyfile(other, target)
                    break
            else:
                with urllib.request.urlopen(entry["url"], timeout=120) as response:
                    data = response.read(entry["size"] + 1)
                if len(data) != entry["size"] or sha(data) != entry["sha256"]:
                    raise RuntimeError(f"{name} differs from its pin")
                target.write_bytes(data)
        out[name] = target
    for stray in folder.iterdir():
        if stray.name not in out:
            raise RuntimeError(f"unpinned file in {folder}: {stray.name}")
    return out


def source_tree():
    """Hashes every original source file; returns (kotlin+java paths, manifest digest, count)."""
    paths = sorted(p for p in SERVER.rglob("*") if p.is_file() and p.suffix in (".kt", ".java"))
    manifest = hashlib.sha256()
    for path in paths:
        data = path.read_bytes().replace(b"\r", b"")
        manifest.update(f"{path.relative_to(SERVER).as_posix()} {sha(data)}\n".encode())
    return paths, manifest.hexdigest(), len(paths)


def compile_server(jars, compiler, tree_digest, paths):
    """Compiles the unmodified sources once per distinct input set (about three minutes)."""
    classes = RUN / "classes"
    stdlib = next(p for p in compiler if p.name.startswith("kotlin-stdlib-"))
    notes = next(p for p in compiler if p.name.startswith("annotations-"))
    library = [stdlib, notes, *[jars[name] for name in sorted(jars)]]
    version = run(["java", "-version"]).stderr.decode().strip().splitlines()[0]
    key = sha(json.dumps([tree_digest, version, KOTLINC_FLAGS, JAVAC_FLAGS, [[p.name, sha(p.read_bytes())] for p in [*compiler, *library]]]).encode())
    stamp = RUN / "classes.stamp"
    if not (classes.is_dir() and stamp.is_file() and stamp.read_text() == key):
        if stamp.exists():
            stamp.unlink()
        if classes.exists():
            shutil.rmtree(classes)
        classes.mkdir(parents=True)
        cp = os.pathsep.join(map(str, library))
        run(["java", "-Xmx10g", "-Xss16m", "-cp", os.pathsep.join(map(str, compiler)), "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",
             *KOTLINC_FLAGS, "-classpath", cp, "-d", classes, SERVER])
        listing = RUN / "java-sources.txt"
        listing.write_text("".join('"' + p.as_posix() + '"\n' for p in paths if p.suffix == ".java"), encoding="utf-8", newline="\n")
        run(["javac", "-J-Xmx6g", *JAVAC_FLAGS, "-cp", os.pathsep.join(map(str, [classes, *library])), "-d", classes, "@" + str(listing)])
        stamp.write_text(key)
    return classes, library, version


def build_harness(classes, library, compiler):
    """Redirects the pinned call sites into a copy of the class files and compiles the adapters."""
    tool, pinned, harness = RUN / "tool", RUN / "pinned", RUN / "harness"
    for folder in [tool, pinned, harness]:
        if folder.exists():
            shutil.rmtree(folder)
        folder.mkdir(parents=True)
    embeddable = next(p for p in compiler if p.name.startswith("kotlin-compiler-embeddable-"))
    source = HERE / "tool/Rewrite.java"
    read(source)
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", embeddable, "-d", tool, source])
    census = RUN / "census.json"
    run(["java", "-cp", os.pathsep.join(map(str, [tool, embeddable])), "Rewrite", classes, pinned, census])
    sources = sorted((HERE / "harness").rglob("*.java"))
    for path in sources:
        read(path)
    run(["javac", "--release", "11", "-nowarn", "-encoding", "UTF-8", "-cp", os.pathsep.join(map(str, [classes, *library])), "-d", harness, *sources])
    return os.pathsep.join(map(str, [pinned, harness, *library])), json.loads(census.read_text(encoding="utf-8"))


def world_template(lock):
    """The world directory a scenario starts from: pinned data and the derived configuration."""
    template = RUN / "template"
    if template.exists():
        shutil.rmtree(template)
    data = {}
    for line in git("ls-tree", "-r", lock["server"]["commit"], "Server/data").decode().splitlines():
        meta, name = line.split("\t")
        blob = git("cat-file", "blob", meta.split()[2])
        relative = name[len("Server/"):]
        target = template / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if relative.startswith("data/cache/"):
            # The repository stores the cache as LFS pointers; the bytes come from assets/cache and must match them.
            pointer = dict(row.split(" ", 1) for row in blob.decode().splitlines() if row)
            asset = ROOT / "assets/cache" / target.name
            blob = asset.read_bytes() if asset.is_file() else b""
            if len(blob) != int(pointer["size"]) or "sha256:" + sha(blob) != pointer["oid"]:
                raise RuntimeError(f"assets/cache/{target.name} is missing or differs from the pinned LFS pointer; run cargo xtask cache-fetch")
        target.write_bytes(blob)
        data[relative] = sha(blob)
    shipped = git("cat-file", "blob", lock["server"]["commit"] + ":Server/worldprops/default.conf")
    text = shipped.decode()
    for key, (old, new) in CONFIG_OVERRIDES.items():
        text, count = re.subn(rf"(?m)^{key} = {old}\b", f"{key} = {new}", text)
        if count != 1:
            raise RuntimeError(f"default.conf: expected exactly one '{key} = {old}'")
    conf = template / "worldprops/default.conf"
    conf.parent.mkdir(parents=True)
    conf.write_bytes(text.encode())
    config = dict(shippedSha256=sha(shipped), usedSha256=sha(text.encode()),
                  overrides={key: dict(shipped=old, used=new) for key, (old, new) in CONFIG_OVERRIDES.items()})
    return template, data, config


def play(passname, scenario, classpath, template, seed_files):
    """One scenario in one fresh JVM and one fresh world directory. Returns its canonical records."""
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
    trace = folder / "trace.jsonl"
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    command = [exe, *JVM_FLAGS, f"-Djava.io.tmpdir={folder / 'tmp'}", "-cp", classpath, "oracle.worldtrace.WorldTrace", script, trace]
    with (folder / "stdout.log").open("wb") as out, (folder / "stderr.log").open("wb") as err:
        p = subprocess.run(list(map(str, command)), cwd=world, stdin=subprocess.DEVNULL, stdout=out, stderr=err, timeout=1800)
    if p.returncode or not trace.is_file():
        tail = (folder / "stderr.log").read_bytes()[-4000:].decode(errors="replace")
        raise RuntimeError(f"scenario {scenario['name']} failed ({p.returncode}); see {folder}\n{tail}")
    records = [json.loads(line) for line in trace.read_text(encoding="utf-8").splitlines() if line]
    for record in records:
        if record.get("refused") or record.get("unpinned"):
            raise RuntimeError(f"scenario {scenario['name']}: refused or unpinned host access: {record.get('refused')} {record.get('unpinned')}")
        if record.get("handoffs", {}).get("foreign"):
            raise RuntimeError(f"scenario {scenario['name']}: work arrived from an unpinned thread: {record['handoffs']['foreign']}")
    return records


def one_pass(passname, classpath, template):
    """All scenarios once. A scenario may start from files an earlier one of the same pass wrote."""
    plan = scenarios.build(read(SERVER / "core/ServerConstants.kt"), read(ROOT / "reference/client/client/src/main/java/rt4/GlobalConfig.java"))
    independent = [s for s in plan if not s.get("filesFrom")]
    out = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        for s, records in zip(independent, pool.map(lambda s: play(passname, s, classpath, template, {}), independent)):
            out[s["name"]] = records
    for s in plan:
        if s.get("filesFrom"):
            seed = {f["path"]: bytes.fromhex(f["hex"]) for record in out[s["filesFrom"]] for f in record.get("files", []) if "hex" in f}
            if not seed:
                raise RuntimeError(f"{s['filesFrom']} wrote no character save for {s['name']} to start from")
            out[s["name"]] = play(passname, s, classpath, template, seed)
    lines = []
    boot = None
    for s in plan:
        for record in out[s["name"]]:
            if record["t"] == "scenario":
                record.update(description=s["description"], filesFrom=s.get("filesFrom"), steps=len(s["steps"]))
            elif record["t"] == "boot":
                # Every scenario boots the same world; the full record is kept once and the others point at it.
                digest = sha(json.dumps({k: v for k, v in record.items() if k != "i"}, sort_keys=True, separators=(",", ":")).encode())
                if boot is None:
                    boot = digest
                elif digest == boot:
                    record = dict(t="boot", i=record["i"], sameAsFirstScenario=True, sha256=digest)
                record["sha256"] = digest
            lines.append(json.dumps(record, sort_keys=True, separators=(",", ":"), ensure_ascii=True) + "\n")
    return "".join(lines).encode(), plan


def census(data):
    out = []
    current = None
    for line in data.decode().splitlines():
        record = json.loads(line)
        if record["t"] == "scenario":
            current = dict(name=record["name"], records=0, ticks=0, sessions=0, packets=0, loginReplies=0, bytesOut=0,
                           droppedAfterClose=0, savesWritten=0)
            out.append(current)
            continue
        current["records"] += 1
        current["ticks"] += record["t"] == "tick"
        current["sessions"] += record.get("op") == "connect"
        for entries in record.get("out", {}).values():
            for entry in entries:
                if entry.get("dropped"):
                    current["droppedAfterClose"] += 1
                    continue
                current["packets"] += entry["k"] == "packet"
                current["loginReplies"] += entry["k"] == "login"
                current["bytesOut"] += len(entry["hex"]) // 2
        current["savesWritten"] += sum("hex" in f for f in record.get("files", []))
    return out


def generate(check):
    lock = json.loads(read(ROOT / "upstream.lock.json"))
    for part in ["server", "client"]:
        path = ROOT / "reference" / part
        if run(["git", "-C", path, "rev-parse", "HEAD"]).stdout.decode().strip() != lock[part]["commit"]:
            raise RuntimeError("Reference pin differs")
        run(["git", "-C", path, "diff", "--quiet", "HEAD", "--"])
    RUN.mkdir(parents=True, exist_ok=True)
    compiler = c.artifacts()
    jars = artifacts()
    paths, tree_digest, count = source_tree()
    for name in STUDIED:
        read(SERVER / name)
    classes, library, version = compile_server(jars, compiler, tree_digest, paths)
    classpath, sites = build_harness(classes, library, compiler)
    template, data_files, config = world_template(lock)
    first, plan = one_pass("pass-1", classpath, template)
    second, _ = one_pass("pass-2", classpath, template)
    if first != second:
        a, b = first.decode().splitlines(), second.decode().splitlines()
        where = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
        detail = ""
        if where < min(len(a), len(b)):
            x, y = json.loads(a[where]), json.loads(b[where])
            detail = f" ({x.get('t')} {x.get('i')}: keys {sorted(k for k in set(x) | set(y) if x.get(k) != y.get(k))})"
        raise RuntimeError(f"two fresh runs differ at fixture line {where + 1}{detail}; see {RUN / 'pass-1'} and {RUN / 'pass-2'}")
    for module in sorted(HERE.glob("*.py")):
        read(module)
    read(ROOT / "oracle/login/generate.py")
    provenance = dict(
        format=1, schema=SCHEMA, upstream=lock, scope=SCOPE,
        sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm="sha256, CR removed",
        sourceTree=dict(root="reference/server/Server/src/main", files=count, kotlin=sum(p.suffix == ".kt" for p in paths),
                        java=sum(p.suffix == ".java" for p in paths), manifestSha256=tree_digest,
                        manifest="sha256 over 'relative/path sha256\\n' for every .kt and .java file in path order, CR removed",
                        gitTree=git("rev-parse", lock["server"]["commit"] + ":Server/src/main").decode().strip()),
        artifacts={name: sha(path.read_bytes()) for name, path in sorted(jars.items())},
        compilerArtifacts={path.name: sha(path.read_bytes()) for path in sorted(compiler)},
        kotlinCompiler="1.8.20", kotlincFlags=KOTLINC_FLAGS, javacFlags=JAVAC_FLAGS, javaVersion=version, jvmFlags=JVM_FLAGS,
        redirectedCallSites=sites, config=config, data=dict(sorted(data_files.items())),
        scenarios=census(first), determinism=dict(freshJvmRuns=2, identical=True),
        fixtureSha256=sha(first),
    )
    if check:
        if (FIXTURE / "trace.jsonl").read_bytes() != first or json.loads((FIXTURE / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("World-trace fixture/provenance differ; inspect before regeneration")
        print(f"{len(plan)} original world-trace scenarios reproduced ({len(first.splitlines())} records, two fresh runs identical).")
    else:
        FIXTURE.mkdir(parents=True, exist_ok=True)
        (FIXTURE / "trace.jsonl").write_bytes(first)
        (FIXTURE / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated {len(plan)} original world-trace scenarios ({len(first.splitlines())} records, two fresh runs identical).")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
