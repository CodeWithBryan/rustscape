"""Complete original BasType decoder, focused on per-part NPC body transforms."""
import argparse, hashlib, importlib.util, json, pathlib, random, struct, subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
RUN = ROOT / 'runs/npc-bas-oracle'
FIX = ROOT / 'fixtures/ui/npc-bas.json'


def generate(root, clean, method, check=False):
    RUN.mkdir(parents=True, exist_ok=True)
    sources = {}
    def source(name):
        raw = (root / f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n', b'\n')
        sources[name + '.java'] = hashlib.sha256(raw).hexdigest()
        return clean(raw.decode())
    (RUN / 'BasType.java').write_text(source('BasType'), encoding='utf-8')
    buffer = source('Buffer')
    code = 'class Buffer {byte[]data;int offset;Buffer(byte[]b){data=b;}'
    code += ''.join(method(buffer, signature) for signature in ['public final int g1(', 'public final int g2(', 'public final int g2b(']) + '}'
    (RUN / 'Buffer.java').write_text(code, encoding='utf-8')
    (RUN / 'Probe.java').write_text(DRIVER, encoding='utf-8')
    subprocess.run(['javac', '-encoding', 'UTF-8', '-d', str(RUN), *map(str, RUN.glob('*.java'))], check=True)
    u2 = lambda n: struct.pack('>H', n & 65535)
    rng = random.Random(53020091020)
    cases = []
    for index in range(256):
        raw = bytearray([1]) + u2(rng.choice([-1, 808, 819, 65534])) + u2(rng.choice([-1, 819, 65534]))
        raw += bytes([26, index, 255-index])
        for _ in range(index % 17):
            slot = rng.randrange(12)
            raw += bytes([27, slot]) + b''.join(u2(rng.randrange(-32768, 32768)) for _ in range(6))
        # Duplicated slots replace the complete old six-field array.
        if index % 3 == 0:
            for values in [[-32768, -1, 32767, 0, 1024, 2047], [127, -128, 64, 1, 512, 1792]]:
                raw += bytes([27, 11]) + b''.join(map(u2, values))
        for opcode in [2, 6, 38, 39, 40, 41, 42, 43, 44, 45]:
            raw += bytes([opcode]) + u2(rng.randrange(65536))
        raw += b'\0'
        cases.append(dict(bytes=raw.hex()))
    result = subprocess.run(['java', '-cp', str(RUN), 'Probe'], input='\n'.join(c['bytes'] for c in cases)+'\n', text=True, capture_output=True, check=True)
    rows = result.stdout.splitlines()
    assert len(rows) == len(cases)
    for case, row in zip(cases, rows): case['expected'] = json.loads(row)
    content = dict(sources=sources, scope='Unchanged complete original BasType decoder and original Buffer reads;256 deterministic valid byte streams. Movement fields, signed per-part transforms, duplicate-slot replacement and stop offsets. Geometry is independently covered by the existing original player-form body/GL corpus. Acceleration remains rejected by the Rust movement decoder; no pinned-cache visual change or full NPC renderer claim.', cases=cases)
    if check:
        if json.loads(FIX.read_text()) != content: raise SystemExit('NPC BAS fixture differs')
    else: FIX.write_text(json.dumps(content, separators=(',', ':'))+'\n')
    print('Original NPC BAS decoder:', len(cases), 'observations', flush=True)


DRIVER = r'''
import java.io.*;import java.util.*;
public class Probe {
public static void main(String[]args)throws Exception {
BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));String line;
while((line=reader.readLine())!=null){Buffer data=new Buffer(HexFormat.of().parseHex(line));BasType b=new BasType();b.decode(data);
System.out.println("["+Arrays.toString(new int[]{b.idleAnimationId,b.walkAnimation,b.runAnimationId,b.standingCCWTurn,b.standingCWTurn,b.walkFullTurnAnimationId,b.walkCCWTurnAnimationId,b.walkCWTurnAnimationId,b.slowWalkAnimationId,b.slowWalkFullTurnAnimationId,b.slowWalkCCWTurnAnimationId,b.slowWalkCWTurnAnimationId,b.runFullTurnAnimationId,b.runCCWTurnAnimationId,b.runCWTurnAnimationId,b.terrainAlignWidth,b.terrainAlignDepth})+","+Arrays.deepToString(b.modelRotateTranslate)+","+data.offset+"]");
}}}
'''

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    spec = importlib.util.spec_from_file_location('npc_bas_visual', ROOT / 'oracle/visual/generate.py')
    visual = importlib.util.module_from_spec(spec);spec.loader.exec_module(visual)
    generate(ROOT, visual.clean, visual.method, parser.parse_args().check)
