"""Add starter-tree stump dependencies to the existing Lumbridge visual pack."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from prepare_visual import Cache, ROOT, loc_def, verify_cache


def prepare(output, base):
    raw = base.read_bytes()
    assert raw[:8] == b'RS09VIS1'
    end = 12 + int.from_bytes(raw[8:12], 'little')
    manifest = json.loads(raw[12:end])
    files = {name: raw[end+start:end+start+n] for name,(start,n) in manifest['files'].items()}
    path = ROOT / 'assets/cache'
    verify_cache(path)
    cache = Cache(path)
    scene = json.loads(files['scene.json'])
    for id in [1341,1342,1343,1345,1347,1351,3880,3884,37653]:
        data = cache.file(16,id>>8,id&255)
        files[f'loc/{id}'] = data
        definition = loc_def(data)
        scene['definitions'][str(id)] = definition
        for mid in definition['models']:
            if f'model/{mid}' not in files:
                files[f'model/{mid}'] = cache.file(7,mid)
    for id in [2734, 3102, 3103, 3104]:
        files[f'sound/{id}'] = cache.file(4,id)
    files['scene.json'] = json.dumps(scene,separators=(',',':')).encode()
    blob = bytearray()
    for name,data in files.items():
        manifest['files'][name] = [len(blob),len(data)]
        blob.extend(data)
    header = json.dumps(manifest,separators=(',',':')).encode()
    with output.open('xb') as f:
        f.write(b'RS09VIS1'+struct.pack('<I',len(header))+header)
        f.write(blob)
    print(json.dumps({'output':str(output),'sha256':hashlib.sha256(output.read_bytes()).hexdigest()}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--base', type=Path, default=ROOT / 'assets/visual/live/lumbridge-expanded.pack')
    args = parser.parse_args()
    prepare(args.output, args.base)
