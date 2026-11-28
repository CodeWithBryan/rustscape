"""Designed corpus for gem cutting, limestone, amulet stringing and silver stringing. Table rows are read from the pinned sources, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main/content/global/skill/crafting"
CHISEL, WOOL, LIMESTONE, ONYX_AMULET = 1755, 1759, 3211, 6579


def text(name):
    return (MAIN / name).read_bytes().replace(b"\r", b"").decode("utf-8")


def gems():
    rows = re.findall(r"^\t([A-Z_]+)\(new Item\((\d+)\), new Item\((\d+)\), (\d+), new Animation\(\d+\), [\d.]+\)", text("gem/Gems.java"), re.M)
    if len(rows) != 9:
        raise RuntimeError("Gems differs from the 9 rows this corpus expects")
    return [dict(name=name, uncut=int(uncut), gem=int(gem), level=int(level)) for name, uncut, gem, level in rows]


def amulets():
    """The allowed nodes of AmuletStringingPlugin with the level of the JewelleryItem of each."""
    allowed = [int(n) for n in re.search(r"super\(([\d, ]+)\);", text("AmuletStringingPlugin.java")).group(1).split(", ")]
    levels = {int(send): int(level) for level, send in re.findall(r"^\t\t[A-Z_]+\((\d+), \d+, \d+, (\d+),", text("jewellery/JewelleryCrafting.java"), re.M)}
    if len(allowed) != 7 or len(levels) != 29:
        raise RuntimeError("AmuletStringingPlugin or JewelleryItem differs from what this corpus expects")
    return [dict(amulet=id, level=levels[id]) for id in allowed]


def stringable():
    source = text("silver/SilverStringingListener.kt")
    ids = [int(n) for n in re.findall(r"Items\.UNSTRUNG_[A-Z]+_(\d+)", source)]
    if len(ids) != 2:
        raise RuntimeError("SilverStringingListener differs from the 2 products this corpus expects")
    return ids


def state(**changes):
    base = dict(level=99, held={})
    base.update(changes)
    return base


def held(*pairs):
    return {str(id): amount for id, amount in pairs}


def handle(plugin, used, with_):
    return dict(op="handle", plugin=plugin, used=used, **{"with": with_})


def use(used, with_):
    return dict(op="use", used=used, **{"with": with_})


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


def gem_cases(rows):
    out = []
    for row in rows:
        uncut, level = row["uncut"], row["level"]
        for pair in [(uncut, CHISEL), (CHISEL, uncut)]:
            # One uncut gem: create(0, 1) at once. Otherwise the dialogue.
            for have in [0, 1, 2, 5]:
                for chisels in [0, 1]:
                    for at in [level, level - 1]:
                        ops = [handle("gem", *pair), GET_ALL]
                        if have != 1:
                            ops.append(create(min(have, 3) or 1))
                        out.append(dict(state=state(level=at, held=held((uncut, have), (CHISEL, chisels))), ops=ops + pulses(4)))
        stocked = held((uncut, 6), (CHISEL, 1))
        for amount in [0, -1, 1, 28]:
            out.append(dict(state=state(held=stocked), ops=[handle("gem", uncut, CHISEL), create(amount, 4)] + pulses(3)))
        # The world changes under a running pulse.
        for changes in [dict(held=held((uncut, 6))), dict(held=held((CHISEL, 1))), dict(level=level - 1), dict(held=held((uncut, 1), (CHISEL, 1)))]:
            out.append(dict(state=state(held=stocked), ops=[handle("gem", uncut, CHISEL), create(5), PULSE, PULSE, change(**changes), PULSE, PULSE, STOP]))
    uncut = rows[0]["uncut"]
    stocked = held((uncut, 6), (CHISEL, 1))
    # Events with no uncut gem, a gem used on something that is no chisel, and operations without their object.
    out.append(dict(state=state(held=stocked), ops=[handle("gem", CHISEL, CHISEL), handle("gem", 1, 2), handle("gem", CHISEL, rows[0]["gem"]), GET_ALL, create(1)]))
    out.append(dict(state=state(held=stocked), ops=[handle("gem", uncut, 5), GET_ALL, create(2)] + pulses(3)))
    out.append(dict(state=state(held=stocked), ops=[PULSE, STOP, GET_ALL, create(1), handle("gem", uncut, CHISEL), dict(op="getAll", index=7), STOP, PULSE]))
    return out


def limestone_cases():
    out = []
    for have in [0, 1, 2, 3, 5]:
        for chisels in [0, 1]:
            for level in [1, 11, 12, 99]:
                for pair in [(LIMESTONE, CHISEL), (CHISEL, LIMESTONE), (5, 6)]:
                    out.append(dict(state=state(level=level, held=held((LIMESTONE, have), (CHISEL, chisels))),
                                    ops=[handle("limestone", *pair)] + pulses(2 * have + 3)))
    stocked = held((LIMESTONE, 6), (CHISEL, 1))
    for changes in [dict(held=held((LIMESTONE, 6))), dict(held=held((CHISEL, 1))), dict(level=11), dict(held=held((LIMESTONE, 1), (CHISEL, 1))),
                    dict(held=held((LIMESTONE, 40), (CHISEL, 1)))]:
        for before in [1, 2, 3]:
            out.append(dict(state=state(held=stocked), ops=[handle("limestone", LIMESTONE, CHISEL)] + [PULSE] * before + [change(**changes)] + pulses(5)))
    # Enough runs for the animation's fifth tick, and a second pulse run over the first.
    out.append(dict(state=state(held=held((LIMESTONE, 20), (CHISEL, 1))), ops=[handle("limestone", LIMESTONE, CHISEL)] + pulses(24)))
    out.append(dict(state=state(held=stocked), ops=[PULSE, STOP, handle("limestone", LIMESTONE, CHISEL), PULSE, handle("limestone", CHISEL, LIMESTONE)] + pulses(4)))
    return out


def amulet_cases(rows, gem_rows):
    out = []
    for row in rows:
        amulet, level = row["amulet"], row["level"]
        for pair in [(WOOL, amulet), (amulet, WOOL)]:
            for at in [level, level - 1, 99, 1]:
                for holding in [held((WOOL, 1), (amulet, 1)), held((WOOL, 2), (amulet, 3)), held((amulet, 1)), held((WOOL, 1)), {}]:
                    out.append(dict(state=state(level=at, held=holding), ops=[handle("amulet", *pair), handle("amulet", *pair)]))
    # Other nodes: an onyx amulet used on anything, anything used on a product of another kind, and no product at all.
    others = [(ONYX_AMULET, 5), (ONYX_AMULET, ONYX_AMULET), (WOOL, 1635), (WOOL, 13281), (WOOL, 11130), (WOOL, WOOL), (WOOL, gem_rows[0]["gem"]), (5, 6),
              (1673, 1675), (WOOL, 1692), (0, 0)]
    for level in [1, 99]:
        full = held((WOOL, 5), (ONYX_AMULET, 2), (1635, 1), (13281, 1), (11130, 1), (1673, 1), (1675, 1), (5, 1), (6, 1))
        out.append(dict(state=state(level=level, held=full), ops=[handle("amulet", *pair) for pair in others]))
    return out


def silver_cases(products):
    out = []
    for product in products:
        for holding in [held((WOOL, 1), (product, 1)), held((WOOL, 3), (product, 2)), held((product, 1)), held((WOOL, 1)), {}]:
            for level in [1, 99]:
                out.append(dict(state=state(level=level, held=holding), ops=[use(WOOL, product), use(product, WOOL), use(WOOL, product)]))
    out.append(dict(state=state(held=held((WOOL, 3), (2961, 1), (1716, 1))), ops=[use(WOOL, 2961), use(WOOL, 1716), use(WOOL, WOOL), use(0, 0), use(1714, 1720)]))
    return out


def requests():
    gem_rows = gems()
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="tables")])]
    cases += gem_cases(gem_rows)
    cases += limestone_cases()
    cases += amulet_cases(amulets(), gem_rows)
    cases += silver_cases(stringable())
    for n, case in enumerate(cases):
        case["id"] = n
    return cases
