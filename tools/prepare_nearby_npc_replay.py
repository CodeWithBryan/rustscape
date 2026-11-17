"""Explicit visual input for the expanded NPC pack; no NPC AI or gameplay."""
import argparse,hashlib,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'oracle/players'))
from generate import Bits
from npc_assets import ROOT_TYPES

def generate(check=False):
    bits=Bits();bits.put(8,0)
    # Display each logical type once, retaining the invisible morph parent3384.
    for index,id in enumerate(ROOT_TYPES,10):
        column=(index-10)%5;row=(index-10)//5;x=3216+column*2;y=3208+row*2
        bits.put(15,index);bits.put(1,1);bits.put(3,0);bits.put(1,0)
        bits.put(5,(y-3218)&31);bits.put(14,id);bits.put(5,(x-3222)&31)
    frames=[dict(tick=0,opcode=32,payload=list(bits.bytes()))]
    # Original small Varbit packet37: p1(-128|value), ip2(id).
    for tick,value in [(30,4),(90,0),(150,4),(210,8)]:
        frames.append(dict(tick=tick,opcode=37,payload=[(value|128)&255,1850&255,1850>>8]))
    source=ROOT/'reference/client/client/src/main/java/rt4/Protocol.java'
    result=dict(label='Explicit nearby NPC asset and morph packet replay; synthetic display positions, no AI or authoritative gameplay',start=[3222,3218],protocol_sha256=hashlib.sha256(source.read_bytes().replace(b'\r\n',b'\n')).hexdigest(),frames=frames)
    content=json.dumps(result,separators=(',',':')).encode()+b'\n';path=ROOT/'fixtures/players/npc-nearby-replay.json'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Nearby NPC rendering fixture differs')
    else:path.write_bytes(content)
    print(f'Nearby NPC visual fixture: {len(ROOT_TYPES)} logical types, default-hidden/visible/hidden/visible/fallback-hidden morph phases')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args();generate(a.check)
