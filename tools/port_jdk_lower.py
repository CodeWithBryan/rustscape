"""Author native JDK 11 ROOT casing/word data from reviewed exhaustive observations."""
import argparse,json,pathlib
ROOT=pathlib.Path(__file__).resolve().parents[1];data=json.loads((ROOT/'fixtures/string-case/data.json').read_text(encoding='utf-8'))
text='// Authored from exhaustive JDK 11 observations in fixtures/string-case.\n// tools/port_jdk_lower.py --check; no runtime Java or code generation.\n'
text+='#[rustfmt::skip]\npub(super) const LOWER: &[(u32, &str)] = &[\n'+''.join(f'    (0x{cp:x}, "'+''.join('\\u{'+f'{ord(c):x}'+'}'for c in value)+'"),\n'for cp,value in data['lowerPairs'])+'];\n'
for name,kind,values,render in [('WORDS','(u32, u32, i32)',data['wordRanges'],lambda v:'('+', '.join(map(str,v))+')'),('CASED','(u32, u32)',data['casedRanges'],lambda v:'('+', '.join(map(str,v))+')'),('STATES','usize',data['states'],str),('END','bool',data['end'],lambda v:str(v).lower()),('LOOK','bool',data['lookahead'],lambda v:str(v).lower())]:
    text+=f'#[rustfmt::skip]\npub(super) const {name}: &[{kind}] = &[\n'+''.join('    '+render(v)+',\n'for v in values)+'];\n'
text+=f'pub(super) const CATEGORIES: usize = {data["categories"]};\n'
text+='#[rustfmt::skip]\npub(super) const BACK: &[usize] = &[\n'+''.join('    '+str(v)+',\n'for v in data['backwards'])+'];\n'
path=ROOT/'crates/game-core/src/java_lower_data.rs';p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args()
if a.check:
    if path.read_text(encoding='utf-8')!=text:raise RuntimeError('Native ROOT casing data differs')
    print(f'{len(data["lowerPairs"])} lower mappings and complete ROOT word/cased data match.')
else:path.write_text(text,encoding='utf-8',newline='\n');print('Authored native ROOT casing data.')
