"""Check actual Windows ground-click request/server response/render trajectories."""
import json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1]/'runs/visual'
for mode in ['walk','run']:
    prefix=f'live-{mode}-native'
    read=lambda suffix:json.loads((ROOT/f'{prefix}-{suffix}.json').read_text())
    state=read('state');network=read('network');actions=read('actions');ui=read('ui')
    frames=[json.loads(s)for s in(ROOT/f'{prefix}-frames.jsonl').read_text().splitlines()]
    assert state['live']and state['capture_route_issued']
    assert network['in_game']and network['diagnostics']==[]
    assert network['frames']['225']>=5 and network['frames']['153']>=1
    assert len(actions)==1 and actions[0]['opcode']==215
    assert actions[0]['payload'][0]==(129 if mode=='run' else 128)
    assert state['head']==[3211,3215]and state['fine']==[3211*128+64,3215*128+64]
    assert state['movement_sequence']==808 and state['destination_flag']==[0,47]
    poses=[f['movement']for f in frames if f['movement']['base']is not None]
    server={(p['server_tile'][0]+p['base'][0],p['server_tile'][1]+p['base'][1])for p in poses}
    assert (3207,3216)in server and (3211,3215)in server
    assert len(server)>=3
    active=[p for p in poses if p['sequence']==(824 if mode=='run' else 819)]
    assert len(active)>=2 and len({tuple(p['fine'])for p in active})>=2
    assert len({tuple(p['fine'])for p in poses})>=6
    if mode=='run':assert network['frames']['234']>=1 and 0<=ui['run_energy']<=100
    print(f"Windows {mode}: {len(frames)} GPU samples, {len(server)} authoritative tile samples, {len(active)} locomotion poses, final (3211,3215), clear153 handled, zero diagnostics.")
