"""Scenery door options that the pinned content claims before DoorManagingPlugin.

DoorManagingPlugin is the global handler of "open", "close", "shut" and
"go-through". The original's content claims many objects first: Kotlin
InteractionListeners `on(ids, SCENERY, "open", ...)` (run before any option
handler) and per-definition handlers `SceneryDefinition.forId(id)
.getHandlers().put("option:open", ...)` (looked up before the global one).
None of that content is ported in the live world, so it must refuse those
objects by name instead of letting the door handler open them.

The table is read from the pinned sources of the classes in the class scan
(crates/server/src/startup/scan_order.txt). An id expression is resolved from
integer literals, ConstLib constants (`Scenery.NAME_1234`), constants and
literal arrays declared in the same file. Expressions it cannot resolve are
listed with their class in UNRESOLVED; the live world names those classes.
"""
import argparse
import hashlib
import pathlib
import re
import subprocess

R = pathlib.Path(__file__).resolve().parents[1]
SRC = R / 'reference/server/Server/src/main'
SCAN = R / 'crates/server/src/startup/scan_order.txt'
TARGET = R / 'crates/server/src/world/live/door_claims.rs'
OPTIONS = ('open', 'close', 'shut', 'go-through')
SECTIONS = ('[scan.contentInterfaces]', '[scan.initializable]')


def scanned():
    names, section = set(), None
    for line in SCAN.read_text(encoding='utf-8').splitlines():
        if line.startswith('['):
            section = line
        elif section in SECTIONS and line and not line.startswith('#'):
            names.add(line.strip())
    return names


def call_args(text, start):
    """The argument strings of the call whose '(' is at start."""
    depth, args, cur, quote = 0, [], '', None
    for i in range(start, len(text)):
        c = text[i]
        if quote:
            cur += c
            if c == quote and text[i - 1] != '\\':
                quote = None
        elif c == '"':
            quote = c
            cur += c
        elif c in '([{':
            depth += 1
            if depth > 1:
                cur += c
        elif c in ')]}':
            depth -= 1
            if depth == 0:
                args.append(cur.strip())
                return args
            cur += c
        elif c == ',' and depth == 1:
            args.append(cur.strip())
            cur = ''
        else:
            cur += c
    return None


def split_list(body):
    out, depth, cur = [], 0, ''
    for c in body:
        if c in '([{':
            depth += 1
        elif c in ')]}':
            depth -= 1
        if c == ',' and depth == 0:
            out.append(cur.strip())
            cur = ''
        else:
            cur += c
    if cur.strip():
        out.append(cur.strip())
    return out


LIST = re.compile(r'(?:intArrayOf|arrayOf|listOf|setOf|mutableListOf)\s*\((.*)\)', re.S)
JAVA_ARRAY = re.compile(r'(?:new\s+int\s*\[\s*\]\s*)?\{(.*)\}', re.S)
CONSTLIB = re.compile(r'(?:org\.rs09\.consts\.)?(?:Scenery|Sceneries)\.[A-Z0-9_]*?_(\d+)')


def declaration(name, text):
    pattern = (r'(?:(?:val|var|int|Integer|int\[\]|IntArray|SceneryDefinition)\s+|,\s*)' + re.escape(name)
               + r'\s*(?::\s*[\w<>\[\]]+)?\s*=(?!=)\s*')
    d = re.search(pattern, text)
    if not d:
        # A loop variable: for (int i : DOORS) / for (i in DOORS).
        loop = re.search(r'for\s*\(\s*(?:(?:final\s+)?\w+\s+)?' + re.escape(name)
                         + r'\s*(?::|in)\s*([^)]+)\)', text)
        return loop.group(1) if loop else None
    depth, out = 0, ''
    for c in text[d.end():]:
        if c in '([{':
            depth += 1
        elif c in ')]}':
            depth -= 1
        if depth <= 0 and c in ';\n,':
            break
        out += c
    return out


