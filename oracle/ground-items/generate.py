"""Unchanged RT4 ground packet branches, pile selection and intrusive lists.

Only dependency adapters, metadata/height/publication services and observations
are supplied here. The reference checkout is never edited.
"""
import argparse
import hashlib
import importlib.util
import json
import pathlib
import random
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/ground-items-oracle'
FIX = ROOT / 'fixtures/ground-items/oracle.json'


def generate(check=False, java_home=None):
    pins = json.loads((ROOT / 'upstream.lock.json').read_text(encoding='utf-8'))
    commit = subprocess.run(['git', '-C', str(ROOT / 'reference/client'), 'rev-parse', 'HEAD'], capture_output=True, text=True, check=True).stdout.strip()
    assert commit == pins['client']['commit'], 'Client reference differs from pin'
    subprocess.run(['git', '-C', str(ROOT / 'reference/client'), 'diff', '--quiet', 'HEAD', '--'], check=True)
    spec = importlib.util.spec_from_file_location('visual', ROOT / 'oracle/visual/generate.py')
    visual = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(visual)
    sources = {}

    def source(name):
        raw = (ROOT / f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n', b'\n')
        sources[name + '.java'] = hashlib.sha256(raw).hexdigest()
        return visual.clean(raw.decode())

    protocol, buffer = source('Protocol'), source('Buffer')
    names = ['OBJ_REVEAL', 'OBJ_ADD', 'OBJ_COUNT', 'OBJ_DEL']
    constants = source('ServerProt')
    code = 'class ServerProt{' + ''.join(re.search(r'public static final int ' + n + r' = [^;]+;', constants).group(0) for n in names) + '}\n'
    code += 'class Protocol{static Buffer inboundBuffer;static int opcode,length;'
    code += 'static void readZonePacket(){' + 'else '.join(visual.method(protocol, 'if (opcode == ServerProt.' + n + ')') for n in names) + '}'
    code += visual.method(protocol, 'public static void spawnGroundObject(')
    code += 'static void frame(int op,byte[]data){inboundBuffer=new Buffer(data);opcode=op;length=data.length;switch(op){'
    for op, name in [(26, 'UPDATE_ZONE_PARTIAL_FOLLOWS'), (230, 'UPDATE_ZONE_PARTIAL_ENCLOSED')]:
        branch = visual.method(protocol, 'if (opcode == ServerProt.' + name + ')')
        code += f'case {op}:' + branch[branch.index('{') + 1:branch.rindex('return true;')] + 'break;'
    full = visual.method(protocol, 'if (opcode == ServerProt.UPDATE_ZONE_FULL_FOLLOWS)')
    code += 'case 112:' + full[full.index('{') + 1:full.index('for (ChangeLocRequest')] + 'break;default:readZonePacket();}}}\n'
    readers = ['g1(', 'g1add(', 'g1neg(', 'g1sub(', 'g2(', 'ig2(', 'g2add(', 'ig2add(']
    code += 'class Buffer{byte[]data;int offset;Buffer(byte[]data){this.data=data;}' + ''.join(visual.method(buffer, 'public final int ' + n) for n in readers) + '}\n'
    for name in ['Node', 'SecondaryNode', 'LinkedList', 'ObjStackNode']:
        # Public classes can share this generated compilation unit after removing
        # only the visibility modifier; all fields and method bodies are intact.
        code += source(name).replace('public final class ', 'final class ').replace('public class ', 'class ') + '\n'
    login = source('LoginManager')
    rebase = login[login.index('byte xEnd = 104;'):login.index('for (ChangeLocRequest', login.index('byte xEnd = 104;'))]
    code += 'class GroundRebase{static void shift(int deltaX,int deltaY){' + rebase + '}}\n'
    adapter = (HERE / 'Support.java').read_text(encoding='utf-8')
    code += adapter
    RUN.mkdir(parents=True, exist_ok=True)
    java = RUN / 'GroundItemsOracle.java'
    java.write_text(code, encoding='utf-8')
    def executable(name):
        if java_home is None:
            return name
        suffix = '.exe' if __import__('os').name == 'nt' else ''
        return str(pathlib.Path(java_home) / 'bin' / (name + suffix))
    subprocess.run([executable('javac'), '-encoding', 'UTF-8', '-d', str(RUN), str(java)], check=True)
    cases = scenarios()
    process = subprocess.run([executable('java'), '-cp', str(RUN), 'GroundItemsOracle'], input='\n'.join('|'.join(c['commands']) for c in cases) + '\n', text=True, encoding='utf-8', capture_output=True, check=True)
    lines = process.stdout.splitlines()
    assert len(lines) == len(cases), process.stdout[-1000:]
    for case, line in zip(cases, lines):
        case['expected'] = json.loads(line)
    result = dict(reference_commit=commit, sources=sources, adapter_sha256=hashlib.sha256(adapter.encode()).hexdigest(), bindings='Unchanged four Protocol.readZonePacket ground branches, complete spawnGroundObject, complete Node/SecondaryNode/LinkedList/ObjStackNode, original selected Buffer readers, zone origin/enclosed ground phases and LoginManager array-shift statements. Explicit ObjStack identity/fields, mutable metadata, terrain-height and publication callbacks. Not ObjStack model rendering, scenery resetLoops, SceneGraph placement/raiseZ, world simulation or scene teardown/rebuild.', cases=cases)
    if check:
        assert json.loads(FIX.read_text(encoding='utf-8')) == result, 'Ground item fixture differs'
    else:
        FIX.parent.mkdir(parents=True, exist_ok=True)
        FIX.write_text(json.dumps(result, separators=(',', ':')) + '\n', encoding='utf-8')
    print('Original ground items:', len(cases), 'scenarios,', sum(len(c['commands']) for c in cases), 'commands')


def wire(op, *data):
    return 'U ' + str(op) + ' ' + bytes(x & 255 for x in data).hex()


def add(x, y, item, amount):
    return wire(33, item, item >> 8, (x << 4) | y, amount >> 8, amount + 128)


def scenarios():
    cases = []

    def case(name, *commands):
        cases.append(dict(name=name, commands=list(commands)))

    setup = ['M 1 10 0', 'M 2 20 0', 'M 3 5 1', 'M 4 30 0']
    case('duplicates-first-count-remove', *setup, add(2, 3, 1, 1), add(2, 3, 1, 2), add(2, 3, 2, 1), wire(14, 35, 128, 1, 0, 1, 0, 7), wire(240, 93, 128, 1), wire(240, 93, 0, 2))
    case('ties-and-distinct-render-objects', *setup, 'K 0 2 3 1 0', 'K 0 2 3 1 1', 'K 0 2 3 4 0', 'K 0 2 3 2 0', 'K 0 2 3 2 1', 'K 0 2 3 3 0', 'R 2 3', 'M 1 30 0', 'R 2 3', 'M 4 -100 0', 'R 2 3')
    case('strict-value-floor', 'M 1 -99999999 0', add(0, 0, 1, 1), 'M 1 -99999998 0', 'R 0 0', 'M 1 -100000000 0', 'R 0 0')
    for stackable in [-1, 0, 1, 2]:
        case(f'wrapping-value-{stackable}', f'M 1 2147483647 {stackable}', 'M 2 1 0', 'K 0 0 0 1 2147483647', 'K 0 0 0 2 1', 'R 0 0', 'V 1 1 -2147483648', 'R 0 0')
    case('owner-is-network-slot', 'P 2 75', wire(26, -8, 16), wire(135, 75 + 128, 0, -35, 7, 0, 227, 3), wire(135, 74 + 128, 0, -35, 7, 0, 227, 3), wire(33, 227, 3, 35, 0, 129), wire(112, 8, -16))
    case('high-id-is-not-masked-on-add', add(0, 0, 32769, 1), wire(240, 128, 128, 1), add(0, 0, 1, 5), wire(240, 128, 128, 1))
    case('nonnull-empty-list-and-unmatched-count', 'E 0 0 0', wire(14, 0, 0, 1, 0, 0, 0, 1), 'K 0 0 0 2 1', wire(240, 128, 0, 1), 'R 1 1')
    case('full-zone-bounds-prefix', add(0, 0, 1, 2), wire(26, -100, 100), add(3, 3, 2, 2), wire(112, 100, -100))
    case('out-of-scene-updates', wire(26, -255, 255), add(7, 7, 1, 1), wire(240, 0, 0, 1), wire(14, 0, 0, 1, 0, 0, 0, 1))
    case('enclosed-multiple-ground', *setup, wire(230, 8 + 128, 128 - 8, 33, 1, 0, 35, 0, 129, 33, 2, 0, 35, 0, 129, 14, 35, 0, 1, 0, 1, 0, 8, 240, 128 - 35, 0, 2))
    for dx, dy in [(1, 2), (-1, -2), (0, 0), (104, 0), (0, -104), (2147483647, -2147483648)]:
        case(f'rebase-{dx}-{dy}', 'K 0 2 3 1 1', 'K 3 103 103 2 2', f'Z {dx} {dy}')
    for stage in ['info', 'height', 'publish']:
        for command in ['X', 'P 1 7', 'L 0 2 3', 'V 1 2 99', 'E 0 2 3']:
            case(f'callback-{stage}-{command}', *setup, 'K 0 2 3 1 0', 'K 0 2 3 2 0', 'R 2 3', f'F {stage} 1 {command}', 'R 2 3')
    case('captured-next-unlinked', *setup, 'K 0 2 3 1 0', 'K 0 2 3 2 0', 'K 0 2 3 3 0', 'F info 1 D 2', 'R 2 3')
    case('nested-refresh-replaces-list-cursor', *setup, 'K 0 2 3 1 0', 'K 0 2 3 2 0', 'F info 1 R 2 3', 'R 2 3')
    case('best-node-transferred-before-promotion', *setup, 'K 0 2 3 1 0', 'K 0 2 3 2 0', 'F info 2 T 0 5 5 1', 'R 2 3')
    case('iterator-observation-and-removal', 'K 0 2 3 1 0', 'K 0 2 3 2 0', 'H 0 2 3', 'D 2', 'N 0 2 3', 'N 0 2 3')
    case('retained-node-after-list-replacement', 'K 0 2 3 1 0', 'K 0 2 3 2 0', 'L 0 2 3', 'D 1', 'T 0 5 5 2', 'R 5 5')
    case('best-moved-into-discarded-list', *setup, 'K 0 2 3 1 0', 'K 0 2 3 2 0', 'F info 2 T 0 5 5 1', 'R 2 3', 'L 0 5 5', 'D 1', 'R 2 3')
    case('packet-add-failure-keeps-created-node', 'F info 1 X', add(2, 3, 1, 8), 'D 1', 'R 2 3')
    rebase_rng = random.Random(530104)
    deltas = [-2147483648, -105, -104, -103, -8, -1, 0, 1, 8, 103, 104, 105, 2147483647]
    for i in range(64):
        commands = []
        for j in range(12):
            p, x, y = rebase_rng.randrange(4), rebase_rng.randrange(104), rebase_rng.randrange(104)
            commands.append(f'K {p} {x} {y} {j % 4} {j}')
        commands.extend(['E 0 50 50', 'A 1 50 50 0 50 50', 'K 1 50 50 995 17', f'Z {deltas[i % len(deltas)]} {deltas[(i * 7) % len(deltas)]}', 'V 1 50 100', 'Z -1 1'])
        case(f'array-rebase-alias-{i}', *commands)
    rng = random.Random(530240)
    reader_rng = random.Random(530135)
    for opcode, size in [(33, 5), (135, 7), (240, 3), (14, 7)]:
        for value in [0, 1, 127, 128, 129, 254, 255]:
            case(f'reader-{opcode}-{value}', wire(opcode, *([value] * size)))
        for length in range(size):
            case(f'truncated-{opcode}-{length}', wire(opcode, *([255] * length)))
        for i in range(32):
            case(f'raw-reader-{opcode}-{i}', wire(opcode, *(reader_rng.randrange(256) for _ in range(size))))
    for i in range(192):
        commands = [f'P {i % 4} {i % 2048}']
        for item in range(1, 6):
            commands.append(f'M {item} {rng.choice([-100000000, -99999999, -1, 0, 1, 10, 65535, 2147483647])} {rng.randrange(3)}')
        for j in range(20):
            x, y, item, amount = rng.randrange(2), rng.randrange(2), rng.randrange(1, 6), rng.choice([0, 1, 2, 32767, 65535])
            if j % 4 < 2:
                commands.append(add(x, y, item, amount))
            elif j % 4 == 2:
                commands.append(wire(240, 128 - ((x << 4) | y), 128 if i % 2 else 0, item))
            else:
                new = rng.randrange(65536)
                commands.append(wire(14, (x << 4) | y, 128 if i % 2 else 0, item, amount >> 8, amount, new >> 8, new))
        case(f'packets-{i}', *commands)
    return cases


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', help='JDK 17+; CI uses JDK 25')
    args = parser.parse_args()
    generate(args.check, args.java_home)
