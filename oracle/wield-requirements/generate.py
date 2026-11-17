"""The unchanged ItemDefinition.hasRequirement (the wield/use skill check).

The method's text is taken verbatim from the pinned ItemDefinition.java into
an adapter class holding only the definition's configuration; Skills.SKILL_NAME
and StringUtils.isPlusN are taken verbatim too. Player, its Skills levels and
PacketDispatch.sendMessage are recording adapters. The configuration is
absent, a value of another class, or a java.util.HashMap filled in the case's
order (its JDK iteration order is part of the result: the first failing skill
names the message). Java is the test oracle only; --check never updates the
fixtures.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import random
import re
import urllib.request

R = pathlib.Path(__file__).resolve().parents[2]
H = pathlib.Path(__file__).resolve().parent
O = R / "runs/wield-requirements-oracle"
S = R / "reference/server/Server/src/main"
spec = importlib.util.spec_from_file_location("compiler", R / "oracle/login/generate.py")
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

NAMES = 24  # Skills.SKILL_NAME.length in the pinned source (checked below)


def levels(seed_levels=None, default=1):
    out = [default] * NAMES
    for k, v in (seed_levels or {}).items():
        out[k] = v
    return out


def requests():
    rows = []

    def add(requirements, lv, wield=True, message=True, label=""):
        rows.append(dict(label=label, requirements=requirements, levels=lv, wield=wield, message=message))

    # No configuration, an empty map, a configuration of another class.
    for wield in [False, True]:
        for message in [False, True]:
            add(None, levels(), wield, message, "absent")
            add([], levels(), wield, message, "empty")
            add("text", levels(), wield, message, "not a map")
    # One skill: below, equal and above, every skill (the message names it,
    # "an" for the isPlusN names), both verbs and both message flags.
    for skill in range(NAMES):
        for have, need in [(1, 2), (2, 2), (3, 2), (98, 99), (99, 99), (0, 1), (1, 0)]:
            for wield in [False, True]:
                for message in [False, True]:
                    add([[skill, need]], levels({skill: have}), wield, message, f"skill {skill}")
    # Keys outside SKILL_NAME are skipped (24 is the first past the end).
    for key in [-2147483648, -100, -1, 24, 25, 32, 99, 2147483647]:
        for need in [0, 99, 2147483647]:
            add([[key, need]], levels(), True, True, f"key {key}")
            add([[key, need], [0, 2]], levels(), True, True, f"key {key} then attack")
            add([[0, 1], [key, need]], levels(), False, True, f"attack then key {key}")
    # Level boundaries.
    for need in [-2147483648, -1, 0, 1, 2147483647]:
        for have in [-2147483648, -1, 0, 1, 99, 2147483647]:
            add([[3, need]], levels({3: have}), True, True, "bounds")
    # JDK HashMap order: bucket collisions (16 and 0, 17 and 1), keys past the
    # table, resizes past twelve entries, reversed and shuffled insertion.
    pairs = [[16, 0], [0, 16], [17, 1], [1, 17], [33, 1], [1, 33], [48, 32, 16, 0], [23, 7], [7, 23]]
    for keys in pairs:
        for failing in keys:
            add([[k, 50 if k == failing else 1] for k in keys], levels({k: 10 for k in range(NAMES)}), True, True, f"order {keys}")
            add([[k, 50] for k in keys], levels({k: 10 for k in range(NAMES)}), True, True, f"all fail {keys}")
    for count in [12, 13, 16, 20, 24]:
        keys = list(range(count))
        for order in [keys, list(reversed(keys))]:
            add([[k, 50] for k in order], levels({k: 10 for k in range(NAMES)}), False, True, f"resize {count}")
            add([[k, 50 if k == order[-1] else 1] for k in order], levels({k: 10 for k in range(NAMES)}), True, True, f"resize {count} last")
    generator = random.Random(530)
    for n in range(300):
        size = generator.randint(1, 30)
        keys = generator.sample(range(-3, 40), min(size, 43))
        req = [[k, generator.randint(0, 99)] for k in keys]
        lv = [generator.randint(1, 99) for _ in range(NAMES)]
        add(req, lv, generator.random() < 0.5, generator.random() < 0.7, f"random {n}")
    return rows


def json_simple():
    artifact = json.loads(c.read(R / "oracle/auth/json.lock.json"))
    jar = O / "json-simple-1.1.1.jar"
    if not jar.exists() or hashlib.sha256(jar.read_bytes()).hexdigest() != artifact["sha256"]:
        with urllib.request.urlopen(artifact["url"], timeout=60) as response:
            data = response.read()
        assert len(data) == artifact["size"] and hashlib.sha256(data).hexdigest() == artifact["sha256"]
        jar.write_bytes(data)
    return jar


def generate(check):
    O.mkdir(parents=True, exist_ok=True)
    classes = O / "classes"
    classes.mkdir(exist_ok=True)
    java = []

    def adapter(name, text):
        p = O / "adapters" / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text, encoding="utf-8")
        java.append(p)

    definition = c.read(S / "core/cache/def/impl/ItemDefinition.java")
    skills = c.read(S / "core/game/node/entity/skill/Skills.java")
    strings = c.read(S / "core/tools/StringUtils.java")
    config = c.read(S / "core/game/system/config/ItemConfigParser.kt")
    names = re.search(r"public static final String\[\] SKILL_NAME = \{[^}]*\};", skills).group(0)
    assert names.count('"') // 2 == NAMES
    key = re.search(r'const val REQUIREMENTS = ("[^"]+")', config).group(1)
    adapter(
        "core/cache/def/impl/ItemDefinition.java",
        "package core.cache.def.impl;\n"
        "import core.game.node.entity.player.Player;\n"
        "import core.game.node.entity.skill.Skills;\n"
        "import core.tools.StringUtils;\n"
        "import core.game.system.config.ItemConfigParser;\n"
        "import java.util.Map;\n"
        "public class ItemDefinition {\n"
        "public Object configuration;\n"
        "@SuppressWarnings(\"unchecked\") public <V> V getConfiguration(String key) {"
        "if (!ItemConfigParser.REQUIREMENTS.equals(key)) throw new AssertionError(key);"
        "return (V) configuration; }\n"
        + c.method(definition, "public boolean hasRequirement(")
        + "\n}\n",
    )
    adapter(
        "core/game/system/config/ItemConfigParser.java",
        "package core.game.system.config;public class ItemConfigParser{public static final String REQUIREMENTS="
        + key
        + ";}",
    )
    adapter(
        "core/game/node/entity/skill/Skills.java",
        "package core.game.node.entity.skill;public class Skills{"
        + names
        + "private final int[] staticLevels;public Skills(int[] s){staticLevels=s;}"
        "public int getStaticLevel(int slot){return staticLevels[slot];}}",
    )
    adapter(
        "core/tools/StringUtils.java",
        "package core.tools;public class StringUtils{" + c.method(strings, "public static boolean isPlusN(") + "}",
    )
    adapter(
        "core/game/node/entity/player/link/PacketDispatch.java",
        "package core.game.node.entity.player.link;public class PacketDispatch{"
        "public void sendMessage(String m){oracle.WieldRequirementsOracle.MESSAGES.add(m);}}",
    )
    adapter(
        "core/game/node/entity/player/Player.java",
        "package core.game.node.entity.player;public class Player{"
        "private final core.game.node.entity.skill.Skills skills;"
        "private final core.game.node.entity.player.link.PacketDispatch dispatch=new core.game.node.entity.player.link.PacketDispatch();"
        "public Player(int[] s){skills=new core.game.node.entity.skill.Skills(s);}"
        "public core.game.node.entity.skill.Skills getSkills(){return skills;}"
        "public core.game.node.entity.player.link.PacketDispatch getPacketDispatch(){return dispatch;}}",
    )
    c.read(H / "WieldRequirementsOracle.java")
    java.append(H / "WieldRequirementsOracle.java")
    jar = json_simple()
    cp = os.pathsep.join(map(str, [classes, jar]))
    c.run(["javac", "--release", "11", "-encoding", "UTF-8", "-cp", cp, "-d", classes, *java])
    cases = requests()
    data = "".join(json.dumps(row) + "\n" for row in cases).encode()
    exe = c.JAVA_HOME / "bin" / ("java.exe" if os.name == "nt" else "java") if c.JAVA_HOME else "java"
    import subprocess

    p = subprocess.run(
        [str(exe), "-Dfile.encoding=UTF-8", "-cp", cp, "oracle.WieldRequirementsOracle"],
        cwd=R,
        input=data,
        capture_output=True,
        timeout=120,
    )
    assert p.returncode == 0, p.stderr.decode(errors="replace")
    rows = [json.loads(line) for line in p.stdout.splitlines() if line]
    assert len(rows) == len(cases)
    assert all({k: v for k, v in row.items() if k != "expected"} == case for row, case in zip(rows, cases))
    fixture = "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in rows).encode()
    c.read(pathlib.Path(__file__))
    provenance = dict(
        format=1,
        upstream=json.loads(c.read(R / "upstream.lock.json")),
        sources=dict(sorted(c.SOURCES.items())),
        caseCount=len(rows),
        fixtureSha256=hashlib.sha256(fixture).hexdigest(),
        scope=(
            "ItemDefinition.hasRequirement verbatim from the pinned source, with Skills.SKILL_NAME and "
            "StringUtils.isPlusN verbatim; the configuration (absent, another class, or a java.util.HashMap "
            "filled in the case's order), Skills.getStaticLevel, Player and PacketDispatch.sendMessage are "
            "adapters. Compares the result, the exception class and the messages. Null map values and "
            "non-Integer keys are not covered (the native configuration holds int pairs)."
        ),
    )
    folder = R / "fixtures/wield-requirements"
    folder.mkdir(exist_ok=True)
    if check:
        assert (folder / "oracle.jsonl").read_bytes() == fixture, "fixture differs"
        assert json.loads((folder / "provenance.json").read_text()) == provenance, "provenance differs"
    else:
        (folder / "oracle.jsonl").write_bytes(fixture)
        (folder / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    print(len(rows), "hasRequirement cases reproduced" if check else "hasRequirement cases generated")


if __name__ == "__main__":
    a = argparse.ArgumentParser(description=__doc__)
    a.add_argument("--check", action="store_true")
    a.add_argument("--java-home", type=pathlib.Path)
    args = a.parse_args()
    c.JAVA_HOME = args.java_home or (
        pathlib.Path(os.environ["RS09_ORACLE_JAVA_HOME"]) if "RS09_ORACLE_JAVA_HOME" in os.environ else None
    )
    generate(args.check)
