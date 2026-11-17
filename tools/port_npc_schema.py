"""Author/check the native stored-field schema from the pinned NpcDefinition."""
import argparse,pathlib,re
ROOT=pathlib.Path(__file__).resolve().parents[1]
SOURCE=ROOT/'reference/server/Server/src/main/core/cache/def/impl/NPCDefinition.java'
def snake(s):return re.sub(r'([a-z0-9])([A-Z])',r'\1_\2',re.sub(r'([A-Z]+)([A-Z][a-z])',r'\1_\2',s)).lower()
def generate():
 source=re.sub(r'/\*.*?\*/|//[^\n]*','',SOURCE.read_text(encoding='utf-8'),flags=re.S)
 header,constructor=source.split('public NPCDefinition(int id)',1);constructor=constructor[:constructor.index('\n\t}')]
 fields=[('int','id'),('String','name'),('String','examine'),('String[]','options'),('Map','handlers')];defaults={}
 for m in re.finditer(r'^\s*(?:(?:private|public|protected)\s+)?(?P<static>static\s+)?(?P<type>int\[\]\[\]|(?:int|short|byte|String)\[\]|int|short|byte|boolean|String|Object)\s+(?P<name>\w+)\s*(?:=\s*(?P<value>[^;]+))?;',header,re.M):
  if not m['static']:
   fields.append((m['type'],m['name']))
   if m['value'] is not None:defaults[m['name']]=m['value'].strip()
 fields.append(('Graphics[]','combatGraphics'))
 for name,value in re.findall(r'(\w+)\s*=\s*([^;]+);',constructor):defaults[name]=value.strip()
 types={'int':'i32','short':'i16','byte':'i8','boolean':'bool','String':'Option<String>','Object':'Option<serde_json::Value>','Map':'Shared<BTreeMap<String, Property>>','String[]':'Option<Shared<Vec<Option<String>>>>','int[]':'Option<Shared<Vec<i32>>>','short[]':'Option<Shared<Vec<i16>>>','byte[]':'Option<Shared<Vec<i8>>>','int[][]':'crate::definitions::item::IntegerRows','Graphics[]':'Option<Shared<Vec<Option<Shared<super::Graphics>>>>>'}
 def initial(kind,name):
  value=defaults.get(name)
  if kind=='Map':return'Shared::new(BTreeMap::new())'
  if name=='id':return'0'
  if kind=='Graphics[]':return'Some(Shared::new(vec![None; 3]))'
  if value=='new int[0]':return'Some(Shared::new(vec![]))'
  if value=='new String[5]':return'Some(Shared::new(vec![None; 5]))'
  if value and value.startswith('"'):return'Some('+value+'.into())'
  if value and value!='null':return re.sub(r'\((?:byte|short)\)\s*','',value)
  return'None'if kind.endswith('[]')or kind in ['Object','String']else'false'if kind=='boolean'else'0'
 data='// Authored by tools/port_npc_schema.py; source field names are retained in observations.\nuse super::{Shared, Property};\nuse serde::{Serialize, Deserialize};\nuse std::collections::BTreeMap;\n#[derive(Debug, Serialize, Deserialize)]\n#[serde(default)]\npub struct NpcDefinition {\n'
 for kind,name in fields:data+=f'    #[serde(rename = "{name}")]\n    pub {snake(name)}: {types[kind]},\n'
 # dropTables is the native drop-table holder, not a stored cache/configuration
 # field: it stays out of `fields`, so no observation or helper names it.
 if not re.search(r'private\s+NPCDropTables\s+dropTables\s*=\s*new\s+NPCDropTables\(this\);',header):raise RuntimeError('dropTables declaration differs')
 data+='    /// `dropTables`: never null in a new definition. It is not a cache or\n    /// configuration field, so observations leave it out.\n    #[serde(skip)]\n    pub drop_tables: Option<crate::world::drop_tables::NpcDropTables>,\n'
 data+='}\nimpl Default for NpcDefinition {\n    fn default() -> Self {\n        Self {\n'+''.join(f'            {snake(name)}: {initial(kind,name)},\n'for kind,name in fields)+'            drop_tables: Some(crate::world::drop_tables::NpcDropTables::new()),\n        }\n    }\n}\n'
 # Keep raw schema authoring separate from rustfmt so --check does not depend on
 # a particular editor or host newline convention.
 helper='// Authored by tools/port_npc_schema.py; differential observation only.\nuse rs09_server::definitions::npc::Handle;\nuse serde_json::{Value, json};\nuse std::collections::BTreeMap;\npub fn set(h:&Handle, field:&str, value:&Value) {let mut h=h.write();match field {\n'
 for kind,name in fields:
  target=f'h.{snake(name)}'
  helper+=f'"{name}" => '+(f'*{target}.write() = serde_json::from_value(value.clone()).unwrap(),'if kind=='Map'else f'{target} = serde_json::from_value(value.clone()).unwrap(),')+'\n'
 helper+='_=>panic!("unknown field {field}"),}}\npub fn references(h:&Handle, ids:&mut BTreeMap<usize,usize>)->Value {fn tag<T>(v:&rs09_server::definitions::item::Shared<T>,ids:&mut BTreeMap<usize,usize>)->Value {let next=ids.len();json!(*ids.entry(v.identity()).or_insert(next))}let mut out=BTreeMap::new();out.insert("self",tag(h,ids));let h=h.read();\n'
 for kind,name in sorted(fields,key=lambda v:v[1]):
  if kind=='Map':helper+=f'out.insert("{name}",tag(&h.{snake(name)},ids));\n'
  elif kind.endswith('[]'):helper+=f'out.insert("{name}",h.{snake(name)}.as_ref().map_or(Value::Null,|v|tag(v,ids)));\n'
 helper+='json!(out)}\n'
 helper+='pub fn share(from:&Handle,to:&Handle,field:&str) { match field {\n'
 for kind,name in fields:
  if kind.endswith('[]'):helper+=f'"{name}"=>{{let a=from.read().{snake(name)}.clone();to.write().{snake(name)}=a;}},\n'
 helper+='_=>panic!("unknown shared field {field}"),}}\n'
 helper+='pub fn mutate(h:&Handle,field:&str,index:usize,value:&Value)->Result<(),rs09_server::definitions::npc::Error> {use rs09_server::definitions::npc::Error;let h=h.read();match field {\n'
 for kind,name in fields:
  if kind.endswith('[]')and kind not in ['int[][]','Graphics[]']:
   leaf={'byte[]':'value.as_i64().unwrap() as i8','short[]':'value.as_i64().unwrap() as i16','int[]':'value.as_i64().unwrap() as i32','String[]':'value.as_str().map(str::to_owned)'}[kind]
   helper+=f'"{name}"=>{{let a=h.{snake(name)}.as_ref().ok_or(Error::Null)?;*a.write().get_mut(index).ok_or(Error::Index)?={leaf};}},\n'
 helper+='_=>panic!("unknown mutable field {field}"),}Ok(())}\n'
 return {ROOT/'crates/server/src/definitions/npc/data.rs':data,ROOT/'oracle/npc-definitions/fields.rs':helper}
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');a=p.parse_args()
 for path,text in generate().items():
  if a.check:
   # Compare after the project's rustfmt, without changing saved files.
   import subprocess
   formatted=subprocess.run(['rustfmt','--edition','2024','--emit','stdout'],input=text.encode(),capture_output=True,check=True).stdout.decode().replace('\r','')
   if path.read_text(encoding='utf-8').replace('\r','')!=formatted:raise RuntimeError(f'NPC schema differs: {path}')
  else:path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8')
 print('NPC schema checked.'if a.check else'NPC schema authored.')
