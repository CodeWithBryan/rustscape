"""Designed corpus for spinning and silver casting. Table rows are read from the pinned sources, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main/content/global"
BAR, SPIN, SILVER, HEADBAND, SEERS = 2355, 459, 438, 14631, 10806
ONE, FIVE, ALL, X = 155, 196, 124, 199


def text(name):
    return (MAIN / name).read_bytes().replace(b"\r", b"").decode("utf-8")


def spins():
    line = re.search(r"^\tWOOL\(.*;$", text("skill/crafting/spinning/SpinningItem.java"), re.M).group()
    rows = re.findall(r"[A-Z0-9_]+\((\d+), (\d+), (\d+), (\d+), [\d.]+\)", line)
    if len(rows) != 11:
        raise RuntimeError("SpinningItem differs from the 11 rows this corpus expects")
    return [dict(button=int(b), need=int(n), product=int(p), level=int(l)) for b, n, p, l in rows]


def silvers():
    source = text("skill/crafting/silver/SilverProduct.kt")
    buttons = dict(re.findall(r"^private const val (BUTTON_[A-Z_]+) = (\d+)$", source, re.M))
    rows = re.findall(r"^    [A-Z_]+\((BUTTON_[A-Z_]+), Items\.[A-Z_]+_(\d+), Items\.[A-Z_]+_(\d+), (\d+), (\d+), [\d.]+, ", source, re.M)
    if len(rows) != 8 or len(buttons) != 9:
        raise RuntimeError("SilverProduct differs from the 8 rows and 9 buttons this corpus expects")
    return [dict(button=int(buttons[b]), mould=int(m), product=int(p), amount=int(a), level=int(l)) for b, m, p, a, l in rows]


def wheels():
    ids = [int(n) for n in re.findall(r"SceneryDefinition\.forId\((\d+)\)", text("handlers/scenery/SpinSpinningWheelPlugin.java"))]
    if len(ids) != 10:
        raise RuntimeError("SpinSpinningWheelPlugin differs from the 10 wheels this corpus expects")
    return ids


def furnaces():
    block = re.search(r"FURNACES = intArrayOf\((.*?)\)", text("skill/crafting/silver/SilverCraftingListener.kt"), re.S).group(1)
    ids = [int(n) for n in re.findall(r"Scenery\.[A-Z_]+_(\d+)", block)]
    if len(ids) != 23:
        raise RuntimeError("SilverCraftingListener differs from the 23 furnaces this corpus expects")
    return ids


def state(**changes):
    base = dict(level=99, held={}, seersHard=False, near=False, hat=None, region=0, taskDone=False, bowstrings=None, full=False)
    base.update(changes)
    return base


def held(*pairs):
    return {str(id): amount for id, amount in pairs}


def use(with_, used=BAR, x=3272, y=3185):
    return dict(op="use", used=used, x=x, y=y, **{"with": with_})


def spin(opcode, id):
    return dict(op="spinButton", opcode=opcode, button=id)


def button(opcode, id, component=SILVER):
    return dict(op="button", component=component, opcode=opcode, button=id)


def enter(value):
    return dict(op="input", value=value)


def change(**changes):
    return dict(op="set", state=changes)


PULSE, STOP, OPEN = dict(op="pulse"), dict(op="stop"), dict(op="open", component=SILVER)
VALUES = [0, -1, 1, 3, 28, "2", "0", "-1", "+3", "abc", "", "2147483648", " 2", "2.0"]


def census(out):
    kinds = {}
    for case in out:
        for op, result in zip(case["ops"], case["expected"]):
            key = op["op"] + (":throws" if "throws" in result else "")
            kinds[key] = kinds.get(key, 0) + 1
    return dict(sorted(kinds.items()))


def spin_cases(rows, ids):
    out = [dict(state=state(), ops=[dict(op="wheel", id=id) for id in ids + [1, 0]])]
    everything = held(*sorted({(row["need"], 3) for row in rows}))
    # Every button around the eleven, with every opcode.
    for id in range(15, 38):
        for opcode in [ONE, FIVE, ALL, X, 0]:
            out.append(dict(state=state(held=everything), ops=[spin(opcode, id), enter(2)] + [PULSE] * 9 + [STOP]))
    for row in rows:
        id, need, level = row["button"], row["need"], row["level"]
        for at in [level, level - 1]:
            for holding in [held((need, 2)), {}]:
                for opcode in [ONE, ALL, 7]:
                    out.append(dict(state=state(level=at, held=holding), ops=[spin(opcode, id)] + [PULSE] * 9))
    wool, flax = rows[0], rows[1]
    for value in VALUES:
        out.append(dict(state=state(held=held((flax["need"], 5))), ops=[spin(X, flax["button"]), enter(value)] + [PULSE] * 13 + [STOP, PULSE]))
    out.append(dict(state=state(held=held((flax["need"], 5))), ops=[enter(3), PULSE, STOP]))
    # The Seers' headband at the Seers' wheel halves the ticks of a spin.
    for hard in [False, True]:
        for near in [False, True]:
            for hat in [None, HEADBAND, 1]:
                out.append(dict(state=state(held=held((wool["need"], 4)), seersHard=hard, near=near, hat=hat),
                                ops=[spin(ALL, wool["button"])] + [PULSE] * 17))
    # The bowstring task counts every spin in the region, of any item.
    for row in [wool, flax]:
        for region in [SEERS, SEERS + 1]:
            for done in [False, True]:
                for spun in [None, 0, 2, 3, 4, 5, 9]:
                    out.append(dict(state=state(held=held((row["need"], 3)), region=region, taskDone=done, bowstrings=spun),
                                    ops=[spin(ALL, row["button"])] + [PULSE] * 13))
    # The world changes under a running pulse.
    stocked = held((flax["need"], 4))
    for changes in [dict(held={}), dict(level=1), dict(held=held((flax["need"], 1))), dict(seersHard=True, near=True, hat=HEADBAND),
                    dict(region=SEERS, bowstrings=4)]:
        out.append(dict(state=state(held=stocked), ops=[spin(FIVE, flax["button"])] + [PULSE] * 5 + [change(**changes)] + [PULSE] * 12))
    # A pulse that never started is still the kept pulse.
    out.append(dict(state=state(level=1, held=stocked), ops=[spin(ONE, flax["button"]), PULSE, change(level=99), PULSE, PULSE, PULSE, PULSE, STOP]))
    return out


def silver_cases(rows, ids):
    out = []
    for furnace in ids + [1, 2781]:
        out.append(dict(state=state(held=held((BAR, 1))), ops=[use(furnace), OPEN]))
    out.append(dict(state=state(), ops=[use(ids[0], used=2357), use(ids[0], used=ids[0]), dict(op="open", component=SPIN),
                                        button(ONE, 16, SPIN), PULSE, STOP, enter(1)]))
    everything = held((BAR, 3), *[(row["mould"], 1) for row in rows])
    # Every button around the eight, with every opcode.
    for id in range(14, 76):
        for opcode in [ONE, FIVE, ALL, X, 0]:
            out.append(dict(state=state(held=everything), ops=[use(ids[0]), button(opcode, id), enter(2)] + [PULSE] * 4))
    for row in rows:
        id, mould, level = row["button"], row["mould"], row["level"]
        for at in [level, level - 1]:
            for holding in [held((BAR, 2), (mould, 1)), held((mould, 1)), held((BAR, 2)), {}]:
                for opcode in [ONE, ALL]:
                    for first in [[use(ids[7], x=3272, y=3185)], []]:
                        out.append(dict(state=state(level=at, held=holding), ops=first + [button(opcode, id)] + [PULSE] * 4))
    holy, bolts = rows[0], rows[-1]
    for row in [holy, bolts]:
        stocked = held((BAR, 6), (row["mould"], 1))
        for value in VALUES:
            out.append(dict(state=state(held=stocked), ops=[use(ids[3], x=1, y=2), button(X, row["button"]), enter(value)] + [PULSE] * 5))
        # A full inventory: the bar is taken and the pulse ends.
        out.append(dict(state=state(held=stocked, full=True), ops=[button(FIVE, row["button"]), PULSE, change(full=False), PULSE, PULSE]))
        for changes in [dict(held=held((BAR, 6))), dict(held=held((row["mould"], 1))), dict(level=1), dict(full=True), dict(held={})]:
            out.append(dict(state=state(held=stocked), ops=[use(ids[1]), button(FIVE, row["button"]), PULSE, change(**changes), PULSE, PULSE, PULSE]))
        # The furnace is read when the pulse is made, not when it runs.
        out.append(dict(state=state(held=stocked), ops=[use(ids[1], x=5, y=6), button(ONE, row["button"]), use(ids[2], x=7, y=8), PULSE,
                                                         button(ALL, row["button"]), PULSE, PULSE, STOP, PULSE]))
    out.append(dict(state=state(held=everything), ops=[enter(3), PULSE, STOP, button(X, 16), button(ONE, 23), enter("4"), PULSE, PULSE]))
    return out


def requests():
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="tables")])]
    cases += spin_cases(spins(), wheels())
    cases += silver_cases(silvers(), furnaces())
    for n, case in enumerate(cases):
        case["id"] = n
    return cases
