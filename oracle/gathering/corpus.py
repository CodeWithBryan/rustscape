"""Designed gathering corpus. Row counts are discovered from the pinned sources, not typed here."""
import functools
import pathlib
import re
import struct

ROOT = pathlib.Path(__file__).resolve().parents[2]
MAIN = ROOT / "reference/server/Server/src/main"
ROW = re.compile(r"^\s*([A-Z][A-Z_0-9]*)\((.*)\)\s*[,;]?\s*(?://.*)?$")
MASK = (1 << 64) - 1
GAMMA = 0x9E3779B97F4A7C15
NAMES = {
    "1": "Coins", "1511": "Logs", "1521": "Oak logs", "1519": "Willow logs", "6333": "Teak logs", "1517": "Maple logs", "3239": "Bark",
    "6332": "Mahogany logs", "10810": "Arctic pine logs", "12581": "Eucalyptus logs", "1515": "Yew logs", "1513": "Magic logs",
    "771": "Dramen branch", "3692": "Strange looking branch", "2862": "Achey tree logs", "5070": "Bird's nest",
    "436": "Copper ore", "438": "Tin ore", "434": "Clay", "1761": "Soft clay", "3211": "Limestone", "668": "Blurite ore", "440": "Iron ore",
    "442": "Silver ore", "453": "Coal", "444": "Gold ore", "446": "'perfect' gold ore", "447": "Mithril ore", "449": "Adamantite ore",
    "451": "Runite ore", "1625": "Uncut opal", "1627": "Uncut jade", "1629": "Uncut red topaz", "1623": "Uncut sapphire",
    "1621": "Uncut emerald", "1619": "Uncut ruby", "1617": "Uncut diamond", "1436": "Rune essence", "7936": "Pure essence",
    "6971": "Sandstone (1kg)", "6973": "Sandstone (2kg)", "6975": "Sandstone (5kg)", "6977": "Sandstone (10kg)",
    "6979": "Granite (500g)", "6981": "Granite (2kg)", "6983": "Granite (5kg)", "12630": "Rubium", "4703": "Magic stone",
    "2572": "Ring of wealth", "1635": "Gold ring", "14652": "Ring of the Star Sprite", "1704": "Amulet of glory", "1712": "Amulet of glory(4)", "11074": "Bracelet of clay",
}
WOODCUTTING, MINING, FIREMAKING = 8, 14, 11


def rows(path, stop):
    text = (MAIN / path).read_bytes().replace(b"\r", b"").decode("utf-8")
    body = text[text.index("{") + 1 : text.index(stop)]
    out = []
    for line in body.split("\n"):
        match = ROW.match(line)
        if match:
            out.append((match.group(1), [part.strip() for part in match.group(2).split(",")]))
    return out


def bits(value):
    return "d:%016x" % struct.unpack(">Q", struct.pack(">d", value))[0]


# ---- JDK generators, used only to design seeds with chosen draws ----------
class JavaRandom:
    def __init__(self, seed):
        self.state = (seed ^ 0x5DEECE66D) & ((1 << 48) - 1)

    def bits(self, n):
        self.state = (self.state * 0x5DEECE66D + 0xB) & ((1 << 48) - 1)
        return self.state >> (48 - n)

    def next_int(self, bound):
        if bound & (bound - 1) == 0:
            return (bound * self.bits(31)) >> 31
        while True:
            b = self.bits(31)
            v = b % bound
            if b - v + (bound - 1) < (1 << 31):
                return v

    def next_double(self):
        return ((self.bits(26) << 27) + self.bits(27)) / float(1 << 53)


@functools.lru_cache(maxsize=None)
def shared_seed(*draws):
    """Smallest seed whose first nextInt(bound) results are the requested (bound, value) pairs."""
    assert all(0 <= value < bound for bound, value in draws), draws
    seed = 0
    while True:
        r = JavaRandom(seed)
        if all(r.next_int(bound) == value for bound, value in draws):
            return seed
        seed += 1


def _unmix(z):
    z ^= z >> 33
    z = z * pow(0xC4CEB9FE1A85EC53, -1, 1 << 64) & MASK
    z ^= z >> 33
    z = z * pow(0xFF51AFD7ED558CCD, -1, 1 << 64) & MASK
    z ^= z >> 33
    return z


def signed(value):
    value &= MASK
    return value - (1 << 64) if value >> 63 else value


def tlr_seed_for_unit(k):
    """Thread-local seed whose next nextDouble(0, m) is (k * 2^-53) * m, for 0 <= k < 2^53."""
    return str(signed(_unmix(k << 11) - GAMMA))


def tlr_seed_for_ratio(numerator, denominator=256):
    """Seed pair around an exact success chance numerator/denominator*100: (equal, one unit below)."""
    k = numerator * (1 << 53) // denominator
    return tlr_seed_for_unit(k), tlr_seed_for_unit(k - 1)


def scenario(subject, node, ops, **world):
    return dict(kind="scenario", subject=subject, node=node, ops=ops, **world)


def op(name, **kw):
    return dict(op=name, **kw)


def requests():
    from importlib import util
    cases = [dict(kind="counts")]
    spec = util.spec_from_file_location("tables", pathlib.Path(__file__).with_name("cases_tables.py"))
    tables = util.module_from_spec(spec); spec.loader.exec_module(tables)
    cases += tables.cases(rows)
    # Order is part of the fixture: new modules are appended, never inserted.
    for name in ["cases_random.py", "cases_woodcutting.py", "cases_mining.py", "cases_fishing.py"]:
        path = pathlib.Path(__file__).with_name(name)
        if path.is_file():
            spec = util.spec_from_file_location(path.stem, path)
            module = util.module_from_spec(spec); spec.loader.exec_module(module)
            cases += module.cases(Helpers)
    return cases


class Helpers:
    """Namespace handed to the case modules."""
    rows = staticmethod(rows); bits = staticmethod(bits); shared_seed = staticmethod(shared_seed)
    tlr_seed_for_unit = staticmethod(tlr_seed_for_unit); tlr_seed_for_ratio = staticmethod(tlr_seed_for_ratio)
    scenario = staticmethod(scenario); op = staticmethod(op); JavaRandom = JavaRandom; NAMES = NAMES
    WOODCUTTING = WOODCUTTING; MINING = MINING; FIREMAKING = FIREMAKING


def census(out):
    counts = {}
    for case in out:
        key = case["kind"] if case["kind"] != "scenario" else "scenario:" + case["subject"]
        counts[key] = counts.get(key, 0) + 1
    return dict(sorted(counts.items()))
