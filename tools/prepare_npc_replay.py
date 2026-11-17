"""Build explicit original NPC32 rendering/audio input; never NPC AI or gameplay."""
import argparse,hashlib,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'oracle/players'))
from generate import Bits

def generate(check=False,overheads=False):
    frames=[];ids=[]
    def packet(tick,changes=None,adds=()):
        nonlocal ids
        changes=changes or {};bits=Bits();bits.put(8,len(ids));extended=[];next_ids=[]
        for index in ids:
            change=changes.get(index);bits.put(1,change is not None)
            if change is None:next_ids.append(index);continue
            kind,args=change;bits.put(2,kind)
            if kind==3:continue
            next_ids.append(index)
            if kind==0:extended.append((index,args));continue
            direction,mask=args;bits.put(3,direction);bits.put(1,mask is not None)
            if mask is not None:extended.append((index,mask))
        for index,id,x,y,angle,mask in adds:
            next_ids.append(index);bits.put(15,index);bits.put(1,1);bits.put(3,angle);bits.put(1,mask is not None);bits.put(5,(y-3218)&31);bits.put(14,id);bits.put(5,(x-3222)&31)
            if mask is not None:extended.append((index,mask))
        if extended:bits.put(15,32767)
        data=bytearray(bits.bytes())
        for _,mask in extended:
            if mask[0]=='animation':data.extend(bytes([16])+(mask[1]&65535).to_bytes(2,'big')+bytes([0]))
            elif mask[0]=='type':data.extend(bytes([1])+mask[1].to_bytes(2,'little'))
            elif mask[0]=='overheads':
                _,damage,bar,text=mask
                data.extend(bytes([0x62,damage,255,(128-bar)&255,0,128])+text.encode('latin1')+b'\0')
            elif mask[0]=='hit':data.extend(bytes([2,(-mask[1])&255,127]))
            else:raise ValueError(mask)
        frames.append(dict(tick=tick,opcode=32,payload=list(data)));ids=next_ids
    anim=lambda id:('animation',id)
    packet(0,adds=[(10,0,3221,3214,0,None),(20,1,3223,3214,0,None),(30,81,3225,3217,0,None)])
    packet(30,{10:(0,anim(863)),20:(1,(4,None)),30:(1,(3,None))})
    if overheads:
        packet(30,{10:(0,('overheads',3,192,'Hello there! Welcome to Lumbridge.')),20:(0,('overheads',1,64,'Good day, adventurer.'))})
        packet(31,{10:(0,('hit',5))})
        packet(32,{10:(0,('hit',9))})
        packet(33,{10:(0,('hit',11))})
    packet(60,{20:(3,None),30:(1,(4,None))})
    packet(90,adds=[(20,9,3223,3215,0,anim(866))])
    if overheads:packet(95,{20:(0,('overheads',2,128,'Mind how you go.'))})
    packet(120,{10:(0,anim(863))})
    packet(150,{10:(3,None),30:(1,(1,None))})
    packet(180,{30:(1,(6,None))},adds=[(10,0,3221,3214,0,anim(863))])
    packet(210,{20:(0,anim(-1))})
    packet(240,{20:(1,(3,None)),10:(1,(4,None))})
    packet(270,{20:(1,(4,None)),10:(1,(3,None))})
    packet(300,{10:(0,anim(863)),20:(0,anim(866))})
    if overheads:packet(300,{10:(0,('overheads',0,255,'Hello again!')),20:(0,('overheads',4,224,'Enjoy your stay.'))})
    packet(330,{30:(0,('type',81))})
    packet(360,{10:(0,anim(-1))})
    packet(390,{20:(0,anim(-1))})
    packet(420,{10:(0,anim(863))})
    for tick in [30,180,300,420]:frames.append(dict(tick=tick,opcode=172,payload=[0,127,1,0,0]))
    frames.sort(key=lambda f:f['tick'])
    source=ROOT/'reference/client/client/src/main/java/rt4/Protocol.java'
    result=dict(label='Explicit original NPC32 rendering/audio'+('/overheads'if overheads else'')+' fixture; no AI or authoritative gameplay',start=[3222,3218],protocol_sha256=hashlib.sha256(source.read_bytes().replace(b'\r\n',b'\n')).hexdigest(),frames=frames)
    content=json.dumps(result,separators=(',',':')).encode()+b'\n';path=ROOT/'fixtures/players'/('npc-overheads-replay.json'if overheads else'npc-render-replay.json')
    if check:
        if path.read_bytes()!=content:raise SystemExit('NPC rendering fixture differs')
    else:path.write_bytes(content)
    print(f'NPC rendering fixture: {len(frames)} packets, 3 distinct server indices, spawn/remove/readd/walk/wave/dance/type-change and original synthesized effect requests')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--overheads',action='store_true');a=p.parse_args();generate(a.check,a.overheads)
