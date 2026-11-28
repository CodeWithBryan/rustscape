"""Designed pickpocketing corpus. Table rows are read from the pinned source, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "reference/server/Server/src/main/content/global/skill/thieving/Pickpockets.kt"


def constants():
    """Each constant's name, ids and required level, from the pinned enum."""
    text = SOURCE.read_bytes().replace(b"\r", b"").decode("utf-8")
    body = text.split("{", 1)[1].split("companion object")[0]
    out = []
    for match in re.finditer(r"^    ([A-Z_]+)\((.*?)WeightBasedTable\.create\(", body, re.S | re.M):
        name, head = match.group(1), match.group(2)
        span = re.match(r"\s*IntStream\.rangeClosed\((\d+), (\d+)\)\.toArray\(\),(.*)", head, re.S)
        if span:
            ids, rest = list(range(int(span.group(1)), int(span.group(2)) + 1)), span.group(3)
        else:
            listed = re.match(r"\s*intArrayOf\((.*?)\),(.*)", head, re.S)
            ids = [int(re.search(r"(\d+)$", token.strip()).group(1)) for token in listed.group(1).split(",")]
            rest = listed.group(2)
        out.append(dict(name=name, ids=ids, level=int(rest.split(",")[0].strip())))
    if len(out) != 18:
        raise RuntimeError("Pickpockets differs from the 18 constants this corpus expects")
    return out


def state(**changes):
    base = dict(level=99, inCombat=False, canRoll=True, hasClue=False, smoothHands=False, gloves=False, chance=1.0,
                loot=[[995, 3]], draws=[1, 0, 1, 0], attackAnimation=190)
    base.update(changes)
    return base


def change(**values):
    return dict(op="set", state=values)


def pick(npc):
    return dict(op="pickpocket", npc=npc)


CHANCES = [1.0, 12.5, 30.0, 47.265625, 47.27, 60.0, 75.0, 93.75, 94.0, 97.0, 99.9]
LOOTS = [[], [[995, 3]], [[995, 9], [5318, 1]], [[1, 1], [2677, 1], [995, 40]]]


def requests():
    all_constants = constants()
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="tables")])]
    # Every id of the map and its neighbours, by lookup and by the lambda.
    listed = sorted({i for constant in all_constants for i in constant["ids"]})
    probe = sorted({i + d for i in listed for d in (-1, 0, 1)} | {-1, 0, 65535})
    cases.append(dict(state=state(), ops=[dict(op="forID", npc=i) for i in probe]))
    for start in range(0, len(probe), 40):
        cases.append(dict(state=state(), ops=[pick(i) for i in probe[start:start + 40]]))
        cases.append(dict(state=state(chance=99.99, level=1), ops=[pick(i) for i in probe[start:start + 40]]))
    for constant in all_constants:
        npc, required = constant["ids"][0], constant["level"]
        last = constant["ids"][-1]
        # The checks before the attempt, in order.
        for combat in [True, False]:
            for level in sorted({1, required - 1, required, required + 1, 99}):
                for can in [True, False]:
                    cases.append(dict(state=state(inCombat=combat, level=level, canRoll=can), ops=[pick(npc), pick(last)]))
        # The attempt: the chance against the level's threshold with both bonuses, with and without a clue.
        for level in sorted({required, (required + 99) // 2, 99}):
            for smooth in [False, True]:
                for gloves in [False, True]:
                    for clue in [False, True]:
                        ops = []
                        for chance in CHANCES:
                            ops += [change(chance=chance, draws=[1, 0, 1, 0]), pick(npc)]
                        cases.append(dict(state=state(level=level, smoothHands=smooth, gloves=gloves, hasClue=clue), ops=ops))
        # What a success hands over, and each damage draw of a failure.
        for loot in LOOTS:
            cases.append(dict(state=state(loot=loot), ops=[pick(npc), change(hasClue=True), pick(last)]))
        for draws in [[0, 1], [1, 0], [2, 2], [5, 5]]:
            for animation in [190, -1]:
                cases.append(dict(state=state(chance=99.99, level=required, draws=draws, attackAnimation=animation),
                                  ops=[pick(npc), pick(last)]))
    for number, case in enumerate(cases):
        case["id"] = number
    return cases


def census(out):
    kinds = {}
    for case in out:
        for op, result in zip(case["ops"], case["expected"]):
            key = op["op"] + (":throws" if "throws" in result else "")
            kinds[key] = kinds.get(key, 0) + 1
    return dict(sorted(kinds.items()))
