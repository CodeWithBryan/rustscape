"""Designed fletching corpus. Table rows are read from the pinned sources, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main/content/global/skill/fletching"
# Declared item-name inputs (the cache definitions are not loaded by the oracle).
NAMES = {
    "52": "Arrow shaft", "50": "Shortbow (u)", "48": "Longbow (u)", "9440": "Wooden stock", "2864": "Ogre arrow shaft",
    "4825": "Unstrung comp bow", "54": "Oak shortbow (u)", "56": "Oak longbow (u)", "9442": "Oak stock",
    "60": "Willow shortbow (u)", "58": "Willow longbow (u)", "9444": "Willow stock", "64": "Maple shortbow (u)",
    "62": "Maple longbow (u)", "9448": "Maple stock", "68": "Yew shortbow (u)", "66": "Yew longbow (u)", "9452": "Yew stock",
    "72": "Magic shortbow (u)", "70": "Magic longbow (u)", "9446": "Teak stock", "9450": "Mahogany stock",
}
KNIFE, WOLF_BONES, BOW_STRING, CROSSBOW_STRING = 946, 2859, 1777, 9438
SHAFT, HEADLESS, OGRE_SHAFT = 52, 53, 2864


def text(path):
    return (MAIN / path).read_bytes().replace(b"\r", b"").decode("utf-8")


def item(token):
    return int(re.fullmatch(r"Items\.[A-Z0-9_]+?_(\d+)", token.strip()).group(1))


def rows(path):
    out = []
    for line in text(path).split("\n"):
        match = re.match(r"^\s*([A-Z][A-Z_0-9]*)\((.*)\)\s*[,;]\s*$", line)
        if match:
            out.append((match.group(1), [part.strip() for part in match.group(2).split(",")]))
    return out


def logs():
    return [dict(name=n, log=item(a[0]), finished=item(a[1]), level=int(a[3])) for n, a in rows("log/LogCraftInfo.kt")]


def bows():
    return [dict(name=n, crossbow=a[0] == "true", unstrung=item(a[1]), strung=item(a[2]), level=int(a[3]))
            for n, a in rows("stringing/StringableCraftInfo.kt")]


def arrows():
    return [dict(name=n, tip=item(a[0]), arrow=item(a[1]), level=int(a[2])) for n, a in rows("arrow/ArrowCraftInfo.kt")]


def feathers():
    return [item(token) for token in re.findall(r"Items\.[A-Z0-9_]+", text("Feathers.kt").split("const val")[0])]


def state(**changes):
    base = dict(level=99, clockReady=True, space=True, broads=True, taskDone=False, held={}, quests={},
                attributes={}, at=[3200, 3200, 0], draws=[])
    base.update(changes)
    return base


def use(used, with_):
    return dict(op="use", used=used, **{"with": with_})


def create(amount, index=0):
    return dict(op="create", amount=amount, index=index)


def script(stage):
    return dict(op="script", stage=stage)


def change(**changes):
    return dict(op="set", state=changes)


GET_ALL = dict(op="getAll", index=0)
# Inside each Seers' Village zone, on its edges, one tile outside and on another plane.
SEERS = [[2721, 3489, 0], [2724, 3493, 0], [2725, 3493, 0], [2727, 3487, 0], [2730, 3490, 0], [2726, 3488, 0],
         [2721, 3487, 0], [2730, 3493, 0], [2731, 3493, 0], [2720, 3490, 0], [2725, 3486, 0], [2725, 3494, 0],
         [2722, 3490, 1], [2728, 3488, 2], [2725, 3490, 3], [3200, 3200, 0]]


def log_cases():
    out = []
    crafts = logs()
    by_log = {}
    for craft in crafts:
        by_log.setdefault(craft["log"], []).append(craft)
    for log, group in by_log.items():
        for index, craft in enumerate(group):
            for level in sorted({max(1, craft["level"] - 1), craft["level"], 99}):
                for chompy, zogre, bones in [(1, 8, 1), (0, 7, 0), (1, 8, 0), (0, 8, 1)]:
                    if (chompy, zogre, bones) != (1, 8, 1) and not craft["name"].startswith("OGRE"):
                        continue
                    held = {str(log): 2, str(KNIFE): 1, str(WOLF_BONES): bones}
                    quests = dict(BIG_CHOMPY_BIRD_HUNTING=chompy, ZOGRE_FLESH_EATERS=zogre)
                    out.append(dict(state=state(level=level, held=held, quests=quests, draws=[0, 3, 1]), ops=[
                        use(KNIFE, log), GET_ALL, create(3, index), script(0), script(1), script(2)]))
        # The log used on the knife, and an index past the list.
        out.append(dict(state=state(held={str(log): 1}), ops=[use(log, KNIFE), create(1, len(group)), create(1, -1)]))
    first = crafts[0]
    for amount in [-1, 0, 1, 2, 5]:
        out.append(dict(state=state(held={str(first["log"]): 4}), ops=[
            use(KNIFE, first["log"]), create(amount), script(0), script(1), script(4), script(3)]))
    # The clock, a level lost between cuts, and bones lost after the create.
    out.append(dict(state=state(clockReady=False, held={str(first["log"]): 2}), ops=[
        use(KNIFE, first["log"]), create(2), script(0), change(clockReady=True), script(1), script(2)]))
    oak = next(c for c in crafts if c["name"] == "OAK_SHORTBOW")
    out.append(dict(state=state(level=oak["level"], held={str(oak["log"]): 3}), ops=[
        use(KNIFE, oak["log"]), create(3), script(0), change(level=oak["level"] - 1), script(1)]))
    comp = next(c for c in crafts if c["name"] == "OGRE_COMP_BOW")
    out.append(dict(state=state(held={str(comp["log"]): 2, str(WOLF_BONES): 1}, quests=dict(ZOGRE_FLESH_EATERS=8)), ops=[
        use(KNIFE, comp["log"]), create(2, 1), script(0), script(1), script(2)]))
    ogre = next(c for c in crafts if c["name"] == "OGRE_ARROW_SHAFT")
    for draw in range(4):
        out.append(dict(state=state(held={str(ogre["log"]): 1}, quests=dict(BIG_CHOMPY_BIRD_HUNTING=5), draws=[draw]), ops=[
            use(KNIFE, ogre["log"]), create(1), script(0)]))
    magic = next(c for c in crafts if c["name"] == "MAGIC_SHORTBOW")
    for at in SEERS:
        for done in [False, True]:
            out.append(dict(state=state(held={str(magic["log"]): 1}, at=at, taskDone=done), ops=[
                use(KNIFE, magic["log"]), create(1), script(0)]))
    # Names: none, empty, and the first letters aOrAn tells apart.
    stock = next(c for c in crafts if c["name"] == "TEAK_STOCK")
    for name in [None, "", "(u)", " Hat (u) ", "Helm", "h", "Urn", "ice (u)", "Elm", "Oar", "apple", "Yew"]:
        for level in [1, 99]:
            out.append(dict(state=state(level=level, held={str(stock["log"]): 1}, names={str(stock["finished"]): name}), ops=[
                use(KNIFE, stock["log"]), create(1), script(0)]))
    return out


def bow_cases():
    out = []
    all_bows = bows()
    for bow in all_bows:
        right = CROSSBOW_STRING if bow["crossbow"] else BOW_STRING
        wrong = BOW_STRING if bow["crossbow"] else CROSSBOW_STRING
        for level in sorted({max(1, bow["level"] - 1), bow["level"], 99}):
            for zogre in ([7, 8] if bow["name"] == "OGRE_COMP_BOW" else [0]):
                held = {str(bow["unstrung"]): 2, str(right): 3, str(wrong): 1}
                out.append(dict(state=state(level=level, held=held, quests=dict(ZOGRE_FLESH_EATERS=zogre)), ops=[
                    use(wrong, bow["unstrung"]), use(right, bow["unstrung"]), GET_ALL, create(5), script(0), script(1), script(2)]))
        out.append(dict(state=state(held={str(bow["unstrung"]): 1, str(right): 1}, quests=dict(ZOGRE_FLESH_EATERS=9)), ops=[
            use(bow["unstrung"], right), create(1), script(0)]))
    short = all_bows[0]
    # Nothing to string with, no bow, the clock, a level lost between bows.
    for held in [{str(short["unstrung"]): 1}, {str(BOW_STRING): 1}, {}]:
        out.append(dict(state=state(held=dict(held)), ops=[use(BOW_STRING, short["unstrung"]), create(2), script(0)]))
    out.append(dict(state=state(clockReady=False, level=short["level"], held={str(short["unstrung"]): 2, str(BOW_STRING): 2}), ops=[
        use(BOW_STRING, short["unstrung"]), create(2), script(0), change(clockReady=True), script(0),
        change(level=short["level"] - 1), script(1)]))
    magic = next(b for b in all_bows if b["name"] == "MAGIC_SHORTBOW")
    for at in SEERS:
        for fletched in [None, True, False]:
            attributes = {} if fletched is None else {"diary:seers:fletch-magic-short-bow": fletched}
            out.append(dict(state=state(held={str(magic["unstrung"]): 1, str(BOW_STRING): 1}, at=at, attributes=attributes), ops=[
                use(BOW_STRING, magic["unstrung"]), create(1), script(0)]))
    return out


def arrow_cases():
    out = []
    amounts = [0, 1, 2, 14, 15, 16, 40]
    for feather in feathers():
        for shafts in amounts:
            for held_feathers in ([0, 1, 15, 40] if feather != feathers()[0] else amounts):
                held = {str(SHAFT): shafts, str(feather): held_feathers, str(HEADLESS): 3}
                out.append(dict(state=state(level=1, held=held), ops=[
                    use(SHAFT, feather), GET_ALL, create(3), script(0), script(1), script(2)]))
        out.append(dict(state=state(space=False, held={str(SHAFT): 5, str(feather): 5}), ops=[use(feather, SHAFT), create(1)]))
        for shafts in [0, 1, 5, 6, 7, 10]:
            for held_feathers in [0, 1, 3, 4, 7, 8, 23, 24, 25, 100]:
                for level in [4, 5]:
                    held = {str(OGRE_SHAFT): shafts, str(feather): held_feathers}
                    out.append(dict(state=state(level=level, held=held), ops=[
                        use(OGRE_SHAFT, feather), GET_ALL, create(2), script(0), script(1)]))
        out.append(dict(state=state(space=False, held={str(OGRE_SHAFT): 5, str(feather): 20}), ops=[use(feather, OGRE_SHAFT), create(1)]))
    for arrow in arrows():
        for level in sorted({max(1, arrow["level"] - 1), arrow["level"], 99}):
            for broads in [True, False]:
                for space in [True, False]:
                    for headless, tips in [(40, 40), (15, 3), (1, 20), (0, 5), (5, 0), (16, 31)]:
                        if not (broads and space) and (headless, tips) != (40, 40):
                            continue
                        held = {str(HEADLESS): headless, str(arrow["tip"]): tips}
                        out.append(dict(state=state(level=level, broads=broads, space=space, held=held), ops=[
                            use(HEADLESS, arrow["tip"]), GET_ALL, create(3), script(0), script(1), script(2)]))
        out.append(dict(state=state(clockReady=False, level=arrow["level"], held={str(HEADLESS): 30, str(arrow["tip"]): 30}), ops=[
            use(arrow["tip"], HEADLESS), create(2), script(0), change(clockReady=True), script(0),
            change(level=max(0, arrow["level"] - 1)), script(1)]))
    return out


def block(path):
    """Enum rows that may span lines: NAME(arguments) up to the companion object."""
    body = text(path)
    body = body[body.index("{", body.index("enum class")) + 1:body.index("companion object")]
    out = []
    for match in re.finditer(r"([A-Z][A-Z_0-9]*)\(((?:[^()]|\([^()]*\))*)\)\s*[,;]", body):
        out.append((match.group(1), [part.strip() for part in match.group(2).split(",") if part.strip()]))
    return out


def bolts():
    return [dict(name=n, unfinished=item(a[0]), finished=item(a[1]), level=int(a[2])) for n, a in block("bolts/BoltCraftInfo.kt")]


def darts():
    return [dict(name=n, tip=item(a[0]), dart=item(a[1]), level=int(a[2])) for n, a in block("darts/DartCraftInfo.kt")]


def crossbows():
    return [dict(name=n, stock=item(a[0]), limb=item(a[1]), unstrung=item(a[2]), level=int(a[3]))
            for n, a in block("crossbow/UnfinishedCrossbowCraftInfo.kt")]


def gems():
    return [dict(name=n, untipped=item(a[0]), gem=item(a[1]), tip=item(a[3]), tipped=item(a[4]), level=int(a[5]))
            for n, a in block("gem/GemBoltsCraftInfo.kt")]


def run(n, stage):
    return dict(op="script", stage=stage, n=n)


CHISEL, MITHRIL_BOLTS, GRAPPLE_TIP, ROPE, GRAPPLE = 1755, 9142, 9416, 954, 9418
PAIRS = [(40, 40), (10, 3), (1, 20), (0, 5), (5, 0), (11, 21), (2, 2)]


def ammo_cases():
    out = []
    plumes = feathers()
    for number, bolt in enumerate(bolts()):
        feather = plumes[number % len(plumes)]
        for level in sorted({max(1, bolt["level"] - 1), bolt["level"], 99}):
            for broads in [True, False]:
                for space in [True, False]:
                    for first, second in PAIRS:
                        if not (broads and space) and (first, second) != (40, 40):
                            continue
                        held = {str(feather): first, str(bolt["unfinished"]): second}
                        out.append(dict(state=state(level=level, broads=broads, space=space, held=held), ops=[
                            use(feather, bolt["unfinished"]), GET_ALL, create(3), script(0), script(1), script(2)]))
        out.append(dict(state=state(clockReady=False, level=bolt["level"], held={str(feather): 30, str(bolt["unfinished"]): 30}), ops=[
            use(bolt["unfinished"], feather), create(2), script(0), change(clockReady=True), script(0),
            change(level=bolt["level"] - 1), script(1)]))
    for number, dart in enumerate(darts()):
        feather = plumes[number % len(plumes)]
        for level in sorted({max(1, dart["level"] - 1), dart["level"], 99}):
            for trap in [0, 99, 100, 101]:
                for space in [True, False]:
                    for first, second in PAIRS:
                        if not (trap == 100 and space) and (first, second) != (40, 40):
                            continue
                        held = {str(feather): first, str(dart["tip"]): second}
                        out.append(dict(state=state(level=level, space=space, held=held, quests=dict(THE_TOURIST_TRAP=trap)), ops=[
                            use(feather, dart["tip"]), GET_ALL, create(3), script(0), script(1), script(2)]))
        out.append(dict(state=state(clockReady=False, level=dart["level"], held={str(feather): 30, str(dart["tip"]): 30},
                                    quests=dict(THE_TOURIST_TRAP=100)), ops=[
            use(dart["tip"], feather), create(2), script(0), change(clockReady=True), script(0),
            change(level=max(0, dart["level"] - 1)), script(1)]))
    bows = crossbows()
    for number, bow in enumerate(bows):
        wrong = bows[(number + 1) % len(bows)]["limb"]
        for level in sorted({max(1, bow["level"] - 1), bow["level"], 99}):
            for space in [True, False]:
                for stocks, limbs in [(2, 2), (1, 0), (0, 1), (3, 1)]:
                    if not space and (stocks, limbs) != (2, 2):
                        continue
                    held = {str(bow["stock"]): stocks, str(bow["limb"]): limbs, str(wrong): 1}
                    out.append(dict(state=state(level=level, space=space, held=held), ops=[
                        use(wrong, bow["stock"]), use(bow["limb"], bow["stock"]), GET_ALL, create(3), script(0), script(1), script(2)]))
        out.append(dict(state=state(clockReady=False, level=bow["level"], held={str(bow["stock"]): 2, str(bow["limb"]): 2}), ops=[
            use(bow["stock"], bow["limb"]), create(2), script(0), change(clockReady=True), script(0),
            change(level=bow["level"] - 1), script(1)]))
    all_gems = gems()
    for gem in all_gems:
        for level in sorted({max(1, gem["level"] - 1), gem["level"], 99}):
            for held_gems in [0, 1, 2, 5]:
                # The start script queues the animation loop (1) and the craft loop (2).
                out.append(dict(state=state(level=level, held={str(gem["gem"]): held_gems, str(CHISEL): 1}), ops=[
                    use(CHISEL, gem["gem"]), GET_ALL, create(2), run(0, 0), run(1, 0), run(2, 0), run(1, 1), run(2, 1),
                    run(1, 2), run(2, 2), run(1, 3)]))
        out.append(dict(state=state(clockReady=False, level=gem["level"], held={str(gem["gem"]): 3}), ops=[
            use(gem["gem"], CHISEL), create(3), run(0, 0), run(2, 0), run(1, 0), change(clockReady=True), run(2, 0),
            change(level=gem["level"] - 1), run(1, 1), run(2, 1), run(1, 2)]))
        for level in sorted({max(1, gem["level"] - 1), gem["level"], 99}):
            for space in [True, False]:
                for first, second in PAIRS:
                    if not space and (first, second) != (40, 40):
                        continue
                    held = {str(gem["untipped"]): first, str(gem["tip"]): second}
                    out.append(dict(state=state(level=level, space=space, held=held), ops=[
                        use(gem["untipped"], gem["tip"]), GET_ALL, create(3), script(0), script(1), script(2)]))
        # The tip on its bolts, and every other bolt with this tip.
        ops = [use(gem["tip"], gem["untipped"]), create(1), script(0)]
        ops += [use(other, gem["tip"]) for other in sorted({g["untipped"] for g in all_gems} - {gem["untipped"]})]
        out.append(dict(state=state(held={str(gem["untipped"]): 4, str(gem["tip"]): 4}), ops=ops))
    for used, with_ in [(MITHRIL_BOLTS, GRAPPLE_TIP), (GRAPPLE_TIP, MITHRIL_BOLTS), (ROPE, GRAPPLE), (GRAPPLE, ROPE)]:
        for level in [58, 59]:
            for first, second in [(1, 1), (0, 1), (1, 0), (5, 5)]:
                for ready in [True, False]:
                    held = {str(used): first, str(with_): second}
                    out.append(dict(state=state(level=level, clockReady=ready, held=held), ops=[
                        use(used, with_), script(0), change(clockReady=True), script(0), script(1)]))
    return out


def requests():
    cases = [dict(state=state(), ops=[dict(op="registrations")])]
    cases += log_cases() + bow_cases() + arrow_cases()
    # Pairs no listener registers.
    cases.append(dict(state=state(), ops=[use(KNIFE, BOW_STRING), use(SHAFT, HEADLESS), use(1, 2)]))
    cases += ammo_cases()
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