def resolve(expr, text, seen=()):
    expr = re.sub(r'//[^\n]*', '', expr).strip().rstrip(',').strip()
    if re.fullmatch(r'-?\d+', expr):
        return [int(expr)]
    m = LIST.fullmatch(expr) or JAVA_ARRAY.fullmatch(expr)
    if m:
        ids = []
        for part in split_list(m.group(1)):
            r = resolve(part, text, seen)
            if r is None:
                return None
            ids += r
        return ids
    f = re.fullmatch(r'(?:SceneryDefinition\.)?forId\s*\((.*)\)', expr, re.S)
    if f:
        return resolve(f.group(1), text, seen)
    if expr in seen or not re.fullmatch(r'[A-Za-z_][\w.]*', expr):
        return None
    for alias in re.findall(r'import\s+org\.rs09\.consts\.Scenery\s+as\s+(\w+)', text):
        q = re.fullmatch(re.escape(alias) + r'\.[A-Z0-9_]*?_(\d+)', expr)
        if q:
            return [int(q.group(1))]
    q = CONSTLIB.fullmatch(expr)
    if q:
        return [int(q.group(1))]
    parts = expr.split('.')
    body = declaration(parts[-1], text)
    if body is not None:
        return resolve(body, text, seen + (expr,))
    if len(parts) == 2:
        # Owner.NAME declared in another file's class or object.
        for other in OWNERS.get(parts[0], []):
            other_text = other.read_text(encoding='utf-8')
            body = declaration(parts[1], other_text)
            if body is not None:
                return resolve(body, other_text, seen + (expr,))
    return None


OWNERS = {}


def index_owners():
    for path in SRC.rglob('*'):
        if path.suffix in ('.kt', '.java'):
            for m in re.finditer(r'\b(?:class|object)\s+([A-Z]\w*)',
                                 path.read_text(encoding='utf-8')):
                OWNERS.setdefault(m.group(1), []).append(path)


def enclosing_class(text, pos, package, names):
    best = None
    for m in re.finditer(r'\b(?:class|object)\s+([A-Z]\w*)', text[:pos]):
        fq = package + '.' + m.group(1)
        if fq in names:
            best = fq
    return best


def scan():
    names = scanned()
    index_owners()
    claims, unresolved, digest = set(), set(), hashlib.sha256()
    for path in sorted(SRC.rglob('*')):
        if path.suffix not in ('.kt', '.java'):
            continue
        raw = path.read_bytes().replace(b'\r', b'')
        text = raw.decode('utf-8')
        if not any(f'"{o}"' in text or f'"option:{o}"' in text for o in OPTIONS):
            continue
        pm = re.search(r'^\s*package\s+([\w.]+)', text, re.M)
        if not pm:
            continue
        package = pm.group(1)
        found = False
        # InteractionListeners: on(ids, SCENERY, options..., handler).
        for m in re.finditer(r'(?<![\w.])on\s*\(', text):
            args = call_args(text, m.end() - 1)
            if not args or len(args) < 3 or not re.fullmatch(r'(?:IntType\.)?SCENERY', args[1]):
                continue
            opts = [a[1:-1].lower() for a in args[2:] if re.fullmatch(r'"[^"]*"', a)]
            opts = [o for o in opts if o in OPTIONS]
            cls = enclosing_class(text, m.start(), package, names)
            if not opts or cls is None:
                continue
            found = True
            ids = resolve(args[0], text)
            for o in opts:
                if ids is None:
                    unresolved.add((cls, o, ' '.join(args[0].split())))
                else:
                    claims.update((i, o, cls) for i in ids)
        # Per-definition handlers: forId(id).getHandlers().put("option:open", h),
        # forId(id).handlers["option:open"] = h, def.handlers[...] = h.
        offset = 0
        for line in text.split('\n'):
            line_start, offset = offset, offset + len(line) + 1
            om = re.search(r'"option:(open|close|shut|go-through)"', line)
            if not om or 'ItemDefinition' in line or 'NPCDefinition' in line:
                continue
            fm = re.search(r'forId\s*\(', line)
            if fm:
                args = call_args(line, fm.end() - 1)
                expr = args[0] if args else line.strip()
            else:
                vm = re.search(r'\b(\w+)\s*\.\s*(?:getHandlers\(\)|handlers)', line)
                expr = vm.group(1) if vm else line.strip()
            cls = enclosing_class(text, line_start, package, names)
            if cls is None:
                continue
            found = True
            ids = resolve(expr, text)
            if ids is None:
                unresolved.add((cls, om.group(1), ' '.join(expr.split())))
            else:
                claims.update((i, om.group(1), cls) for i in ids)
        if found:
            digest.update(path.relative_to(SRC).as_posix().encode() + b'\0' + raw)
    return sorted(claims), sorted(unresolved), digest.hexdigest()


