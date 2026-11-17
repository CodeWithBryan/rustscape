"""Pinned ItemDefinition option lookup/registration with shared native-handler identity."""
import argparse,hashlib,importlib.util,itertools,json,os,pathlib,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/item-option-registry-oracle'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(action,**kw):return dict(op=action,**kw)
def requests():
 rows=[]
 def add(actions,**kw):rows.append(dict(actions=actions,**kw))
 for name,global_value,local_value in itertools.product([None,'take','Take','null','','İΣ😀'],[None,0,1],[None,0,1,'wrong']):
  key='option:'+('null'if name is None else name)
  add([op('register',name=name,handler=global_value),op('config',key=key,value=local_value),op('lookup',name=name),op('register',name=name,handler=1),op('lookup',name=name)])
 for id,null in itertools.product([-2147483648,-1,1,2,995,22937,2147483647],[False,True]):
  add([op('register',name='take',handler=0),op('lookup',id=id,name='take')],nullDefinition=null)
 for name in [None,'take','Take','null','']:
  add([op('capture'),op('register',name=name,handler=None),op('contains',name=name),op('register',name=name,handler=0),op('get',name=name),op('remove',name=name),op('contains',name=name),op('put',name=name,handler=1),op('clear'),op('get',name=name)])
 mutations=[op('fail'),op('set',nullDefinition=True),op('cache',id=1,definition=1),op('config',key='option:take',value=1),op('register',name='take',handler=1),op('clear'),op('lookup',name=None)]
 for event,at,mutation in itertools.product(['lookup','configuration','log'],[1,2],mutations):
  add([op('register',name='take',handler=0),op('lookup',id=22937 if event=='log'else 1,name='take'),op('lookup',name='take')],nullDefinition=event=='log',hooks=[dict(event=event,at=at,actions=[mutation])])
 add([op('capture'),op('capture'),op('put',name='take',handler=0),op('config',key='option:take',value=1),op('lookup',name='take'),op('config',key='option:take',value=None),op('lookup',name='take'),op('register',name=None,handler=2),op('lookup',name=None)])
 return rows
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server';assert run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];run(['git','-C',ref,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
 pin=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=pin['sha256']:
  cached=ROOT/'runs/legacy-interaction-oracle/json-simple-1.1.1.jar'
  if cached.exists()and hashlib.sha256(cached.read_bytes()).hexdigest()==pin['sha256']:data=cached.read_bytes()
  else:
   with urllib.request.urlopen(pin['url'],timeout=60)as response:data=response.read()
  assert len(data)==pin['size']and hashlib.sha256(data).hexdigest()==pin['sha256'];jar.write_bytes(data)
 java=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 server=ref/'Server/src/main';source=read(server/'core/cache/def/impl/ItemDefinition.java')
 methods=''.join(c.method(source,s)for s in ['public static OptionHandler getOptionHandler(int nodeId, String name)','public static boolean setOptionHandler(String name, OptionHandler handler)','public static Map<String, OptionHandler> getOptionHandlers()'])
 fallback=c.method(source,'public static ItemDefinition forId(int itemId)').replace(' forId(', ' fallback(',1)
 adapter('core/cache/def/impl/ItemDefinition.java','''package core.cache.def.impl;
import java.util.*;import core.game.interaction.OptionHandler;import core.tools.Log;import static core.api.ContentAPIKt.log;
public class ItemDefinition{public static final Map<Integer,ItemDefinition>DEFINITIONS=new HashMap<>();private static final Map<String,OptionHandler>OPTION_HANDLERS=new HashMap<>();public final Map<String,Object>handlers;public final int key;
public ItemDefinition(){key=oracle.Support.definitions.size();oracle.Support.definitions.add(this);handlers=new Observed(key);}
static class Observed extends HashMap<String,Object>{int key;Observed(int i){key=i;}public Object get(Object k){Object v=super.get(k);oracle.Support.emit("configuration",key,k);return v;}}
public static ItemDefinition forId(int id){oracle.Support.emit("lookup",id);return oracle.Support.nullDefinition?null:fallback(id);}
'''+fallback+c.method(read(server/'core/cache/def/Definition.java'),'public <V> V getConfiguration(String key)')+methods+'}')
 adapter('core/game/interaction/OptionHandler.java','package core.game.interaction;public class OptionHandler{public final int id;public OptionHandler(int i){id=i;}}')
 adapter('core/tools/Log.java','package core.tools;public enum Log{ERR;}')
 adapter('core/api/ContentAPIKt.java','package core.api;public class ContentAPIKt{public static void log(Class<?>c,core.tools.Log level,String text){oracle.Support.emit("log",c.getName(),level.name(),text);}}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,jar]));run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(row)+'\n'for row in cases),encoding='utf-8')
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as stdin:result=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in result.stdout.splitlines()if line];assert len(rows)==len(cases);assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 data=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Unchanged ItemDefinition getOptionHandler/setOptionHandler/getOptionHandlers and Definition.getConfiguration bodies, original forId fallback body under a recording lookup adapter. Native handler identity, nullable global keys/values, local type failures, case-sensitive keys, shared map aliases, lookup/config/log reentry and failure prefixes. Map content snapshots use authored sorted keys, not HashMap iteration. Java iterator/views/serialization, default locale, plugin execution/startup and live Take remain separate.')
 folder=ROOT/'fixtures/item-option-registry';folder.mkdir(parents=True,exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original item-option registry scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original item-option registry scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
