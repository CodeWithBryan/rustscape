"""Validate actual renderer readbacks for the explicit nearby NPC visual replay."""
import argparse,hashlib,json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1]

def check(path):
    visual=path/'runs/visual';rows=[json.loads(l)for l in (visual/'actor-frames/frames.jsonl').read_text().splitlines()]
    expected=json.loads((ROOT/'fixtures/players/npc-assets/oracle.json').read_text())['root_types']
    phases={}
    for row in rows:
        frame=int(row['file'].split('.')[0])*6;views=row['npc_views']
        assert sorted(v['type']for v in views)==expected,frame
        for v in views:
            target=(3385 if 30<=frame<90 or 150<=frame<210 else None)if v['type']==3384 else v['type']
            assert v['render_type']==target,(frame,v)
        if frame in [0,30,90,150,210,240]:phases[frame]=next(v['render_type']for v in views if v['type']==3384)
    assert len(phases)==6,phases
    final=json.loads((visual/'npc-state.json').read_text());ui=json.loads((visual/'ui-state.json').read_text())
    assert final['diagnostics']==[] and ui['diagnostics']==[] and ui['item_fallbacks']==[]
    assert final['fixture_remaining']==0
    assert len(rows)==41
    images={str((visual/'actor-frames'/r['file']).relative_to(path)):hashlib.sha256((visual/'actor-frames'/r['file']).read_bytes()).hexdigest()for r in rows}
    result=dict(scope='Native GPU capture of synthetic nearby-type display positions and original varbit37 morph phases; no NPC AI/gameplay claim',frames=len(rows),logical_types=expected,morph_render_types_by_frame=phases,diagnostics=final['diagnostics'],ui_diagnostics=ui['diagnostics'],images=images)
    (path/'capture-check.json').write_text(json.dumps(result,indent=2)+'\n')
    print(f'Native NPC display: {len(rows)} GPU frames, {len(expected)} stable logical types, all six morph phase readbacks match, no diagnostics')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('path',type=pathlib.Path);a=p.parse_args();check(a.path)