def quote(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"') + '"'


def render(claims, locations, unresolved, digest):
    out = '// Generated by tools/port_door_claims.py from the pinned content sources;\n'
    out += '// do not edit by hand. Sources SHA256 ' + digest + '\n'
    out += '//! Scenery door options the pinned content claims before DoorManagingPlugin\n'
    out += '//! (InteractionListeners `on(..., SCENERY, ...)` and per-definition\n'
    out += '//! `option:` handlers of the classes in the class scan).\n\n'
    out += '/// (scenery id, option, class), sorted by id and option; the first class\n'
    out += '/// in source order of a shared key.\n'
    out += 'pub const CLAIMS: &[(i32, &str, &str)] = &[\n'
    out += ''.join(f'    ({i}, {quote(o)}, {quote(c)}),\n' for i, o, c in claims) + '];\n\n'
    out += '/// ([x, y, z], option, class): objects the class finds by location at\n'
    out += '/// start-up (`getScenery(x, y, z).id`).\n'
    out += 'pub const LOCATIONS: &[([i32; 3], &str, &str)] = &[\n'
    out += ''.join(f'    ([{x}, {y}, {z}], {quote(o)}, {quote(c)}),\n' for (x, y, z), o, c in locations) + '];\n\n'
    out += '/// (class, option, id expression): registrations whose ids the tool does\n'
    out += '/// not read from the source alone.\n'
    out += 'pub const UNRESOLVED: &[(&str, &str, &str)] = &[\n'
    out += ''.join(f'    ({quote(c)}, {quote(o)}, {quote(e)}),\n' for c, o, e in unresolved) + '];\n\n'
    out += '/// The content class claiming `option` of scenery `id`, if any.\n'
    out += 'pub fn claim(id: i32, option: &str) -> Option<&\'static str> {\n'
    out += '    let at = CLAIMS.partition_point(|(i, o, _)| (*i, *o) < (id, option));\n'
    out += '    CLAIMS\n        .get(at)\n        .filter(|(i, o, _)| *i == id && *o == option)\n'
    out += '        .map(|(_, _, c)| *c)\n}\n\n'
    out += '/// The content class claiming `option` of the object at `at`, if any.\n'
    out += 'pub fn claim_at(at: [i32; 3], option: &str) -> Option<&\'static str> {\n'
    out += '    LOCATIONS\n        .iter()\n        .find(|(l, o, _)| *l == at && *o == option)\n'
    out += '        .map(|(_, _, c)| *c)\n}\n'
    return subprocess.run(['rustfmt', '--edition', '2024', '--emit', 'stdout'], input=out, text=True,
                          capture_output=True, check=True, cwd=R, encoding='utf-8').stdout


# Door registrations read by hand from the source line named beside them (the
# check fails if that line changes). Ids, or a location for an object the
# class finds at start-up with getScenery(x, y, z).
RD = 'content/region/asgarnia/falador/quest/recruitmentdrive/RecruitmentDriveListeners.kt'
SEERS = 'content/region/fremennik/rellekka/quest/thefremenniktrials/SeersHouseListeners.kt'
MANUAL = {
    'Stages.successDoorArray': (RD, 'val successDoorArray = Stages.indexMap.values.map { it.successDoor }.toIntArray()',
                                'ids', 'Scenery\\.DOOR_(\\d+)\\)[,;]?\\s*$'),
    'EASTTRAPDOOR.id': (SEERS, 'val EASTTRAPDOOR = getScenery(2636, 3663, 2)!!', 'location', (2636, 3663, 2)),
    'WESTTRAPDOOR.id': (SEERS, 'val WESTTRAPDOOR = getScenery(2631, 3663, 2)!!', 'location', (2631, 3663, 2)),
}


def manual(claims, unresolved):
    locations, rest = set(), []
    for cls, o, e in unresolved:
        if e not in MANUAL:
            rest.append((cls, o, e))
            continue
        path, evidence, kind, value = MANUAL[e]
        text = (SRC / path).read_text(encoding='utf-8')
        assert evidence in text, f'{path}: {evidence!r} changed; review the door claims'
        if kind == 'ids':
            ids = [int(n) for n in re.findall(value, text, re.M)]
            assert ids, f'{path}: no ids for {e}'
            claims.extend((i, o, cls) for i in ids)
        else:
            locations.add((value, o, cls))
    return sorted(claims), sorted(locations), rest


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--check', action='store_true')
    a = p.parse_args()
    claims, unresolved, digest = scan()
    claims, locations, unresolved = manual(list(claims), unresolved)
    seen, unique = set(), []
    for i, o, c in claims:
        if (i, o) not in seen:
            seen.add((i, o))
            unique.append((i, o, c))
    out = render(unique, locations, unresolved, digest)
    if a.check:
        assert TARGET.read_text(encoding='utf-8') == out, 'door claims differ; review the pinned sources'
    else:
        TARGET.write_text(out, encoding='utf-8', newline='\n')
    print(len(unique), 'claims,', len(unresolved), 'unresolved', '(checked)' if a.check else '(generated)')


if __name__ == '__main__':
    main()
