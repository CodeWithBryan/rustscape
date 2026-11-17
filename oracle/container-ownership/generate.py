"""Original complete Container/Item with aliased receivers and exposed listener lists."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/container-ownership-oracle'
spec=importlib.util.spec_from_file_location('inventory',ROOT/'oracle/inventory/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
c=base.compiler;read,run=base.read,base.run
def op(name,**kw):return dict(op=name,**kw)
def via(action,container=None):
 if container is not None:action=dict(action,container=container)
 return op('via',action=action)
def requests():
 template=next(x for x in base.requests()if x['kind']=='sequence'and x['containers']and x['containers'][0]['capacity']==4 and x['containers'][0]['type']=='DEFAULT'and x['containers'][0]['sort']=='ID')
 rows=[]
 def add(actions,**kw):
  row=copy.deepcopy(template);row.update(kw);row['actions']=actions;rows.append(row)
 init=[op('aliasContainer',value=10),op('aliasContainer',container=10,value=11),op('captureListeners'),op('sameContainer',source=10),op('sameList',container=10,list=0)]
 for kind in ['DEFAULT','SHOP','ALWAYS_STACK','NEVER_STACK']:
  for sort in ['ID','HASH']:
   for capacity in [0,1,4]:
    add(init+[op('add',item='b',fire=False),op('captureEvent',container=10),op('captureArray',container=11),op('clear',container=10,fire=False),op('add',container=11,item='c',fire=False),op('update'),op('copy',source=11),op('add',container=10,item='a'),op('retireContainer',value=0),op('newContainer',value=0,capacity=2,type='DEFAULT',sort='ID'),op('sameContainer',source=10),op('add',item='d'),op('get',container=10,slot=0,as_='old'),op('itemAmount',item='old',value=17),op('listValues',list=0)],containers=[dict(id=0,capacity=capacity,type=kind,sort=sort,listeners=[0,1])])
 changes=[via(op('clear',fire=False),10),via(op('clear',fire=True),11),via(op('update',force=True),10),via(op('replace',slot=0,item='c',fire=False),11),via(op('copy',source=11),10),op('listAdd',list=0,value=2),op('listRemove',list=0,value=1),op('listUnregister',list=0,value=0),op('listSet',list=0,index=1,value=2),op('listSet',list=0,index=1,value=None),op('listClear',list=0),via(op('retireContainer',value=0)),via(op('newContainer',value=0,capacity=1,type='DEFAULT',sort='ID')),op('fail')]
 for event in ['update','refresh']:
  for callback in changes:
   for fire in [False,True]:
    add(init+[op('add',item='b',fire=False),op('captureArray'),op('captureEvent'),op(event),op('add',container=10,item='c',fire=fire),op('sameContainer',source=10),op('sameList',container=11,list=0),op('listValues',list=0)],scripts={'0':[dict(when=event,at=1,action=callback)]})
 for event in ['update','refresh']:
  for fail in [False,True]:
   actions=[via(op('retireContainer',value=0)),via(op('newContainer',value=0,capacity=3,type='ALWAYS_STACK',sort='HASH')),via(op('add',item='a',fire=False),10)]+([op('fail')]if fail else[])
   add(init+[op('add',item='b',fire=False),op(event),op('sameContainer',source=10),op('add',item='d'),op('update',container=10),op('listValues',list=0)],scripts={'0':[dict(when=event,at=1,action=a)for a in actions]})
 for index in [-1,0,1,2,2147483647]:
  for value in [None,0,1,2]:
   add(init+[op('listSet',list=0,index=index,value=value),op('listValues',list=0),op('listRemove',list=0,value=index),op('listUnregister',list=0,value=value),op('listValues',list=0),op('add',item='b'),op('listClear',list=0),op('listClear',list=0),op('listAdd',list=0,value=2),op('refresh',container=10)])
 add(init+[op('retireContainer',value=0),op('retireContainer',value=10),op('retireContainer',value=11),op('listAdd',list=0,value=2),op('listValues',list=0),op('listUnregister',list=0,value=0),op('listValues',list=0),op('newContainer',value=0,capacity=4,type='DEFAULT',sort='ID'),op('sameList',list=0)])
 for operation in [op('copy',source=0),op('addAll',source=0),op('spaceContainer',source=0)]:
  for replace in [False,True]:
   changes=[via(op('retireContainer',value=0))]+([via(op('newContainer',value=0,capacity=1,type='DEFAULT',sort='ID')),via(op('add',item='c',fire=False),0)]if replace else[])+[operation]
   add(init+[op('add',item='b',fire=False),op('update'),op('sameContainer',container=10,source=0),op('update',container=10)],scripts={'0':[dict(when='update',at=1,action=a)for a in changes]})
 rng=random.Random(530)
 actions=[op('add',container=i,item=v,fire=f)for i in [0,10,11]for v in ['a','b','c','nil']for f in [False,True]]
 actions += [op('copy',container=i,source=j)for i in [0,10,11]for j in [0,10,11]]
 actions += [op('clear',container=i,fire=f)for i in [0,10,11]for f in [False,True]]
 actions += [op('update',container=10),op('captureArray',container=11),op('captureEvent',container=10),op('listAdd',list=0,value=2),op('listAdd',list=0,value=None),op('listUnregister',list=0,value=0),op('listRemove',list=0,value=1),op('listSet',list=0,index=0,value=1),op('listValues',list=0),op('sameContainer',source=10),op('retireContainer',value=0),op('newContainer',value=0,capacity=4,type='DEFAULT',sort='ID')]
 for _ in range(96):add(init+[rng.choice(actions)for _ in range(32)])
 return rows
EXTRA='''
   case "aliasContainer":containers.put(integer(a,"value",0),c);return null;
   case "sameContainer":return c==containers.get(integer(a,"source",0));
   case "retireContainer":containers.remove(integer(a,"value",0));return null;
   case "newContainer":containers.put(integer(a,"value",0),new Container(integer(a,"capacity",4),ContainerType.valueOf((String)a.get("type")),SortType.valueOf((String)a.get("sort"))));return null;
   case "via":return action((Map)a.get("action"),null);
   case "captureListeners":exposedLists.add(c.getListeners());return null;
   case "sameList":return c.getListeners()==exposedLists.get(integer(a,"list",0));
   case "listAdd":exposedLists.get(integer(a,"list",0)).add(a.get("value")==null?null:listener(number(a.get("value"))));return null;
   case "listRemove":exposedLists.get(integer(a,"list",0)).remove(number(a.get("value")));return null;
   case "listSet":exposedLists.get(integer(a,"list",0)).set(integer(a,"index",0),a.get("value")==null?null:listener(number(a.get("value"))));return null;
   case "listClear":exposedLists.get(integer(a,"list",0)).clear();return null;
   case "listUnregister":return exposedLists.get(integer(a,"list",0)).remove(a.get("value")==null?null:listener(number(a.get("value"))));
   case "listValues":{List<Object>out=new ArrayList<>();for(ContainerListener l:exposedLists.get(integer(a,"list",0)))out.add(l==null?null:((Listener)l).id);return out;}
'''
def generate(check):
 old={p:hashlib.sha256(p.read_bytes()).hexdigest()for p in [ROOT/'fixtures/inventory/oracle.jsonl',ROOT/'fixtures/inventory/provenance.json']}
 base.generate(True)
 assert all(hashlib.sha256(p.read_bytes()).hexdigest()==v for p,v in old.items())
 RUN.mkdir(exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
 source=read(ROOT/'oracle/inventory/InventoryOracle.java');assert source.count('public class InventoryOracle {')==1 and source.count('case "fail":')==1 and source.count('request=value;containers=')==1
 source=source.replace('public class InventoryOracle {','public class InventoryOracle { static List<List<ContainerListener>> exposedLists;').replace('case "fail":',EXTRA+'case "fail":').replace('request=value;containers=','request=value;exposedLists=new ArrayList<>();containers=')
 driver=RUN/'adapters/oracle/InventoryOracle.java';driver.parent.mkdir(parents=True,exist_ok=True);driver.write_text(source,encoding='utf-8')
 cp=os.pathsep.join(map(str,[classes,base.RUN/'classes',base.RUN/'json-simple-1.1.1.jar']))
 run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,driver])
 cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8')
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:out=subprocess.run(list(map(str,[exe,'-Xmx768m','-Dfile.encoding=UTF-8','-cp',cp,'oracle.InventoryOracle'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if out.returncode:raise RuntimeError(out.stderr.decode(errors='replace'))
 rows=[json.loads(x)for x in out.stdout.splitlines()if x];assert len(rows)==len(cases)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'),ensure_ascii=True)+'\n'for x in rows).encode();read(pathlib.Path(__file__))
 provenance=dict(format=1,upstream=json.loads(read(ROOT/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),baselineInventoryUnchanged=old[ROOT/'fixtures/inventory/oracle.jsonl'],scope='Complete original unchanged Item/Container/ContainerEvent/ContainerListener and enums from the389-case baseline, plus explicit recording-driver aliases, exposed real ArrayLists and receiver-redirection operations. Compares shared object/array/event state, nested callbacks through different references, registry retirement/reuse, listener structural versus nonstructural mutations and failed prefixes. No native complete player/profile, bank/equipment subtypes, listener-object graph GC or accepted gameplay claim.')
 folder=ROOT/'fixtures/container-ownership';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original shared-container scenarios reproduced;389 baseline cases unchanged.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n');print(f'Generated {len(rows)} original shared-container scenarios;389 baseline cases unchanged.')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
