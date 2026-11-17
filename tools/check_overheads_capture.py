"""Check the packaged native GPU capture and logical overhead lifecycle."""
import argparse,json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1]

def check(path):
    visual=path/'runs/visual'
    rows=[json.loads(line)for line in(visual/'actor-frames/frames.jsonl').read_text().splitlines()]
    npc=json.loads((visual/'npc-state.json').read_text());ui=json.loads((visual/'ui-state.json').read_text())
    final=json.loads((visual/'animation-state.json').read_text())
    assert npc['diagnostics']==[] and ui['diagnostics']==[]
    assert npc['fixture_remaining']==0 and npc['sounds_submitted']==4
    assert len(rows)==81 and all((visual/'actor-frames'/f['file']).is_file()for f in rows)
    assert all(b['time']>a['time'] and b['cycle']>=a['cycle']for a,b in zip(rows,rows[1:]))
    assert rows[-1]['time']-rows[0]['time']>=12
    assert {len(f['npcs'])for f in rows}=={2,3}
    records=[(f['cycle'],a['index'],a['state'])for f in rows for a in f['overheads']if a['npc']]
    assert any([h['damage']for h in s['hits']]==[3,0,5,9]for _,i,s in records if i==10)
    assert not any(h['damage']==11 for _,_,s in records for h in s['hits'])
    assert {h['kind']for _,_,s in records for h in s['hits']}=={0,1}
    messages={s['chat']['text']for _,_,s in records if s['chat']is not None}
    assert {'Hello there! Welcome to Lumbridge.','Good day, adventurer.','Mind how you go.','Hello again!','Enjoy your stay.'}<=messages
    assert any(s['chat']is None and s['bar']==192 for _,i,s in records if i==10)
    assert {64,128,192,224,255}<={s['bar']for _,_,s in records}
    assert max(f['overhead_commands']for f in rows)>=22
    assert final['overhead_commands']==0 and final['cycle']==rows[-1]['cycle']
    assert all(s['chat']is None and s['bar_until']<=final['cycle'] and all(h['until']<=final['cycle']for h in s['hits'])for a in final['overheads']for s in[a['state']])
    peak=max(rows,key=lambda f:f['overhead_commands'])
    result=dict(gpuSamples=len(rows),elapsedSeconds=rows[-1]['time']-rows[0]['time'],peakDrawCommands=peak['overhead_commands'],peakImage=str(visual/'actor-frames'/peak['file']),fifthHitDiscarded=True,chatAndHealthExpired=True,soundSubmissions=npc['sounds_submitted'],diagnostics=[])
    print(json.dumps(result,indent=2));return result

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('path',type=pathlib.Path,nargs='?',default=ROOT/'runs/overheads-validation/native-valid');check(p.parse_args().path)
