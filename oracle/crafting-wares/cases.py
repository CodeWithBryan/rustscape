"""Designed corpus for battlestaves, glassblowing, molten glass and spiky vambraces. Table rows are read from the pinned sources, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main/content/global/skill/crafting"
STAFF, PIPE, GLASS, ASH, SAND, CLAWS, INTERFACE = 1391, 1785, 1775, 1781, 1783, 10113, 542
ONE, FIVE, ALL, X = 155, 196, 124, 199


def text(name):
    return (MAIN / name).read_bytes().replace(b"\r", b"").decode("utf-8")


def staves():
    rows = re.findall(r"^        [A-Z_]+\(Items\.[A-Z_]+_(\d+), Items\.[A-Z_]+_(\d+), 1, (\d+), [\d.]+\)", text("BattlestaffListener.kt"), re.M)
    if len(rows) != 4:
        raise RuntimeError("BattlestaffProduct differs from the 4 rows this corpus expects")
    return [dict(orb=int(orb), product=int(product), level=int(level)) for orb, product, level in rows]


def glass():
    rows = re.findall(r"^    [A-Z_]+\((\d+), (\d+), 1, (\d+), [\d.]+\)", text("glass/GlassProduct.kt"), re.M)
    if len(rows) != 8:
        raise RuntimeError("GlassProduct differs from the 8 rows this corpus expects")
    return [dict(button=int(button), product=int(product), level=int(level)) for button, product, level in rows]


def furnaces():
    ids = [int(n) for n in re.search(r"FURNACES = intArrayOf\(([\d, ]+)\)", text("glass/GlassMakeListener.kt")).group(1).split(", ")]
    if len(ids) != 13:
        raise RuntimeError("GlassMakeListener differs from the 13 furnaces this corpus expects")
    return ids


def vambraces():
    ids = [int(n) for n in re.findall(r"addHandler\(Items\.[A-Z_]+_(\d+), ITEM_TYPE, this\)", text("leather/SpikyVambraces.kt"))]
    if len(ids) != 5:
        raise RuntimeError("SpikyVambraces differs from the 5 vambraces this corpus expects")
    return ids


def state(**changes):
    base = dict(level=99, held={})
    base.update(changes)
    return base


def held(*pairs):
    return {str(id): amount for id, amount in pairs}


def use(used, with_):
    return dict(op="use", used=used, **{"with": with_})


def handle(used, with_):
    return dict(op="handle", used=used, **{"with": with_})


def button(opcode, id, component=INTERFACE):
    return dict(op="button", component=component, opcode=opcode, button=id)


def create(amount):
    return dict(op="create", amount=amount, index=0)


def enter(value):
    return dict(op="input", value=value)


def change(**changes):
    return dict(op="set", state=changes)


PULSE, GET_ALL = dict(op="pulse"), dict(op="getAll", index=0)


def census(out):
    kinds = {}
    for case in out:
        for op, result in zip(case["ops"], case["expected"]):
            key = op["op"] + (":throws" if "throws" in result else "")
            kinds[key] = kinds.get(key, 0) + 1
    return dict(sorted(kinds.items()))


def staff_cases(rows):
    out = []
    for row in rows:
        orb, level = row["orb"], row["level"]
        for pair in [(orb, STAFF), (STAFF, orb)]:
            for at in [level, level - 1]:
                for orbs, staffs in [(0, 0), (1, 1), (1, 3), (3, 1), (2, 2), (5, 3), (0, 2), (2, 0)]:
                    out.append(dict(state=state(level=at, held=held((orb, orbs), (STAFF, staffs))),
                                    ops=[use(*pair), GET_ALL, create(2)] + [PULSE] * 4))
        stocked = held((orb, 4), (STAFF, 4))
        for amount in [0, -1, 1, 3, 28]:
            out.append(dict(state=state(held=stocked), ops=[use(orb, STAFF), create(amount)] + [PULSE] * 6))
        # The world changes under a running task.
        for changes in [dict(held=held((orb, 4))), dict(held=held((STAFF, 4))), dict(level=1), dict(held={})]:
            out.append(dict(state=state(held=stocked), ops=[use(orb, STAFF), create(4), PULSE, change(**changes), PULSE, PULSE, PULSE]))
    out.append(dict(state=state(held=held((rows[0]["orb"], 3), (STAFF, 3))), ops=[PULSE, GET_ALL, create(1), use(STAFF, STAFF), use(5, 6), use(0, 0)]))
    return out


def glass_cases(rows):
    out = []
    stocked = held((PIPE, 1), (GLASS, 4))
    for pair in [(PIPE, GLASS), (GLASS, PIPE)]:
        for holding in [stocked, {}]:
            out.append(dict(state=state(held=holding), ops=[use(*pair)]))
    # Every button around the eight, with every opcode.
    for id in range(36, 48):
        for opcode in [ONE, FIVE, ALL, X, 0]:
            out.append(dict(state=state(held=stocked), ops=[button(opcode, id), enter(2)] + [PULSE] * 6))
    for row in rows:
        id, level = row["button"], row["level"]
        for at in [level, level - 1]:
            for holding in [stocked, held((PIPE, 1)), held((GLASS, 4)), held((PIPE, 3), (GLASS, 1)), {}]:
                for opcode in [ONE, ALL]:
                    out.append(dict(state=state(level=at, held=holding), ops=[button(opcode, id)] + [PULSE] * 3))
        for value in [0, -1, 1, 3, 28]:
            out.append(dict(state=state(held=stocked), ops=[button(X, id), enter(value)] + [PULSE] * 5))
        # The world changes under a running pulse; the level is not asked again.
        for changes in [dict(held=held((GLASS, 4))), dict(held=held((PIPE, 1))), dict(level=1), dict(held=held((PIPE, 1), (GLASS, 9)))]:
            out.append(dict(state=state(held=stocked), ops=[button(FIVE, id), PULSE, change(**changes), PULSE, PULSE, PULSE, PULSE, PULSE]))
    out.append(dict(state=state(held=stocked), ops=[enter(3), PULSE, button(ONE, 38, 541), button(X, 38), button(ONE, 40), enter(2), PULSE, PULSE, PULSE]))
    return out


def glass_make_cases(ids):
    out = []
    for furnace in ids:
        for input in [ASH, SAND]:
            for pair in [(input, furnace), (furnace, input)]:
                out.append(dict(state=state(held=held((ASH, 2), (SAND, 3))), ops=[use(*pair), GET_ALL, create(5)] + [PULSE] * 4))
    furnace = ids[0]
    for ash, sand in [(0, 0), (1, 0), (0, 1), (1, 1), (3, 2), (5, 5)]:
        for level in [1, 99]:
            for amount in [1, 3]:
                out.append(dict(state=state(level=level, held=held((ASH, ash), (SAND, sand))),
                                ops=[use(ASH, furnace), GET_ALL, create(amount)] + [PULSE] * 5))
    stocked = held((ASH, 6), (SAND, 6))
    for amount in [0, -1, 28]:
        out.append(dict(state=state(held=stocked), ops=[use(SAND, furnace), create(amount)] + [PULSE] * 8))
    for changes in [dict(held=held((ASH, 6))), dict(held=held((SAND, 6))), dict(held=held((ASH, 1), (SAND, 1))), dict(held={})]:
        out.append(dict(state=state(held=stocked), ops=[use(ASH, furnace), create(4), PULSE, change(**changes), PULSE, PULSE, PULSE]))
    out.append(dict(state=state(held=stocked), ops=[use(ASH, 1), use(GLASS, furnace), use(ASH, SAND), GET_ALL, create(1), PULSE]))
    return out


def vambrace_cases(ids):
    out = []
    for vamb in ids:
        for pair in [(CLAWS, vamb), (vamb, CLAWS)]:
            for level in [32, 31, 99, 1]:
                for holding in [held((CLAWS, 1), (vamb, 1)), held((CLAWS, 2), (vamb, 3)), held((vamb, 1)), held((CLAWS, 1)), {}]:
                    out.append(dict(state=state(level=level, held=holding), ops=[handle(*pair), handle(*pair)]))
    full = held((CLAWS, 5), (ids[0], 1), (10077, 1), (5, 1))
    for level in [1, 99]:
        out.append(dict(state=state(level=level, held=full),
                        ops=[handle(CLAWS, CLAWS), handle(CLAWS, 10077), handle(CLAWS, 5), handle(5, ids[0]), handle(0, 0)]))
    return out


def requests():
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="tables")])]
    cases += staff_cases(staves())
    cases += glass_cases(glass())
    cases += glass_make_cases(furnaces())
    cases += vambrace_cases(vambraces())
    for n, case in enumerate(cases):
        case["id"] = n
    return cases
