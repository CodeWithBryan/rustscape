"""Offline table authoring from the reviewed JDK 11 item oracle observations.

The generated native data is checked in. No JVM/code generation is used at runtime.
"""
import argparse,json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1]
case=next(json.loads(line)for line in (ROOT/'fixtures/items/oracle.jsonl').read_text().splitlines()if json.loads(line)['kind']=='casePairs')
text='''// Generated from JDK 11 Character public API observations in fixtures/items.
// Regenerate with tools/port_jdk_case.py; reviewed values, not JDK source code.
pub(super) const FOLD: &[(u16, u16)] = &[\n'''
text+=''.join(f'    (0x{a:04x}, 0x{b:04x}),\n'for a,b in case['expected'])
text+='];\n'
path=ROOT/'crates/game-core/src/java_case.rs'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');args=p.parse_args()
if args.check:
    if path.read_text()!=text:raise RuntimeError('Native JDK case table differs from reviewed oracle output')
    print(f'{len(case["expected"])} JDK 11 case pairs match checked-in native data.')
else:path.write_text(text,encoding='utf-8',newline='\n');print(f'Wrote {len(case["expected"])} JDK 11 case pairs.')
