"""Check native GPU-capture readbacks against the verified Lumbridge server trace."""
import hashlib,json,pathlib

ROOT=pathlib.Path(__file__).resolve().parents[1]
visual=ROOT/'runs/visual';trace=ROOT/'runs/scene-oracle/lumbridge-door.s2c';meta=json.loads(trace.with_suffix('.json').read_text(encoding='utf-8'))
assert hashlib.sha256(trace.read_bytes()).hexdigest()==meta['wireSha256']
assert [b['tick']for b in meta['batches']]==[0,500]
states={name:json.loads((visual/f'server-door-{name}.json').read_text())for name in ['closed','opened','restored']}
expected={'closed':([0,3207,3217,0,12348,0,1],[1026,16416,16416],2),'opened':([0,3207,3218,0,10327,0,2],[0,4104,82080],1),'restored':([0,3207,3217,0,12348,0,1],[1026,16416,16416],0)}
for name,state in states.items():
    location,flags,remaining=expected[name]
    assert [v for v in state['locations']if v[4]in[12348,10327]]==[location],name
    cells={(p,x,y):value for p,x,y,value in state['collision_flags']}
    assert [cells[(0,x,y)]for x,y in [(3207,3217),(3207,3218),(3208,3218)]]==flags,name
    assert state['replay_frames_remaining']==remaining,name
assert states['closed']['locations']==states['restored']['locations']
assert states['closed']['collision_flags']==states['restored']['collision_flags']
report=dict(trace_sha256=meta['wireSha256'],server_batches=meta['batches'],client_states=states,source_flags='Original joined JVM scene readbacks runs/scene-oracle/door-flags.txt; full native/server allocated-flag hashes matched in scene audit',scope='Actual native-server packet trace consumed by native renderer. Receipt playback is accelerated; tick0/tick500 timing verified by server audit, not wall-clock playback. This is not live world entry.')
(visual/'server-door-verification.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print('Native renderer: exact server door placement/rotation, all three affected collision cells, and complete nearby restoration match.')
