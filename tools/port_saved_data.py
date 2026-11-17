"""Author native record fields and ordered parse/save code from the pinned source.

This is an offline migration aid, never a runtime generator. JVM fixtures are
independent: they execute the original classes and saver methods unchanged.
"""
import pathlib,re
ROOT=pathlib.Path(__file__).resolve().parents[1]
SOURCE=ROOT/'reference/server/Server/src/main/core/game/node/entity/player/link'
OUT=ROOT/'crates/server/src/player/saved_data/records.rs'
def snake(name):return re.sub(r'(?<!^)(?=[A-Z])','_',name).lower()
def block(text,start):
    opening=text.index('{',text.index(start));depth=1;end=opening+1
    while depth:depth+=(text[end]=='{')-(text[end]=='}');end+=1
    return text[opening+1:end-1]
def generate():
    out=['// Authored from pinned ActivityData, QuestData and GlobalData; see tools/port_saved_data.py.','use super::{Error,Result,ItemData,json};','use serde::Serialize;','use serde_json::{Value,Map};']
    for name in ['ActivityData','QuestData','GlobalData']:
        text=(SOURCE/(name+'.java')).read_text();clean=re.sub(r'/\*.*?\*/','',text,flags=re.S)
        declarations=clean[:clean.index('public '+name+'(')] if 'public '+name+'(' in clean else clean[:clean.index('public void parse')]
        fields=re.findall(r'^\s*(?:private\s+)?(?:final\s+)?(int|long|byte|boolean|double|Item)(\[\])?\s+(\w+)\s*(?:=\s*([^;]+))?;',declarations,re.M)
        assert fields,(name,'no fields')
        out+=['#[derive(Clone,Debug,Serialize)]','#[serde(rename_all="camelCase")]','pub struct '+name+' {']
        kinds={}
        for kind,array,key,initial in fields:
            ty={'int':'i32','long':'i64','byte':'i8','boolean':'bool','double':'f64','Item':'Option<ItemData>'}[kind]
            kinds[key]=(kind,bool(array));out.append('pub '+snake(key)+':'+('Vec<'+ty+'>' if array else ty)+',')
        out+=['}','impl Default for '+name+' {fn default()->Self {Self {']
        for kind,array,key,initial in fields:
            default={'boolean':'false','double':'0.0','Item':'None'}.get(kind,'0')
            if array:
                count=int(re.search(r'\[(\d+)\]',initial)[1]);value='vec!['+default+';'+str(count)+']'
                if key=='draynorLever':value='vec![true;6]'
                if key=='desertTreasure':value='vec!['+','.join('Some(ItemData::new('+str(id)+','+str(amount)+'))'for id,amount in [(1513,12),(592,10),(1775,6),(2353,6),(526,2),(973,2),(565,1)])+']'
            else:value=initial or default
            out.append(snake(key)+':'+value+',')
        out+=['}}}','impl '+name+' {','pub fn parse(&mut self,data:&Value)->Result<()> {let data=json::object(data)?;']
        parse=block(clean,'public void parse(JSONObject data)')
        pattern=re.compile(r'\s*(?:(\w+)\s*=\s*(Integer.parseInt|Long.parseLong|Byte.parseByte|Double.parseDouble)\(\s*data.get\("([^"]+)"\).toString\(\)\);|(\w+)\s*=\s*\(boolean\)\s*data.get\("([^"]+)"\);|JSONArray (\w+)\s*=\s*\(JSONArray\) data.get\("([^"]+)"\);\s*for\s*\(int i\s*=\s*0\s*;\s*i\s*<\s*\6.size\(\)\s*;\s*i\+\+\)\s*\{(.*?)\})',re.S)
        keys=[];cursor=0
        for match in pattern.finditer(parse):
            assert not parse[cursor:match.start()].strip(),(name,parse[cursor:match.start()]);cursor=match.end()
            field,converter,key,bfield,bkey,var,akey,body=match.groups()
            if field:
                kind=kinds[field][0];convert={'int':'int','long':'long','byte':'byte','double':'double'}[kind]
                out.append('self.'+snake(field)+'=json::'+convert+'(json::required(data,"'+key+'")?)?;');keys.append((key,field))
            elif bfield:
                out.append('self.'+snake(bfield)+'=json::boolean(json::required(data,"'+bkey+'")?)?;');keys.append((bkey,bfield))
            else:
                field=re.search(r'(\w+)\[i\]\s*=',body)[1];kind=kinds[field][0]
                if kind=='Item':out.append('json::items(data,"'+akey+'",&mut self.'+snake(field)+')?;')
                else:out.append('json::array(data,"'+akey+'",&mut self.'+snake(field)+',json::'+{'int':'int','long':'long','boolean':'boolean','double':'double'}[kind]+')?;')
                keys.append((akey,field))
        assert not parse[cursor:].strip(),(name,parse[cursor:])
        out+=['Ok(())}','pub fn save(&self)->Result<Value>{let mut data=Map::new();']
        for key,field in keys:
            kind,array=kinds[field];expr='self.'+snake(field)
            if array:
                if kind=='Item':value='Value::Array('+expr+'.iter().map(|item|{let item=item.as_ref().ok_or(Error::Null)?;Ok(serde_json::json!({"id":item.id().to_string(),"amount":item.amount.to_string()}))}).collect::<Result<Vec<_>>>()?)'
                elif kind=='boolean':value='serde_json::json!('+expr+')'
                else:value='Value::Array('+expr+'.iter().map(|value|Value::String('+('rs09_game_core::java_double::format(*value)'if kind=='double'else'value.to_string()')+')).collect())'
            else:value='Value::Bool('+expr+')'if kind=='boolean'else'Value::String('+expr+'.to_string())'
            out.append('data.insert("'+key+'".into(),'+value+');')
        out+=['Ok(Value::Object(data))}','}']
    OUT.parent.mkdir(parents=True,exist_ok=True);OUT.write_text('\n'.join(out)+'\n',encoding='utf-8')
if __name__=='__main__':generate()
