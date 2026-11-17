"""Author the native configuration schema from the pinned assignment data.

This is source-authoring tooling, not a runtime dependency or an expected-output
oracle. The independent JVM oracle executes the complete original parser.
"""
import pathlib
import re

ROOT=pathlib.Path(__file__).resolve().parents[1]
SERVER=ROOT/'reference/server/Server/src/main'
parser=(SERVER/'core/game/system/config/ServerConfigParser.kt').read_text()
constants=(SERVER/'core/ServerConstants.kt').read_text()
defaults={name:value.split(' //')[0].strip() for name,value in re.findall(r'var (\w+)(?:\s*:[^=\n]+)?\s*=\s*([^\n]+)',constants)}
def rust(value):
    if value=='null':return 'Initial::Null'
    if value in ['true','false']:return 'Initial::Bool('+value+')'
    if value.startswith('"'):return 'Initial::Text('+value+')'
    if re.fullmatch(r'-?\d+',value):return 'Initial::Int('+value+')'
    location=re.fullmatch(r'Location\((-?\d+),\s*(-?\d+),\s*(-?\d+)\)',value)
    if location:return 'Initial::Location(['+', '.join(location.groups())+'])'
    if value=='LogLevel.VERBOSE':return 'Initial::Text("VERBOSE")'
    raise RuntimeError('Review nonliteral initializer: '+value)

rules=[]
for line in parser.split('private fun parseServerSettings(){')[1].split('private fun Toml.getPath')[0].splitlines():
    match=re.search(r'ServerConstants\.(\w+) = (.*)',line)
    if not match:continue
    field,expression=match.groups()
    if field=='LOG_LEVEL':kind='Kind::LogLevel';key='server.log_level'
    elif field=='DATABASE':kind='Kind::Database';key=''
    elif field=='SERVER_GE_NAME':kind='Kind::ServerGeName';key='world.name_ge'
    else:
        key=re.search(r'data\.get\w+\("([^"]+)"',expression).group(1)
        fallback=re.search(r',\s*(true|false|-?\d+L?|"[^"]*")\)',expression)
        fallback=fallback.group(1) if fallback else None
        if 'parseLocation' in expression:kind='Kind::Location'
        elif 'getPath' in expression:kind='Kind::Path'
        elif 'getBoolean' in expression:kind='Kind::Bool('+('None' if fallback is None else 'Some('+fallback+')')+')'
        elif 'getLong' in expression:kind='Kind::Int('+('None' if fallback is None else 'Some('+fallback.removesuffix('L')+')')+')'
        elif 'getString' in expression:kind='Kind::Text('+('None' if fallback is None else 'Some('+fallback+')')+')'
        else:raise RuntimeError('Review unrecognized config assignment: '+line)
    rules.append(f'    Rule {{ field: "{field}", key: "{key}", kind: {kind}, initial: {rust(defaults[field])} }},')
target=ROOT/'crates/server/src/config/schema.rs';target.parent.mkdir(parents=True,exist_ok=True)
target.write_text('// Generated assignment data from the pinned ServerConfigParser.\n// Review with JVM configuration fixtures; do not infer whole-class parity.\nuse super::{Initial, Kind, Rule};\npub(super) const SERVER: &[Rule] = &[\n'+'\n'.join(rules)+'\n];\n',encoding='utf-8')
print('Authored',len(rules),'ordered server assignments.')
