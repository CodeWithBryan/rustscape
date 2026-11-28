"""Designed herblore corpus. Table rows are read from the pinned sources, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main/content/global/skill/herblore"
DRUIDIC, ZOGRE, HEROES = "DRUIDIC_RITUAL", "ZOGRE_FLESH_EATERS", "HEROES_QUEST"
BREWMASTER = "cape_perks:brewmaster"
VIAL = 227
# Declared item-name inputs (the cache definitions are not loaded by the oracle); the tables add theirs below.
NAMES = {"227": "Vial of water", "5935": "Coconut milk"}


def text(name):
    return (MAIN / name).read_bytes().replace(b"\r", b"").decode("utf-8")


def display(constant):
    """A declared name for an Items constant: its words, the first capitalised."""
    words = re.fullmatch(r"([A-Z0-9_]+?)_(\d+)", constant).group(1).replace("_", " ").lower()
    return words[:1].upper() + words[1:]


def herbs():
    out = []
    for name, herb, _, level, product in re.findall(r"([A-Z_]+)\(new Item\((\d+)\), ([\d.]+), (\d+), new Item\((\d+)\)\)", text("Herbs.java")):
        out.append(dict(name=name, herb=int(herb), level=int(level), product=int(product)))
        NAMES[str(product)] = "Clean " + name.replace("_", " ").lower()
    if len(out) != 20:
        raise RuntimeError("Herbs differs from the 20 rows this corpus expects")
    return out


def item(token, known):
    token = token.strip()
    match = re.fullmatch(r"Item\(Items\.([A-Z0-9_]+?_(\d+))\)", token)
    if match:
        NAMES.setdefault(match.group(2), display(match.group(1)))
        return int(match.group(2))
    match = re.fullmatch(r"Herbs\.([A-Z_]+)\.product", token)
    if match:
        return known["herb:" + match.group(1)]
    match = re.fullmatch(r"(?:UnfinishedPotion\.)?([A-Z_0-9]+)\.potion", token)
    if match:
        return known[match.group(1)]
    if token == "GrindingItem.BIRDS_NEST.product":
        crushed = re.search(r"BIRDS_NEST\(.*?\}, new Item\(Items\.([A-Z_]+_(\d+))\)\)", text("GrindingItem.java"))
        NAMES.setdefault(crushed.group(2), display(crushed.group(1)))
        return int(crushed.group(2))
    raise RuntimeError(f"unread item {token}")


ARG = r"([^,]+(?:\([^)]*\))?)"


def tables():
    known = {"herb:" + h["name"]: h["product"] for h in herbs()}
    unfinished = []
    for name, base, ingredient, level, potion in re.findall(
            rf"^    ([A-Z_]+)\((Item\([^)]*\)), {ARG}, (\d+), (Item\([^)]*\))\)", text("UnfinishedPotion.kt"), re.M):
        row = dict(name=name, base=item(base, known), ingredient=item(ingredient, known), level=int(level), potion=item(potion, known))
        unfinished.append(row)
    if len(unfinished) != 21:
        raise RuntimeError("UnfinishedPotion differs from the 21 rows this corpus expects")
    known.update({row["name"]: row["potion"] for row in unfinished})
    source = text("FinishedPotion.kt")
    body = source[source.index("{", source.index("enum class")):source.index("fun canMake")]
    finished = []
    for match in re.finditer(rf"^    ([A-Z_0-9]+)\({ARG}, {ARG}, (\d+), ([\d.]+), (Item\([^)]*\))(\)?),?\s*$", body, re.M):
        name, base, ingredient, level, _, potion, closed = match.groups()
        quest = None
        if not closed:
            rest = re.sub(r"^\s*//.*$", "", body[match.end(6):], flags=re.M)
            quest = re.search(r"Quests\.([A-Z_]+)", re.search(r"\{ player -> ([^}]*) \}", rest).group(1)).group(1)
        row = dict(name=name, base=item(base, known), ingredient=item(ingredient, known), level=int(level),
                   potion=item(potion, known), quest=quest)
        finished.append(row)
        known[name] = row["potion"]
    if len(finished) != 54:
        raise RuntimeError("FinishedPotion differs from the 54 rows this corpus expects")
    return herbs(), unfinished, finished


def state(**changes):
    base = dict(level=99, held={}, quests={DRUIDIC: 100}, requirements=[], attributes={}, consumables={}, names={}, draws=[0])
    base.update(changes)
    return base


def held(*pairs):
    return {str(id): amount for id, amount in pairs}


def use(used, with_):
    return dict(op="use", used=used, **{"with": with_})


def clean(id, slot=3):
    return dict(op="clean", id=id, slot=slot)


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


def clean_cases(rows):
    out = []
    for herb in rows:
        for stage in [0, 99, 100, 101]:
            ops = []
            for level in sorted({1, herb["level"] - 1, herb["level"], 99}):
                ops += [change(level=level), clean(herb["herb"], herb["level"] % 28)]
            out.append(dict(state=state(quests={DRUIDIC: stage}), ops=ops))
    # Items that are no dirty herb, with and without the quest.
    strangers = [0, -1, VIAL, rows[0]["product"], 65535]
    out.append(dict(state=state(), ops=[clean(id) for id in strangers]))
    out.append(dict(state=state(quests={}), ops=[clean(id) for id in strangers]))
    # The product's name in the message: no name, other cases, "clean" elsewhere, spaces and control characters around it.
    product = str(rows[0]["product"])
    for name in [None, "CLEAN GUAM", "Guam clean", "cleanclean", "", "  Clean  guam  ", "\tClean guam\n", "Clean_guam", "Unclean guam", "CleAn Guam"]:
        out.append(dict(state=state(names={product: name}), ops=[clean(rows[0]["herb"], slot) for slot in [0, 27, -1]]))
    return out


def unfinished_cases(rows):
    out = []
    for row in rows:
        base, ingredient = row["base"], row["ingredient"]
        # One of either: the pulse at once, in both orders of use, at the level and under it.
        for amounts in [(1, 1), (1, 5), (5, 1)]:
            for level in [row["level"], row["level"] - 1]:
                for pair in [(base, ingredient), (ingredient, base)]:
                    out.append(dict(state=state(level=level, held=held((base, amounts[0]), (ingredient, amounts[1]))),
                                    ops=[use(*pair)] + pulses(3)))
        # Several of both: the dialogue, its amount and a run past its end.
        for amounts, amount in [((5, 5), 3), ((2, 2), 5), ((10, 12), 10), ((0, 0), 1), ((0, 3), 1), ((3, 0), 2)]:
            out.append(dict(state=state(held=held((base, amounts[0]), (ingredient, amounts[1]))),
                            ops=[use(base, ingredient), GET_ALL, create(amount)] + pulses(amount + 3)))
        # Without the quest: the dialogue opens, the pulse does not start.
        out.append(dict(state=state(quests={DRUIDIC: 99}, held=held((base, 4), (ingredient, 4))),
                        ops=[use(base, ingredient), create(2)] + pulses(2)))
    row = rows[0]
    base, ingredient = row["base"], row["ingredient"]
    stocked = held((base, 8), (ingredient, 8))
    # What a running pulse does when the world changes under it.
    for changes in [dict(held=held((base, 8), (ingredient, 0))), dict(held=held((base, 0), (ingredient, 8))), dict(level=row["level"] - 1),
                    dict(quests={}), dict(quests={DRUIDIC: 100}, held=held((base, 1), (ingredient, 1)))]:
        out.append(dict(state=state(held=stocked), ops=[use(base, ingredient), create(6), PULSE, PULSE, change(**changes), PULSE, PULSE, STOP]))
    # Amounts the dialogue would not offer, and operations without their object.
    for amount in [0, -1, 1, 28]:
        out.append(dict(state=state(held=stocked), ops=[use(base, ingredient), create(amount)] + pulses(4)))
    out.append(dict(state=state(held=stocked), ops=[PULSE, STOP, GET_ALL, create(1), use(base, ingredient), dict(op="getAll", index=1), create(1, 1)]))
    # The ingredient's name in the message.
    for name in [None, "CLEAN GUAM", "Guam", "clean_guam", "", "Clean  guam leaf", "cleanclean guam"]:
        out.append(dict(state=state(held=held((base, 1), (ingredient, 1)), names={str(ingredient): name}), ops=[use(base, ingredient)] + pulses(3)))
    return out


def finished_cases(rows):
    out = []
    every = sorted({row["quest"] for row in rows if row["quest"]})
    allowed = dict(quests={DRUIDIC: 100, ZOGRE: 100, HEROES: 100}, requirements=every)
    for row in rows:
        base, ingredient = row["base"], row["ingredient"]
        for amounts in [(1, 1), (1, 4), (4, 1)]:
            for level in [row["level"], row["level"] - 1]:
                out.append(dict(state=state(level=level, held=held((base, amounts[0]), (ingredient, amounts[1])), **allowed),
                                ops=[use(base, ingredient)] + pulses(3)))
        out.append(dict(state=state(held=held((base, 1), (ingredient, 1)), **allowed), ops=[use(ingredient, base)] + pulses(3)))
        for amounts, amount in [((5, 5), 3), ((2, 3), 4), ((0, 0), 1)]:
            out.append(dict(state=state(held=held((base, amounts[0]), (ingredient, amounts[1])), **allowed),
                            ops=[use(base, ingredient), GET_ALL, create(amount)] + pulses(amount + 3)))
        # The potion's own requirement unmet: only the Druidic Ritual is done.
        out.append(dict(state=state(held=held((base, 1), (ingredient, 1))), ops=[use(base, ingredient)] + pulses(2)))
        # The skillcape perk: the draw under, at and over its chance, with and without a consumable for the product.
        perk = {BREWMASTER: True}
        for draws in [[14], [15], [99, 0]]:
            for consumables in [{}, {str(row["potion"]): [row["potion"] + 1000, row["potion"]]}]:
                out.append(dict(state=state(held=held((base, 3), (ingredient, 3)), attributes=perk, consumables=consumables, draws=draws, **allowed),
                                ops=[use(base, ingredient), create(2)] + pulses(4)))
    row = rows[0]
    base, ingredient = row["base"], row["ingredient"]
    stocked = held((base, 6), (ingredient, 6))
    for attributes in [{BREWMASTER: False}, {BREWMASTER: "yes"}, {BREWMASTER: 1}, {"cape_perks:other": True}]:
        out.append(dict(state=state(held=stocked, attributes=attributes), ops=[use(base, ingredient), create(2)] + pulses(4)))
    for changes in [dict(held=held((base, 6), (ingredient, 0))), dict(held=held((base, 0), (ingredient, 6))), dict(level=row["level"] - 1),
                    dict(quests={DRUIDIC: 99})]:
        out.append(dict(state=state(held=stocked), ops=[use(base, ingredient), create(5), PULSE, PULSE, change(**changes), PULSE, PULSE, STOP]))
    for name in [None, "EYE OF NEWT", "", "Clean eye"]:
        out.append(dict(state=state(held=held((base, 1), (ingredient, 1)), names={str(ingredient): name}), ops=[use(base, ingredient)] + pulses(3)))
    return out


def requests():
    herb_rows, unfinished, finished = tables()
    every = sorted({row["quest"] for row in finished if row["quest"]})
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="tables")])]
    # canMake of every finished potion, by quest stage and by the requirements met.
    quests = [{}, {ZOGRE: 6}, {ZOGRE: 7}, {ZOGRE: 99}, {ZOGRE: 100}, {ZOGRE: 101}, {HEROES: 1}, {HEROES: -1}, {DRUIDIC: 100}]
    cases += [dict(state=state(quests=q), ops=[dict(op="canMake")]) for q in quests]
    cases += [dict(state=state(requirements=[quest]), ops=[dict(op="canMake")]) for quest in every]
    cases.append(dict(state=state(requirements=every), ops=[dict(op="canMake")]))
    cases += clean_cases(herb_rows)
    cases += unfinished_cases(unfinished)
    cases += finished_cases(finished)
    # Pairs no listener is registered for, and registered pairs that are no potion.
    first, second = unfinished[0], finished[0]
    pairs = [(VIAL, VIAL), (VIAL, second["ingredient"]), (first["potion"], first["ingredient"]), (second["ingredient"], second["ingredient"]),
             (first["potion"], finished[2]["ingredient"]), (finished[2]["ingredient"], first["potion"]), (5935, first["ingredient"]), (0, 0)]
    cases.append(dict(state=state(held=held((VIAL, 2), (first["potion"], 2))), ops=[use(*pair) for pair in pairs]))
    for n, case in enumerate(cases):
        case["id"] = n
    return cases
