"""Check actual packaged-client animation/network/audio capture readbacks."""
import argparse,json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--prefix',choices=['live-animation','motion-regression'],default='live-animation')
prefix=parser.parse_args().prefix
network=json.loads((ROOT/f'runs/visual/{prefix}-network.json').read_text())
state=json.loads((ROOT/f'runs/visual/{prefix}-state.json').read_text())
frames=[json.loads(s)for s in (ROOT/f'runs/visual/{prefix}-frames.jsonl').read_text().splitlines()]
assert network['in_game']and network['diagnostics']==[]
assert network['frames']['162']==1 and network['frames']['225']>=20
assert network['frames']['172']>=2
assert state['live']and state['sound_backend_submissions']==network['frames']['172']
assert all(f['live']for f in frames)
assert all(b['time']>a['time']for a,b in zip(frames,frames[1:]))
assert frames[-1]['time']-frames[0]['time']>=12
active=[f for f in frames if f['action']and f['action'][0]==863]
idle=[f for f in frames if not f['action']or f['action'][0]==-1]
assert len(active)>=8 and idle
assert len({f['action'][1]for f in active})>=8
assert frames[-1]['sounds_submitted']>=2
print(f"Packaged Windows live probe: {len(frames)} GPU frames over {frames[-1]['time']-frames[0]['time']:.2f}s; {len(active)} active wave poses plus idle; {state['sound_backend_submissions']} server sounds submitted; zero diagnostics.")
