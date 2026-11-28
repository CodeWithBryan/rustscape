"""Designed corpus for grinding, herb tar and barbarian mixes. Table rows are read from the pinned sources, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main/content/global/skill/herblore"
DRUIDIC = "DRUIDIC_RITUAL"
STAGE = "/save:barbtraining:herblore"
PESTLE, SWAMP_TAR, BAIT, ROE, CAVIAR = 233, 1939, 313, 11324, 11326
# Declared item-name inputs (the cache definitions are not loaded by the oracle): the tars' ingredients.
NAMES = {"6681": "Ground guam"}


def text(name):
    return (MAIN / name).read_bytes().replace(b"\r", b"").decode("utf-8")


def grinding():
    rows = re.findall(r"^\t([A-Z_]+)\(new Item\[\] \{ ?new Item\(Items\.[A-Z_0-9]+?_(\d+)\) ?\}, new Item\(Items\.[A-Z_0-9]+?_(\d+)\)\)", text("GrindingItem.java"), re.M)
    if len(rows) != 17:
        raise RuntimeError("GrindingItem differs from the 17 rows this corpus expects")
    return [dict(name=name, item=int(item), product=int(product)) for name, item, product in rows]


def tars():
    herbs = {name: int(product) for name, product in re.findall(r"([A-Z_]+)\(new Item\(\d+\), [\d.]+, \d+, new Item\((\d+)\)\)", text("Herbs.java"))}
    out = []
    for name, ingredient, level, tar in re.findall(r"^\t([A-Z_]+)\((.+?), (\d+), [\d.]+, new Item\((\d+)\)\)", text("Tars.java"), re.M):
        herb = re.fullmatch(r"Herbs\.([A-Z_]+)\.getProduct\(\)", ingredient)
        if herb:
            id = herbs[herb.group(1)]
            NAMES[str(id)] = "Clean " + herb.group(1).replace("_", " ").lower()
        else:
            id = int(re.fullmatch(r"new Item\(Items\.[A-Z_]+_(\d+)\)", ingredient).group(1))
        out.append(dict(name=name, ingredient=id, level=int(level), tar=int(tar)))
    if len(out) != 5:
        raise RuntimeError("Tars differs from the 5 rows this corpus expects")
    return out


def mixes():
    rows = re.findall(r"([A-Z_]+)\((\d+), (\d+), (\d+), [\d.]+, (true|false)\)", text("BarbarianPotion.kt"))
    if len(rows) != 24:
        raise RuntimeError("BarbarianPotion differs from the 24 rows this corpus expects")
    return [dict(name=name, item=int(item), level=int(level), product=int(product), both=both == "true") for name, item, level, product, both in rows]


def state(**changes):
    base = dict(level=99, staticLevel=99, held={}, quests={DRUIDIC: 100}, attributes={}, names={})
    base.update(changes)
    return base


def held(*pairs):
    return {str(id): amount for id, amount in pairs}


def handle(plugin, used, with_):
    return dict(op="handle", plugin=plugin, used=used, **{"with": with_})


def use(used, with_, op="use"):
    return dict(op=op, used=used, **{"with": with_})


def create(amount, index=0):
    return dict(op="create", amount=amount, index=index)


def change(**changes):
    return dict(op="set", state=changes)


PULSE, STOP, GET_ALL = dict(op="pulse"), dict(op="stop"), dict(op="getAll", index=0)


def pulses(count):
    return [PULSE] * count + [STOP]


def census(out):
    kinds = {}
    for case in out:
        for op, result in zip(case["ops"], case["expected"]):
            key = op["op"] + (":throws" if "throws" in result else "")
            kinds[key] = kinds.get(key, 0) + 1
    return dict(sorted(kinds.items()))


def grind_cases(rows):
    out = []
    for row in rows:
        item = row["item"]
        amounts = [0, 1, 9, 10, 11, 25, 100] if item == BAIT else [0, 1, 3]
        asked = [1, 2, 3, 10, 28] if item == BAIT else [1, 2, 5]
        for have in amounts:
            for amount in asked:
                out.append(dict(state=state(held=held((item, have), (PESTLE, 1))),
                                ops=[handle("grind", item, PESTLE), GET_ALL, create(amount)] + pulses(min(amount, 4) + 1)))
        # The pestle and mortar used on the item: the pulse's node is the pestle and mortar.
        for pestles in [0, 1, 2]:
            out.append(dict(state=state(held=held((item, 3), (PESTLE, pestles))),
                            ops=[handle("grind", PESTLE, item), GET_ALL, create(2), PULSE, PULSE, STOP]))
        # The item used on something that is no pestle and mortar: the handler does not look.
        out.append(dict(state=state(held=held((item, 2))), ops=[handle("grind", item, 1), GET_ALL, create(1, 3)] + pulses(2)))
    item = rows[0]["item"]
    stocked = held((item, 6), (PESTLE, 1))
    # Events with no grinding item.
    out.append(dict(state=state(held=stocked), ops=[handle("grind", PESTLE, PESTLE), handle("grind", 1, 2), handle("grind", PESTLE, 0), GET_ALL, create(1)]))
    # What a running pulse does when the inventory changes under it, and amounts the dialogue would not offer.
    for changes in [dict(held=held((item, 0))), dict(held=held((item, 1))), dict(held=held((item, 50)))]:
        out.append(dict(state=state(held=stocked), ops=[handle("grind", item, PESTLE), create(4), PULSE, change(**changes), PULSE, PULSE, PULSE, STOP]))
    for amount in [0, -1, 28, 2147483647]:
        out.append(dict(state=state(held=stocked), ops=[handle("grind", item, PESTLE), create(amount)] + pulses(3)))
    out.append(dict(state=state(held=stocked), ops=[PULSE, STOP, GET_ALL, create(1), handle("grind", item, PESTLE), dict(op="getAll", index=7), STOP, PULSE]))
    # Fishing bait that changes between the constructor and the reward.
    for start, later in [(25, 5), (25, 0), (5, 40), (10, 10), (19, 9)]:
        out.append(dict(state=state(held=held((BAIT, start))),
                        ops=[handle("grind", BAIT, PESTLE), create(3), PULSE, change(held=held((BAIT, later))), PULSE, PULSE, STOP]))
    return out


def tar_cases(rows):
    out = []
    for row in rows:
        ingredient, level = row["ingredient"], row["level"]
        full = held((SWAMP_TAR, 45), (ingredient, 3), (PESTLE, 1))
        for pair in [(ingredient, SWAMP_TAR), (SWAMP_TAR, ingredient)]:
            for amount in [1, 2, 5]:
                out.append(dict(state=state(held=full), ops=[handle("tar", *pair), GET_ALL, create(amount)] + pulses(amount + 2)))
        for changes in [dict(level=level - 1), dict(level=level), dict(quests={DRUIDIC: 99}), dict(quests={}), dict(quests={DRUIDIC: 101}),
                        dict(held=held((SWAMP_TAR, 45), (ingredient, 3))), dict(held=held((SWAMP_TAR, 14), (ingredient, 3), (PESTLE, 1))),
                        dict(held=held((SWAMP_TAR, 15), (ingredient, 3), (PESTLE, 1))), dict(held=held((SWAMP_TAR, 29), (ingredient, 3), (PESTLE, 1))),
                        dict(held=held((SWAMP_TAR, 45), (ingredient, 0), (PESTLE, 1))), dict(held=held((SWAMP_TAR, 45), (ingredient, 1), (PESTLE, 1))),
                        dict(held={}), dict(level=level - 1, quests={}, held={})]:
            out.append(dict(state=state(**{**dict(held=full), **changes}), ops=[handle("tar", ingredient, SWAMP_TAR), create(2)] + pulses(4)))
        # The world changes under a running pulse.
        for changes in [dict(level=level - 1), dict(quests={}), dict(held=held((SWAMP_TAR, 45), (ingredient, 3))),
                        dict(held=held((SWAMP_TAR, 14), (ingredient, 3), (PESTLE, 1))), dict(held=held((SWAMP_TAR, 45), (PESTLE, 1)))]:
            out.append(dict(state=state(held=full), ops=[handle("tar", ingredient, SWAMP_TAR), create(3), PULSE, PULSE, change(**changes), PULSE, PULSE, STOP]))
        # The ingredient's name in the message.
        for name in [None, "CLEAN GUAM", "Guam clean", "cleanclean", "", "  Clean  guam  ", "\tClean guam\n", "Unclean guam", "CleAn Guam"]:
            out.append(dict(state=state(held=full, names={str(ingredient): name}), ops=[handle("tar", ingredient, SWAMP_TAR), create(1)] + pulses(3)))
    ingredient = rows[0]["ingredient"]
    full = held((SWAMP_TAR, 45), (ingredient, 3), (PESTLE, 1))
    # Events with no tar ingredient, and an ingredient used on something that is no swamp tar.
    out.append(dict(state=state(held=full), ops=[handle("tar", SWAMP_TAR, SWAMP_TAR), handle("tar", SWAMP_TAR, 5), handle("tar", 5, 6), GET_ALL, create(1)]))
    out.append(dict(state=state(held=full), ops=[handle("tar", ingredient, 5), GET_ALL, create(1, 2)] + pulses(3)))
    for amount in [0, -1, 28]:
        out.append(dict(state=state(held=held((SWAMP_TAR, 600), (ingredient, 28), (PESTLE, 1))), ops=[handle("tar", ingredient, SWAMP_TAR), create(amount)] + pulses(5)))
    out.append(dict(state=state(held=full), ops=[PULSE, STOP, GET_ALL, create(1), handle("tar", ingredient, SWAMP_TAR), dict(op="getAll", index=7), STOP, PULSE]))
    return out


def mix_cases(rows):
    out = []
    for row in rows:
        item, level = row["item"], row["level"]
        for egg in [ROE, CAVIAR]:
            op = "use" if egg == CAVIAR or row["both"] else "mix"
            both = held((item, 2), (egg, 2))
            for stage in [None, 0, 1, 2, 3, -1]:
                attributes = {} if stage is None else {STAGE: stage}
                out.append(dict(state=state(held=both, attributes=attributes), ops=[use(item, egg, op), use(item, egg, op), use(item, egg, op)]))
            for changes in [dict(staticLevel=level - 1), dict(staticLevel=level), dict(staticLevel=level, level=1), dict(held=held((egg, 1))),
                            dict(held=held((item, 1))), dict(held={})]:
                out.append(dict(state=state(**{**dict(held=both, attributes={STAGE: 1}), **changes}), ops=[use(item, egg, op), use(egg, item, op)]))
    item = rows[0]["item"]
    both = held((item, 2), (ROE, 2), (CAVIAR, 2))
    # A stage that is no whole number.
    for value in ["1", True, False, ""]:
        out.append(dict(state=state(held=both, attributes={STAGE: value}), ops=[use(item, ROE), use(item, CAVIAR)]))
    # The listener's handle with nodes no registration names, and pairs no registration names.
    pairs = [(0, ROE), (ROE, item), (item, item), (item, 1), (rows[5]["item"], ROE), (rows[5]["product"], CAVIAR), (rows[0]["product"], CAVIAR)]
    for stage in [0, 1]:
        out.append(dict(state=state(held=both, attributes={STAGE: stage}), ops=[use(*pair, "mix") for pair in pairs]))
    out.append(dict(state=state(held=both, attributes={STAGE: 1}), ops=[use(*pair) for pair in pairs + [(ROE, item), (CAVIAR, rows[5]["item"])]]))
    return out


def requests():
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="tables")])]
    cases += grind_cases(grinding())
    cases += tar_cases(tars())
    cases += mix_cases(mixes())
    for n, case in enumerate(cases):
        case["id"] = n
    return cases
