"""Validate actual packaged NPC packet-replay rendering/audio readbacks."""
import json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1]
visual=ROOT/'runs/visual';state=json.loads((visual/'npc-update-state.json').read_text());frames=[json.loads(s)for s in(visual/'npc-update-frames.jsonl').read_text().splitlines()]
assert state['diagnostics']==[] and state['fixture_remaining']==0
assert state['sounds_submitted']==4
assert {n['index']for n in state['npcs']}=={10,20,30}
assert next(n for n in state['npcs']if n['index']==30)['size']==2
assert all(b['time']>a['time']for a,b in zip(frames,frames[1:]))
assert frames[-1]['time']-frames[0]['time']>=12
assert {len(f['npcs'])for f in frames}=={2,3}
wave=[n for f in frames for n in f['npcs']if n['action'][0]==863]
dance=[n for f in frames for n in f['npcs']if n['action'][0]==866]
assert len(wave)>=8 and len(dance)>=8
assert len({n['action'][1]for n in wave})>=8 and len({n['action'][1]for n in dance})>=8
assert len({tuple(n['fine'])for f in frames for n in f['npcs']if n['index']==30})>=4
assert {n['type']for f in frames for n in f['npcs']if n['index']==20}=={1,9}
assert any(10 not in {n['index']for n in f['npcs']}for f in frames)
assert any(20 not in {n['index']for n in f['npcs']}for f in frames)
print(f"Packaged Windows NPC replay: {len(frames)} GPU frames/{frames[-1]['time']-frames[0]['time']:.2f}s; three wire indices, walking, removal/readdition, {len(wave)} wave/{len(dance)} dance poses, four sound submissions, zero diagnostics.")
