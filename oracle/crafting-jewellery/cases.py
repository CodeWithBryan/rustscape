"""Designed corpus for jewellery casting. Table rows are read from the pinned sources, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main/content/global"
BAR, RING, AMULET, NECKLACE, BRACELET = 2357, 1592, 1595, 1597, 11065
MOULDS = [RING, NECKLACE, AMULET, BRACELET]
ONE, FIVE, ALL, X = 155, 196, 124, 199
# Declared names of the products, in the table's order; the live world reads the cache's.
NAMES = [
    "Gold ring", "Sapphire ring", "Emerald ring", "Ruby ring", "Diamond ring", "Dragonstone ring", "Onyx ring",
    "Gold necklace", "Sapphire necklace", "Emerald necklace", "Ruby necklace", "Diamond necklace", "Dragon necklace",
    "Onyx necklace", "Ring of slaying(8)", "Gold amulet", "Sapphire amulet", "Emerald amulet", "Ruby amulet",
    "Diamond amulet", "Dragonstone ammy", "Onyx amulet", "Gold bracelet", "Sapphire bracelet", "Emerald bracelet",
    "Ruby bracelet", "Diamond bracelet", "Dragon bracelet", "Onyx bracelet",
]


def text(name):
    return (MAIN / name).read_bytes().replace(b"\r", b"").decode("utf-8")


def table():
    source = text("skill/crafting/jewellery/JewelleryCrafting.java")
    constants = dict(re.findall(r"([A-Z_]+) = (\d+)", re.search(r"public static final int RING_MOULD[^;]*;", source).group()))
    rows = re.findall(r"^\t\t([A-Z_]+)\((\d+), (\d+), (\d+), (\d+), ([A-Z_0-9, ]+)\)[,;]$", source, re.M)
    if len(rows) != 29:
        raise RuntimeError("JewelleryItem differs from the 29 rows this corpus expects")
    out = []
    for name, level, _, component, send, items in rows:
        ids = [int(constants.get(i, i)) for i in items.split(", ")]
        out.append(dict(name=name, level=int(level), component=int(component), send=int(send), items=ids))
    return out


def buttons():
    source = text("handlers/iface/JewelleryInterface.java")
    first, second = source.split("switch (button - 3)")
    second = second.split("if (data == null)")[0]
    found = {int(b): n for b, n in re.findall(r"case (\d+):\s+data = JewelleryItem\.([A-Z_]+);", first)}
    found.update({int(b) + 3: n for b, n in re.findall(r"case (\d+):\s+data = JewelleryItem\.([A-Z_]+);", second)})
    if len(found) != 29:
        raise RuntimeError("JewelleryInterface differs from the 29 buttons this corpus expects")
    return {name: button for button, name in found.items()}


def furnaces():
    block = re.search(r"IDS = new int\[\]\{(.*?)\}", text("skill/crafting/JewelleryCraftPlugin.java"), re.S).group(1)
    ids = [int(n) for n in re.findall(r"Scenery\.[A-Z_]+_(\d+)", block)]
    if len(ids) != 13:
        raise RuntimeError("JewelleryCraftPlugin differs from the 13 furnaces this corpus expects")
    return ids


def mould(name):
    return RING if "RING" in name else NECKLACE if "NECKLACE" in name else AMULET if "AMULET" in name else BRACELET


def held(*pairs):
    return {str(id): amount for id, amount in pairs}


def button(opcode, id):
    return dict(op="button", opcode=opcode, button=id)


def enter(value):
    return dict(op="input", value=value)


def change(**changes):
    return dict(op="set", state=changes)


PULSE, STOP = dict(op="pulse"), dict(op="stop")


def census(out):
    kinds = {}
    for case in out:
        for op, result in zip(case["ops"], case["expected"]):
            key = op["op"] + (":throws" if "throws" in result else "")
            kinds[key] = kinds.get(key, 0) + 1
    return dict(sorted(kinds.items()))


def requests():
    rows, of, ids = table(), buttons(), furnaces()
    def state(**changes):
        base = dict(level=99, held={}, ring=False, renamed={})
        base.update(changes)
        return base

    def handle(with_=ids[0]):
        return dict(op="handle", used=BAR, **{"with": with_})

    gems = sorted({i for row in rows for i in row["items"]} - {BAR})
    everything = held((BAR, 3), *[(g, 3) for g in gems], *[(m, 1) for m in MOULDS])
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="tables"), dict(op="names")])]
    cases.append(dict(state=state(held=everything), ops=[handle(id) for id in ids + [1]]))

    # JewelleryCrafting.open over what is held, the level and the names.
    holdings = [
        {}, held((BAR, 1)), held((BAR, 1), (RING, 1)), held((BAR, 1), *[(m, 1) for m in MOULDS]), held(*[(m, 1) for m in MOULDS]),
        everything, held(*[(g, 1) for g in gems], *[(m, 1) for m in MOULDS]), held((BAR, 1), (AMULET, 1)),
        held((BAR, 1), (AMULET, 1), (1607, 1)), held((BAR, 1), (NECKLACE, 1), (BRACELET, 1), (1615, 1)),
        held((BAR, 1), *[(g, 1) for g in gems]),
    ]
    for holding in holdings:
        for level in [1, 8, 50, 99]:
            cases.append(dict(state=state(level=level, held=holding), ops=[handle()]))
    renamed = dict({"1683": "Dragonstone amulet", "13281": "Slayer thing", "1654": "Gold necklace ring", "11069": "Band"})
    missing = {"1645": None}
    for other in [renamed, missing]:
        for holding in [{}, everything, held((BAR, 1), (AMULET, 1))]:
            for level in [1, 99]:
                cases.append(dict(state=state(level=level, held=holding, renamed=other), ops=[handle()]))

    # Every button around the twenty-nine, with every opcode.
    for id in range(18, 95):
        for opcode in [ONE, FIVE, ALL, X, 0]:
            cases.append(dict(state=state(held=everything), ops=[button(opcode, id), enter(2)] + [PULSE] * 11 + [STOP]))
    for row in rows:
        id, level, items = of[row["name"]], row["level"], row["items"]
        full = held((mould(row["name"]), 1), *[(i, 2) for i in items])
        uneven = held((mould(row["name"]), 1), (items[0], 1), (BAR, 3))
        for at in [level, level - 1]:
            for holding in [full, held(*[(i, 2) for i in items]), held((mould(row["name"]), 1), (BAR, 2)),
                            held((mould(row["name"]), 1), (items[0], 2)), uneven, {}]:
                for opcode in [ONE, FIVE, ALL]:
                    for ring in [False, True] if row["name"] == "SLAYER_RING" else [False]:
                        cases.append(dict(state=state(level=at, held=holding, ring=ring), ops=[button(opcode, id)] + [PULSE] * 6))
        for other in [renamed, missing]:
            for at in [level, level - 1]:
                cases.append(dict(state=state(level=at, held=full, renamed=other), ops=[button(ONE, id), PULSE]))
    sapphire = rows[1]
    stocked = held((RING, 1), (sapphire["items"][0], 4), (BAR, 4))
    for value in [0, -1, 1, 3, 28, "2", "abc", ""]:
        cases.append(dict(state=state(held=stocked), ops=[button(X, of[sapphire["name"]]), enter(value)] + [PULSE] * 16 + [STOP, PULSE]))
    cases.append(dict(state=state(held=stocked), ops=[enter(3), PULSE, STOP]))
    # The world changes under a running pulse; the mould is never asked again.
    for changes in [dict(held={}), dict(level=1), dict(held=held((BAR, 4))), dict(held=held((sapphire["items"][0], 4))),
                    dict(held=held((sapphire["items"][0], 1), (BAR, 1)))]:
        cases.append(dict(state=state(held=stocked), ops=[button(FIVE, of[sapphire["name"]])] + [PULSE] * 6 + [change(**changes)] + [PULSE] * 16))
    # The level drops between the prompt and its answer, and between make and the pulse's start.
    cases.append(dict(state=state(held=stocked), ops=[button(X, of[sapphire["name"]]), change(level=1), enter(2), PULSE, STOP]))
    for n, case in enumerate(cases):
        case["id"] = n
    return cases


def names():
    """The declared name table as the ItemDefinition adapter's rows."""
    return ",\n        ".join('{"%d", "%s"}' % (row["send"], name) for row, name in zip(table(), NAMES))
