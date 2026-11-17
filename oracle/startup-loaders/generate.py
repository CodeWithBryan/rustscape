"""Run the original start-up one call at a time and record what the small configuration loaders leave behind.

Builds on oracle/world-trace: the same unchanged compiled server, the same pinned call sites, the same world
directory and the same boot inputs. oracle.worldtrace.StartupPhases calls the statements of GameWorld.prompt(false)
in their order instead of calling prompt, records generator positions after each, dumps the tables of XteaParser,
ObjectConfigParser, ObjectOverrideParser, MusicConfigLoader, RangedConfigLoader and ClueRewardParser after their
phase, writes the boot record as WorldTrace does, and then runs selected loaders over authored inputs.

A run is accepted only if (a) two fresh JVM runs agree byte for byte and (b) its boot record is byte-identical to
the boot record of fixtures/world-trace/trace.jsonl, so the decomposition did not change the start-up.
Java/Kotlin are the test oracle only. --check never rewrites the fixture. See docs/startup.md.
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
RUN = ROOT / "runs/startup-loaders-oracle"
FIXTURE = ROOT / "fixtures/startup-loaders"
WORLD_TRACE = ROOT / "fixtures/world-trace/trace.jsonl"
spec = importlib.util.spec_from_file_location("worldtrace", ROOT / "oracle/world-trace/generate.py")
wt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wt)
read, run, c = wt.read, wt.run, wt.c

SCHEMA = 1
# The original sources this corpus is about; read for provenance.
STUDIED = [
    "core/game/world/GameWorld.kt",
    "core/game/system/config/ConfigParser.kt",
    "core/plugin/ClassScanner.kt",
    "core/ServerStore.kt",
    "core/game/system/config/XteaParser.kt",
    "core/game/system/config/ObjectConfigParser.kt",
    "core/game/system/config/ObjectOverrideParser.kt",
    "core/game/system/config/ShopParser.kt",
    "core/game/system/config/MusicConfigLoader.kt",
    "core/game/system/config/RangedConfigLoader.kt",
    "core/game/system/config/ClueRewardParser.kt",
    "core/api/utils/WeightBasedTable.kt",
    "core/api/utils/WeightedItem.kt",
    "core/cache/def/impl/DataMap.java",
    "core/cache/def/impl/SceneryDefinition.java",
    "core/game/node/scenery/Scenery.java",
    "core/game/world/map/Direction.java",
    "core/game/world/map/Location.kt",
    "core/game/world/map/RegionManager.kt",
    "core/game/world/map/zone/ZoneBorders.java",
    "core/game/node/entity/player/link/music/MusicEntry.java",
    "core/game/node/entity/player/link/music/MusicZone.java",
    "core/game/node/entity/combat/equipment/Ammunition.java",
    "core/game/node/entity/combat/equipment/RangeWeapon.java",
    "core/game/node/entity/combat/equipment/BoltEffect.java",
    "core/game/node/entity/impl/Projectile.java",
    "core/game/world/update/flag/context/Graphics.java",
    "core/tools/RandomFunction.java",
]
# Pinned inputs of the loaders this corpus checks; copied into the fixture for the native tests.
PINNED = ["xteas.json", "object_configs.json", "object_overrides.json", "shops.json", "music_regions.json",
          "music_tiles.json", "ammo_configs.json", "ranged_weapon_configs.json", "clue_rewards.json"]
SCOPE = (
    "The whole original server compiled unchanged (as oracle/world-trace), started by calling the statements of "
    "GameWorld.prompt(false) one at a time in their order (ConfigParser.prePlugin/postPlugin loader by loader, "
    "start-up listeners one by one), with the world-trace boot inputs. Recorded: generator positions and the "
    "original's log lines after every call; the tables XteaParser, ObjectConfigParser, ObjectOverrideParser, "
    "MusicConfigLoader, RangedConfigLoader and ClueRewardParser leave behind, read after their own call; then each "
    "of those loaders and ShopParser again over authored inputs (CONFIG_PATH pointed at a case directory), with "
    "the exception that left it and the draws it made. Accepted only when the resulting boot record is "
    "byte-identical to the world-trace fixture's. Read-only dumps (no definition, region or DataMap lookups that "
    "create entries). Not covered: other JDKs/operating systems, the loaders' log text beyond what is printed, "
    "and every other start-up step's own tables."
)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def text(value):
    return value if isinstance(value, str) else json.dumps(value)


# Authored inputs: each case is one loader run over the files given (missing files are absent on purpose).
def cases():
    out = []

    def case(name, loader, files, **extra):
        out.append(dict(name=name, loader=loader, files={k: text(v) for k, v in files.items()}, **extra))

    xteas = lambda rows: {"xteas.json": {"xteas": rows}}
    case("xtea-valid", "XteaParser", xteas([
        {"regionId": "12850", "keys": "1,-2,3,-2147483648"},
        {"regionId": 12851, "keys": "0,0,0,0"},
        {"regionId": "12850", "keys": "5,6,7,8"},
        {"regionId": "+7", "keys": "9,10,11,12,13"}]))
    case("xtea-short-keys", "XteaParser", xteas([{"regionId": "1", "keys": "1,2,3,4"}, {"regionId": "2", "keys": "1,2,3"},
                                                 {"regionId": "3", "keys": "1,2,3,4"}]))
    case("xtea-bad-region", "XteaParser", xteas([{"regionId": "4", "keys": "1,2,3,4"}, {"regionId": "x", "keys": "1,2,3,4"}]))
    case("xtea-spaced-key", "XteaParser", xteas([{"regionId": "5", "keys": "1, 2,3,4"}]))
    case("xtea-number-keys", "XteaParser", xteas([{"regionId": "6", "keys": 5}]))
    case("xtea-missing-region", "XteaParser", xteas([{"keys": "1,2,3,4"}]))
    case("xtea-missing-array", "XteaParser", {"xteas.json": {"other": []}})
    case("xtea-array-root", "XteaParser", {"xteas.json": []})
    case("xtea-row-not-object", "XteaParser", xteas([{"regionId": "8", "keys": "1,2,3,4"}, 3]))
    case("xtea-bad-json", "XteaParser", {"xteas.json": "{\"xteas\": [}"})
    case("xtea-no-file", "XteaParser", {})

    objects = lambda rows: {"object_configs.json": rows}
    case("object-valid", "ObjectConfigParser", objects([
        {"ids": "60001,60002", "examine": "A thing.", "render_anim": "123", "other": "x"},
        {"ids": "60003", "examine": "", "render_anim": "null"},
        {"ids": "60004", "examine": "null", "render_anim": None},
        {"ids": "60005", "examine": 12, "render_anim": 5},
        {"ids": "60001", "render_anim": "65535"}]), dump=[60001, 60002, 60003, 60004, 60005])
    case("object-spaced-ids", "ObjectConfigParser", objects([{"ids": "60006", "examine": "kept"}, {"ids": "60007, 60008", "examine": "never"}]),
         dump=[60006, 60007, 60008])
    case("object-bad-anim", "ObjectConfigParser", objects([{"ids": "60009,60010", "examine": "first", "render_anim": "x"}]),
         dump=[60009, 60010])
    case("object-missing-ids", "ObjectConfigParser", objects([{"examine": "none"}]), dump=[])
    case("object-trailing-comma", "ObjectConfigParser", objects([{"ids": "60011,", "examine": "t"}]), dump=[60011])
    case("object-object-root", "ObjectConfigParser", {"object_configs.json": {"ids": "60012"}}, dump=[60012])
    case("object-no-file", "ObjectConfigParser", {}, dump=[])

    overrides = lambda rows: {"object_overrides.json": rows}
    case("override-valid", "ObjectOverrideParser", overrides([
        {"mode": "add", "id": "1276", "location": "[3200,3200,0]"},
        {"mode": "add", "id": "1276", "location": "[3200,3200,0]"},
        {"mode": "add", "id": "1276", "location": "3200, 3201, 1", "type": "4", "direction": "ne"},
        {"mode": "remove", "id": 1277, "location": "[ 3300 ,3300,0 ]", "direction": "nw"},
        {"mode": "remove", "id": "1277", "location": "[3300,3300,0]", "direction": "nw"},
        {"mode": "remove", "id": "1277", "location": "[3300,3300,0]", "direction": "w"},
        {"mode": "add", "id": "0", "location": "[3100,3100,0]", "direction": "e"},
        {"mode": "add", "id": "-5", "location": "[3100,3101,0]", "direction": "sw", "type": "10"},
        {"mode": "add", "id": "1278", "location": "[3100,3102,0]", "direction": "se"},
        {"mode": "add", "id": "1278", "location": "[3100,3103,0]", "direction": "s"},
        {"mode": "add", "id": "1278", "location": "[3100,3104,0]", "direction": "q"},
        {"mode": "move", "id": "1278", "location": "[3100,3105,0]"},
        {"mode": "Add", "id": "1278", "location": "[3100,3106,0]"}]))
    case("override-missing-mode", "ObjectOverrideParser", overrides([{"id": "1", "location": "[3000,3000,0]"}]))
    case("override-missing-id", "ObjectOverrideParser", overrides([{"mode": "add", "location": "[3000,3000,0]"}, {"mode": "add", "id": "2", "location": "[3000,3001,0]"}]))
    case("override-short-location", "ObjectOverrideParser", overrides([{"mode": "add", "id": "3", "location": "[3000,3002,0]"}, {"mode": "add", "id": "3", "location": "[3000,3003]"}]))
    case("override-bad-type", "ObjectOverrideParser", overrides([{"mode": "add", "id": "4", "location": "[3000,3004,0]", "type": "x"}]))
    case("override-half-bracket", "ObjectOverrideParser", overrides([{"mode": "add", "id": "5", "location": "[3000,3005,0"}]))
    case("override-no-file", "ObjectOverrideParser", {})

    case("shops-three", "ShopParser", {"shops.json": [{}, {"a": 1}, 3]})
    case("shops-empty", "ShopParser", {"shops.json": []})
    case("shops-object-root", "ShopParser", {"shops.json": {"a": []}})
    case("shops-bad-json", "ShopParser", {"shops.json": "[1,"})
    case("shops-no-file", "ShopParser", {})

    music = lambda regions, tiles: {"music_regions.json": regions, "music_tiles.json": tiles}
    case("music-valid", "MusicConfigLoader", music(
        [{"region": "12850", "id": "76"}, {"region": 12851, "id": 5}, {"region": "12850", "id": "+77"}],
        [{"id": "1", "borders": "{3200,3200,3210,3210}"},
         {"id": "2", "borders": "{3270,3270,3200,3200}-{3100,3100,3101,3101}"},
         {"id": "3", "borders": "{3000,3000,3100,3100,[3001,3001,3002,3002]~[3003,3003,3004,3004]}"},
         {"id": "4", "borders": "{2990,2990,2995,2995,[2991,2991,2992,2992]}"}]))
    case("music-bad-region", "MusicConfigLoader", music([{"region": "1", "id": "2"}, {"region": "r", "id": "3"}], []))
    case("music-short-border", "MusicConfigLoader", music([], [{"id": "5", "borders": "{3200,3200,3210,3210}"}, {"id": "6", "borders": "{3200,3200,3210}"}]))
    case("music-empty-part", "MusicConfigLoader", music([], [{"id": "7", "borders": "{3200,3200,3210,3210}-"}]))
    case("music-spaced", "MusicConfigLoader", music([{"region": " 9", "id": "1"}], []))
    case("music-no-tiles", "MusicConfigLoader", {"music_regions.json": [{"region": "10", "id": "1"}]})

    ammo = lambda rows: rows
    weapons = lambda rows: rows
    ranged = lambda a, w: {"ammo_configs.json": a, "ranged_weapon_configs.json": w}
    good_ammo = {"itemId": "20000", "start_graphic": "19,96", "darkbow_graphic": "", "projectile": "10,40,36,41,5,15,11", "poison_damage": "0"}
    case("ranged-valid", "RangedConfigLoader", ranged(
        [good_ammo,
         {"itemId": "9236", "start_graphic": "28,96", "darkbow_graphic": "1104,96", "projectile": "27,38,36,41,32,5,11", "poison_damage": 3},
         {"itemId": "20000", "start_graphic": "1,2", "darkbow_graphic": " ", "projectile": "1,2,3,4,5,6,7,8", "poison_damage": "-1"}],
        [{"itemId": "20001", "animation": "426", "ammo_slot": "13", "weapon_type": "0", "drop_ammo": "true", "ammunition": "882,884"},
         {"itemId": 20001, "animation": "1", "ammo_slot": "3", "weapon_type": "1", "drop_ammo": "TRUE", "ammunition": "1"},
         {"itemId": "20002", "animation": "427", "ammo_slot": "3", "weapon_type": "2", "drop_ammo": "yes", "ammunition": "9236"}]))
    case("ranged-number-item", "RangedConfigLoader", ranged([dict(good_ammo, itemId=20003)], []))
    case("ranged-missing-darkbow", "RangedConfigLoader", ranged([{k: v for k, v in good_ammo.items() if k != "darkbow_graphic"}], []))
    case("ranged-short-projectile", "RangedConfigLoader", ranged([good_ammo, dict(good_ammo, itemId="20004", projectile="1,2,3,4,5,6")], []))
    case("ranged-bad-ammunition", "RangedConfigLoader", ranged([], [{"itemId": "20005", "animation": "1", "ammo_slot": "3", "weapon_type": "0", "drop_ammo": "false", "ammunition": "882,"}]))
    case("ranged-no-weapons", "RangedConfigLoader", {"ammo_configs.json": [good_ammo]})

    clue = lambda easy, medium, hard, rare: {"clue_rewards.json": {k: v for k, v in dict(easy=easy, medium=medium, hard=hard, rare=rare).items() if v is not None}}
    row = lambda i, lo, hi, w: {"id": i, "minAmount": lo, "maxAmount": hi, "weight": w}
    case("clue-valid", "ClueRewardParser", clue(
        [row(1, 1, 1, 1.0), row(2, 5, 1, "2.5"), row("3", "1", "3", 0), row(4, 1, 1, -1.5)],
        [row(5, 1, 1, 1)], [], [row(6, 1, 1, 1), row(7, 1, 1, "1e2"), row(8, 2, 2, "0.1")]))
    case("clue-missing-rare", "ClueRewardParser", clue([row(9, 1, 1, 1), row(10, 1, 1, 1)], [row(11, 1, 1, 1)], [row(12, 1, 1, 1)], None))
    case("clue-bad-weight", "ClueRewardParser", clue([row(13, 1, 1, 1), row(14, 1, 1, "x")], [], [], []))
    case("clue-bad-amount", "ClueRewardParser", clue([row(15, "1.0", 1, 1)], [], [], []))
    case("clue-no-file", "ClueRewardParser", {})
    return out


def write_cases(folder, plan):
    base = folder / "cases"
    for entry in plan:
        target = base / entry["name"]
        target.mkdir(parents=True)
        for name, data in entry["files"].items():
            (target / name).write_bytes(data.encode())
    path = folder / "cases.json"
    path.write_text(json.dumps([{k: v for k, v in e.items() if k != "files"} for e in plan]), encoding="utf-8", newline="\n")
    return path


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


def play(passname, classpath, template, plan, header):
    folder = RUN / passname
    if folder.exists():
        shutil.rmtree(folder)
    world = folder / "world"
    shutil.copytree(template, world)
    (folder / "tmp").mkdir()
    script = write_cases(folder, plan)
    out = folder / "phases.jsonl"
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    command = [exe, *wt.JVM_FLAGS, f"-Djava.io.tmpdir={folder / 'tmp'}", "-cp", classpath, "oracle.worldtrace.StartupPhases",
               script, out, str(header["clockStart"]), str(header["threadLocalSeed"])]
    with (folder / "stdout.log").open("wb") as o, (folder / "stderr.log").open("wb") as e:
        p = subprocess.run(list(map(str, command)), cwd=world, stdin=subprocess.DEVNULL, stdout=o, stderr=e, timeout=1800)
    if p.returncode or not out.is_file():
        tail = (folder / "stderr.log").read_bytes()[-4000:].decode(errors="replace")
        raise RuntimeError(f"start-up phases failed ({p.returncode}); see {folder}\n{tail}")
    return out.read_bytes()


def boot_digest(record):
    return sha(json.dumps({k: v for k, v in record.items() if k != "i"}, sort_keys=True, separators=(",", ":")).encode())


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
    plan = cases()
    first = play("pass-1", classpath, template, plan, header)
    second = play("pass-2", classpath, template, plan, header)
    if first != second:
        a, b = first.decode().splitlines(), second.decode().splitlines()
        where = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
        raise RuntimeError(f"two fresh runs differ at line {where + 1}; see {RUN / 'pass-1'} and {RUN / 'pass-2'}")
    lines = first.decode().splitlines()
    boot = json.loads(lines[0])
    if boot.get("t") != "boot":
        raise RuntimeError("the harness did not write the boot record first")
    digest = boot_digest(boot)
    if digest != fixture_boot["sha256"]:
        theirs = {k: v for k, v in fixture_boot.items() if k not in ("i", "sha256")}
        keys = sorted(k for k in set(theirs) | set(boot) if k != "i" and boot.get(k) != theirs.get(k))
        raise RuntimeError(f"the decomposed start-up's boot record differs from the world-trace fixture in {keys}")
    body = ("\n".join(lines[1:]) + "\n").encode()
    inputs = {}
    for name in PINNED:
        inputs[name] = (template / "data/configs" / name).read_bytes()
    case_file = (json.dumps(plan, indent=1, sort_keys=True) + "\n").encode()
    for module in sorted(HERE.glob("*.py")):
        read(module)
    for module in sorted(wt.HERE.glob("*.py")):
        read(module)
    read(ROOT / "oracle/login/generate.py")
    phases = [json.loads(l) for l in lines[1:] if json.loads(l)["t"] == "phase"]
    provenance = dict(
        format=1, schema=SCHEMA, upstream=lock, scope=SCOPE,
        sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm="sha256, CR removed",
        sourceTree=dict(root="reference/server/Server/src/main", files=count, manifestSha256=tree_digest),
        artifacts={name: sha(path.read_bytes()) for name, path in sorted(jars.items())},
        compilerArtifacts={path.name: sha(path.read_bytes()) for path in sorted(compiler)},
        kotlinCompiler="1.8.20", javaVersion=version, jvmFlags=wt.JVM_FLAGS, redirectedCallSites=sites, config=config,
        inputs={name: sha(data) for name, data in sorted(inputs.items())},
        bootRecord=dict(sha256=digest, identicalToWorldTrace=True, worldTraceFixtureSha256=sha(WORLD_TRACE.read_bytes())),
        counts=dict(phases=len(phases), tables=sum(json.loads(l)["t"] == "table" for l in lines[1:]),
                    cases=sum(json.loads(l)["t"] == "case" for l in lines[1:])),
        determinism=dict(freshJvmRuns=2, identical=True),
        casesSha256=sha(case_file), fixtureSha256=sha(body),
    )
    if check:
        same = (FIXTURE / "oracle.jsonl").read_bytes() == body and (FIXTURE / "cases.json").read_bytes() == case_file
        same = same and all((FIXTURE / name).read_bytes() == data for name, data in inputs.items())
        if not same or json.loads((FIXTURE / "provenance.json").read_text(encoding="utf-8")) != provenance:
            raise RuntimeError("Startup-loaders fixture/provenance differ; inspect before regeneration")
        print(f"Original start-up reproduced in {len(phases)} phases with {provenance['counts']['cases']} loader cases; "
              f"boot record identical to the world-trace fixture; two fresh runs identical.")
    else:
        FIXTURE.mkdir(parents=True, exist_ok=True)
        (FIXTURE / "oracle.jsonl").write_bytes(body)
        (FIXTURE / "cases.json").write_bytes(case_file)
        for name, data in inputs.items():
            (FIXTURE / name).write_bytes(data)
        (FIXTURE / "provenance.json").write_text(json.dumps(provenance, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"Generated the original start-up in {len(phases)} phases with {provenance['counts']['cases']} loader cases; "
              f"boot record identical to the world-trace fixture; two fresh runs identical.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--java-home", type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None)
    generate(args.check)
