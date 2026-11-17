"""RandomFunction methods, the generators behind them, and SkillingTool selection."""
import itertools

MIN, MAX = -2147483648, 2147483647
# (baseLow, baseHigh, tierModLow, tierModHigh) inputs for the success-chance sweep.
CHANCE = [(64, 200, 32, 100), (32, 100, 16, 50), (16, 50, 8, 25), (15, 46, 8, 23.5), (8, 25, 4, 12.5), (18, 26, 10, 14),
          (6, 30, 3, 13.5), (2, 6, 1, 3), (4, 12.5, 2, 6.25), (255, 255, 0, 0)]
ORDINALS = [0, 1, 2, 3, 4, 5, 6, 7, 14, 16, 17, 18, 19, 20]
LEVELS = list(range(-1, 102)) + [MIN, MIN + 1, -99, 127, 198, 255, 1000, MAX - 1, MAX]


def cases(h):
    out = []

    def add(calls, **seeds):
        out.append(dict(kind="random", calls=calls, **seeds))

    bounds = [MIN, -1, 0, 1, 2, 3, 4, 5, 7, 8, 100, 125, 188, 256, 282, 1 << 30, (1 << 30) + 1, MAX]
    for seed in [0, 1, 2, 530, -1, 1 << 40]:
        add([["random", [n]] for n in bounds], sharedSeed=seed)
        add([["randomize", [n]] for n in bounds], sharedSeed=seed)
        add([["roll", [n]] for n in bounds], sharedSeed=seed)
        add([["randomRange", [a, b]] for a, b in [(0, 5), (5, 0), (0, 0), (3, 3), (-5, 5), (1, 2), (2, 1), (MIN, MAX), (MAX, MIN),
                                                   (0, MIN), (MIN, 0), (MAX, MAX), (-1, MAX), (1, 1 << 30)]], sharedSeed=seed)
        add([["getRandom", [n]] for n in [5, 0, -1, 1, MAX, MIN, 99]], mathSeed=seed)
    for n in [2, 4, 5, 8, 100, 125, 188, 282]:
        for value in sorted(v for v in {0, 1, 2, n // 2, n - 1} if v < n):
            add([["roll", [n]], ["random", [n]], ["roll", [n]]], sharedSeed=h.shared_seed((n, value)))
    for target in range(6):
        seed = 0
        while int(h.JavaRandom(seed).next_double() * 6) != target:
            seed += 1
        add([["getRandom", [5]], ["getRandom", [5]]], mathSeed=seed)
    for low, high, mod_low, mod_high in CHANCE:
        for ordinal in ORDINALS:
            for halve in ([1, 2] if ordinal == 3 else [1]):
                lo, hi = low + ordinal * (mod_low / halve), high + ordinal * (mod_high / halve)
                add([["chance", [h.bits(float(lo)), h.bits(float(hi)), level]] for level in LEVELS])
    for lo, hi in [(float("nan"), 1.0), (1.0, float("nan")), (float("inf"), 1.0), (-float("inf"), 5.0), (1e300, 1e300), (-1e300, 2.0), (-0.0, -0.0), (0.5, 0.5)]:
        add([["chance", [h.bits(lo), h.bits(hi), level]] for level in [MIN, -1, 0, 1, 50, 98, 99, 100, MAX]])
    units = [0, 1, 2, (1 << 52), (1 << 53) - 2, (1 << 53) - 1, 65 << 45, (65 << 45) - 1, 0x1234567890ABC]
    for unit in units:
        add([["randomDouble", [h.bits(m)]] for m in [100.0, 1.0, 0.5, 5e-324, 1e-300, 1e308, 3.0]], tlrSeed=h.tlr_seed_for_unit(unit))
    for seed in ["0", "1", "-1", "4", "9223372036854775807", "-9223372036854775808", "530"]:
        add([["randomDouble", [h.bits(100.0)]] for _ in range(8)], tlrSeed=seed)
    # Gem rock table: the shared list stays shuffled between rolls.
    for shared, shuffle in itertools.product([0, 1, 2, 3, 530, 99991], [0, 1, 2, 7, 530]):
        add([["gems", []] for _ in range(8)], sharedSeed=shared, shuffleSeed=shuffle)
    for value in [0, 1, 3, 4, 5, 6, 8, 9, 10, 14, 15, 16, 29, 30, 31, 59, 60, 61, 126, 127]:
        for shuffle in [0, 5]:
            add([["gems", []], ["gems", []]], sharedSeed=h.shared_seed((128, value)), shuffleSeed=shuffle)

    # ---- SkillingTool selection -------------------------------------------
    tools = [(name, int(a[0].rsplit("_", 1)[-1]), int(a[1])) for name, a in h.rows("content/data/skill/SkillingTool.java", "private final int id")]

    def select(method, levels=None, equipment=None, inventory=None, **kw):
        out.append(dict(kind="selectTool", method=method, levels={str(k): v for k, v in (levels or {}).items()},
                        equipment={str(k): v for k, v in (equipment or {}).items()}, inventory=inventory or [], **kw))

    for name, item, level in tools:
        for skill in [h.WOODCUTTING, h.MINING, 10, 21]:
            for static in [level - 1, level, level + 1]:
                for held in ["none", "weapon", "inventory", "both", "shield"]:
                    select("checkTool", {skill: [1, static]}, {3: item} if held in ("weapon", "both") else {5: item} if held == "shield" else {},
                           [[item, 1]] if held in ("inventory", "both") else [], skill=skill, tool=name)
    everything = [[item, 1] for _, item, _ in tools]
    for method, skill in [("getHatchet", h.WOODCUTTING), ("getPickaxe", h.MINING), ("getHarpoon", 10), ("getButterflyNet", 21)]:
        for static in [0, 1, 5, 6, 20, 21, 31, 40, 41, 60, 61, 80]:
            for fire in [91, 92]:
                select(method, {skill: [1, static], h.FIREMAKING: [fire, 1]}, {}, everything[:28])
                select(method, {skill: [1, static], h.FIREMAKING: [fire, 1]}, {}, everything[8:])
        for name, item, level in tools:
            for fire in [91, 92]:
                select(method, {skill: [1, 99], h.FIREMAKING: [fire, 99]}, {3: item}, [])
                select(method, {skill: [1, 99], h.FIREMAKING: [fire, 99]}, {}, [[item, 1], [13661, 1]])
        select(method, {skill: [99, 99]}, {}, [])
        select(method, {skill: [99, 0]}, {3: 13661}, everything[:28])
    for skill in [h.MINING, h.WOODCUTTING, 10, 21, 0, -1, 24, 11]:
        select("getToolForSkill", {8: [1, 99], 14: [1, 99], 10: [1, 99], 21: [1, 99], h.FIREMAKING: [99, 99]}, {3: 13661}, everything[:28], skill=skill)
        select("getToolForSkill", {}, {}, [], skill=skill)
    return out
