"""Offline authoring of the pinned ItemConfigParser public string constants."""
import argparse,pathlib,re
ROOT=pathlib.Path(__file__).resolve().parents[1]
source=(ROOT/'reference/server/Server/src/main/core/game/system/config/ItemConfigParser.kt').read_text()
pairs=re.findall(r'const val (\w+) = "([^"]*)"',source)
text='// Authored from the pinned ItemConfigParser.kt; runtime never invokes Kotlin.\n'
text+=''.join(f'pub const {key}: &str = "{value}";\n'for key,value in pairs)
text+='pub const ALL: &[(&str, &str)] = &[\n'+''.join(f'    ("{key}", {key}),\n'for key,_ in pairs)+'];\n'
target=ROOT/'crates/server/src/definitions/item_config/constants.rs'
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args()
if a.check:
    if target.read_text()!=text:raise RuntimeError('Native item constants differ from pinned source')
    print(f'{len(pairs)} native item constants match source.')
else:target.write_text(text,encoding='utf-8',newline='\n');print(f'Wrote {len(pairs)} item constants.')
