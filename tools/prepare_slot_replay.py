"""Authored NPC32 wire masks and separate offline self requests for part rendering."""
import argparse,hashlib,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'oracle/players'))
from generate import Bits

def generate(check=False):
    frames=[];ids=[100,101]
    bits=Bits();bits.put(8,0)
    for index,type,dx,dy in [(100,0,-2,0),(101,1,2,0)]:
        for n,v in [(15,index),(1,1),(3,4),(1,0),(5,dy&31),(14,type),(5,dx&31)]:bits.put(n,v)
    frames.append(dict(tick=0,opcode=32,payload=list(bits.bytes())))
    def update(tick,requests):
        bits=Bits();bits.put(8,2)
        for index in ids:
            bits.put(1,index in requests)
            if index in requests:bits.put(2,0)
        bits.put(15,32767);payload=bytearray(bits.bytes())
        for index in ids:
            if index not in requests:continue
            entries=requests[index];payload.extend([8,1,(-len(entries))&255])
            for sequence,delay,mask in entries:payload.extend((sequence&65535).to_bytes(2,'little')+bytes([(128-delay)&255])+mask.to_bytes(2,'big'))
        frames.append(dict(tick=tick,opcode=32,payload=list(payload)))
    update(12,{100:[(855,16,0x1)],101:[(856,0,0xfff)]})
    update(70,{100:[(856,0,0xfff)],101:[(855,12,0x1)]})
    update(140,{100:[(855,0,0xfff)],101:[(856,0,0xfff)]})
    update(220,{100:[(-1,0,0xfff)],101:[(-1,0,0xfff)]})
    update(260,{100:[(855,0,0x1)],101:[(856,0,0xfff)]})
    update(340,{100:[(-1,0,0xfff)],101:[(-1,0,0xfff)]})
    self_requests=[dict(tick=tick,requests=requests)for tick,requests in [
        (12,[(855,16,0x1)]),(70,[(856,0,0xfff)]),(140,[(855,0,0xfff)]),
        (220,[(-1,0,0xfff)]),(260,[(855,0,0x1)]),(340,[(-1,0,0xfff)])]]
    sources={n:hashlib.sha256((ROOT/'reference/client/client/src/main/java/rt4'/n).read_bytes().replace(b'\r\n',b'\n')).hexdigest()for n in ['Protocol.java','Player.java','NpcList.java','PlayerAppearance.java','NpcType.java']}
    output=dict(label='Authored rendering input. NPCs use real NPC_INFO32 slot mask encoding; self uses explicit offline request API. Delay, multiple part masks, replacement and clears; no authoritative gameplay or PLAYER_INFO wire claim.',start=[3222,3218],sources=sources,frames=frames,player_slots=self_requests)
    content=json.dumps(output,separators=(',',':')).encode()+b'\n';path=ROOT/'fixtures/players/slot-render-replay.json'
    if check:assert path.read_bytes()==content,'Slot replay differs'
    else:path.write_bytes(content)
    print('Authored part rendering replay:',len(frames),'NPC32 packets and',len(self_requests),'self request batches')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args();generate(a.check)
