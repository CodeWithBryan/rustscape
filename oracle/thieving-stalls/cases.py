"""Designed stall thieving corpus. Stall rows are read from the pinned source, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "reference/server/Server/src/main/content/global/skill/thieving/Stall.java"

NAMES = {
    "1957": "Onion", "1965": "Cabbage", "1942": "Potato", "1982": "Tomato", "1550": "Garlic", "1891": "Cake",
    "950": "Silk", "442": "Silver ore", "958": "Grey wolf fur", "331": "Raw salmon", "1963": "Banana",
    "712": "Cup of tea", "2007": "Spice", "36": "Candle", "1623": "Uncut sapphire", "1605": "Emerald",
    "1592": "Ring mould", "877": "Bronze bolts", "1935": "Jug", "5096": "Marigold seed", "1323": "Iron scimitar",
    "556": "Air rune", "1931": "Pot",
}


def trailing(token):
    return int(re.search(r"(\d+)\s*$", token.strip()).group(1))


def constants():
    """Each constant's name, full ids, level and reward count, from the pinned enum."""
    text = SOURCE.read_bytes().replace(b"\r", b"").decode("utf-8")
    body = text.split("public enum Stall {", 1)[1].split("public static HashMap", 1)[0]
    out = []
    pattern = r"^\t([A-Z_]+)\(new Integer\[\]\s*\{([^}]*)\},\s*new Integer\[\]\s*\{([^}]*)\},\s*(\d+),\s*new Item\[\]\{(.*?)\},\s*[\d.]+,\s*\d+,"
    for match in re.finditer(pattern, body, re.M):
        name, full, _, level, rewards = match.groups()
        out.append(dict(name=name, ids=[trailing(t) for t in full.split(",")], level=int(level),
                        rewards=len(re.findall(r"new Item\(", rewards))))
    if len(out) != 18:
        raise RuntimeError("Stall differs from the 18 constants this corpus expects")
    return out


def state(**changes):
    base = dict(ticks=100, attr={}, playerActive=True, nodeActive=True, inCombat=False, level=99,
                nodeName="Market stall", freeSlots=5, region=12342, questComplete=False, draws=[10, 0, 10, 0],
                names={}, npcs=[])
    base.update(changes)
    return base


def change(**values):
    return dict(op="set", state=values)


def handle(node, option="steal-from"):
    return dict(op="handle", node=node, option=option)


UPDATE = dict(op="update")
STOP = dict(op="stop")
ATTEMPT = [UPDATE, UPDATE, UPDATE, UPDATE]
NPCS = [
    [], [[32, False]], [[32, True]], [[2236, False]], [[1, False], [32, True], [2236, False], [32, False]],
    [[33, False], [2235, False], [2237, False]], [[2236, True], [32, True]], [[32, False], [2236, False]],
]


def requests():
    stalls = constants()
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="stalls")])]
    listed = sorted({i for s in stalls for i in s["ids"]})
    probe = sorted({i + d for i in listed for d in (-1, 0, 1)} | {-1, 0, 634, 6984, 34381, 65535})
    cases.append(dict(state=state(), ops=[dict(op="forObject", node=i) for i in probe]))
    # getEmpty of every constant for every listed id (an id it does not list, or lists past its empties, throws).
    for s in stalls:
        cases.append(dict(state=state(), ops=[dict(op="empty", stall=s["name"], node=i) for i in listed]))
    # The plugin for every probed id and every option it is given.
    for start in range(0, len(probe), 20):
        cases.append(dict(state=state(draws=[10, 0] * 20), ops=[handle(i) for i in probe[start:start + 20]]))
    for option in ["steal-from", "steal from", "steal"]:
        cases.append(dict(state=state(), ops=[handle(2560, option), handle(1, option)] + ATTEMPT))
    # An option the switch does not name starts nothing.
    cases.append(dict(state=state(), ops=[handle(2560, option) for option in ["Steal-from", "steal-From", "pickpocket", ""]]))
    cases.append(dict(state=state(), ops=[handle(1), UPDATE, STOP, UPDATE]))
    for s in stalls:
        first, level = s["ids"][0], s["level"]
        # The wide grids run for three constants; the others take a sample of each.
        wide = s["name"] in ["VEGETABLE_STALL", "SILK_STALL", "CANDLES"]
        # Every id of the constant and every reward it can draw.
        for node in s["ids"]:
            for loot in range(s["rewards"]):
                cases.append(dict(state=state(draws=[10, loot], names=NAMES), ops=[handle(node)] + ATTEMPT))
        # The requirements at the start, in order.
        for combat in [False, True]:
            for have in sorted({1, level - 1, level, level + 1, 99}):
                for free in [0, 1, 28]:
                    cases.append(dict(state=state(inCombat=combat, level=have, freeSlots=free, nodeName="Silk Stall"),
                                      ops=[handle(first)] + ATTEMPT))
        # The Rellekka keepers.
        for node in s["ids"]:
            for region in [10553, 10554]:
                for quest in [False, True]:
                    cases.append(dict(state=state(region=region, questComplete=quest), ops=[handle(node)] + ATTEMPT))
        # A requirement lost on each tick of the attempt.
        for tick in range(3):
            for lost in [dict(inCombat=True), dict(level=level - 1), dict(freeSlots=0), dict(region=10553)]:
                ops = [handle(first)] + [UPDATE] * tick + [change(**lost)] + ATTEMPT
                cases.append(dict(state=state(), ops=ops))
        # The draw that decides, with the NPCs around.
        for draw in [0, 3, 4, 14]:
            for npcs in NPCS if wide else [NPCS[0], NPCS[4]]:
                cases.append(dict(state=state(draws=[draw, 0, draw, 0], npcs=npcs), ops=[handle(first)] + ATTEMPT + [handle(first)] + ATTEMPT))
        # A scenery that is no longer active, a pulse stopped from outside, and the inactive check.
        cases.append(dict(state=state(nodeActive=False), ops=[handle(first)] + ATTEMPT))
        for tick in range(3):
            cases.append(dict(state=state(), ops=[handle(first)] + [UPDATE] * tick + [STOP] + ATTEMPT))
            for delay in [99, 100, 101, 105] if wide else [101]:
                for gone in [dict(playerActive=False), dict(nodeActive=False), dict(playerActive=False, nodeActive=False), dict()]:
                    ops = [handle(first)] + [UPDATE] * tick + [change(attr=dict(thieveDelay=delay), **gone)] + ATTEMPT
                    cases.append(dict(state=state(), ops=ops))
            cases.append(dict(state=state(), ops=[handle(first)] + [UPDATE] * tick + [change(ticks=90)] + ATTEMPT))
        # The article and the name's case; an empty name throws inside the pulse.
        for name in ["Hat", "herring", "Hh", "h", "EGG", "Uncut opal", "Íris", "", "İstanbul", "x"]:
            cases.append(dict(state=state(names={"*": name}, nodeName=name or "Stall"),
                              ops=[handle(first)] + ATTEMPT))
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
