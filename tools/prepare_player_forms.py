"""Prepare explicit offline player appearance packets from pinned NPC/BAS assets."""
import argparse, hashlib, json, pathlib, struct
from prepare_visual import Cache, npc_dependencies

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / 'fixtures/players/forms'

def generate(check=False):
    cache = Cache(ROOT / 'assets/cache')
    files = {}
    forms = []
    for label, id in [('chicken',41),('cow',81),('dwarf',4),('morph',3384)]:
        definition = cache.file(18,id>>7,id&127)
        models,bas,children = npc_dependencies(definition)
        packet = bytes([0,255,255,255,255]) + struct.pack('>H',id) + bytes([0]*6)
        packet += struct.pack('>H',bas) + struct.pack('>q',1) + bytes([3,3,255,0])
        name = f'player-{label}.bin'
        files[name] = packet
        forms.append(dict(file=name,npc=id,bas=bas,definitionSha256=hashlib.sha256(definition).hexdigest(),
                          basSha256=hashlib.sha256(cache.file(2,32,bas)).hexdigest(),models=sorted(models),children=sorted(children)))
    frames=[]
    for tick,value in [(30,4),(90,0),(150,4),(210,8)]:
        frames.append(dict(tick=tick,opcode=37,payload=[(value|128)&255,1850&255,1850>>8]))
    files['player-morph-replay.json']=(json.dumps(dict(label='Explicit transformed-player varbit preview; no server gameplay',start=[3222,3218],frames=frames),separators=(',',':'))+'\n').encode()
    files['manifest.json']=(json.dumps(dict(scope='Original NPC definition/BAS metadata and explicit offline player appearance/variable packets. No authoritative transformation or interaction outcome.',forms=forms),indent=2)+'\n').encode()
    if check:
        for name,data in files.items():
            if (OUT/name).read_bytes()!=data:raise SystemExit('Player form preview differs: '+name)
    else:
        OUT.mkdir(parents=True,exist_ok=True)
        for name,data in files.items():(OUT/name).write_bytes(data)
    print('Player appearance previews:',len(forms),flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    generate(parser.parse_args().check)
