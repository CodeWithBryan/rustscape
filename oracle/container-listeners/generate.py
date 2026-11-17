"""Original Container with distinct listener objects, named aliases and reentry."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,random,subprocess
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).resolve().parent;O=R/'runs/container-listeners-oracle'
spec=importlib.util.spec_from_file_location('ownership',R/'oracle/container-ownership/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.c

def op(command,**kw):return dict(op=command,**kw)
def requests():
 template=copy.deepcopy(base.requests()[0]);template['containers']=[dict(id=0,capacity=4,type='DEFAULT',sort='ID',listeners=[])];template.pop('scripts',None)
 rows=[]
 def add(actions,**kw):
  row=copy.deepcopy(template);row.update(kw);row['actions']=actions;rows.append(row)
 init=[op('newListener',name='a',label=7),op('aliasListener',name='a',as_='b'),op('newListener',name='c',label=7),op('newListener',name='d',label=8),op('registerNamed',name='a'),op('registerNamed',name='c'),op('registerNamed',name='b'),op('captureListeners')]
 for name in ['a','b','c','d','missing']:
  add(init+[op('sameListener',name='a',other=name),op('unregisterNamed',name=name),op('listValues'),op('refresh'),op('newListener',name=name,label=7),op('unregisterNamed',name=name),op('refreshNamed',name=name),op('retireListener',name=name),op('unregisterNamed',name=name),op('refreshNamed',name=name)])
 for event in ['update','refresh']:
  for action in [op('listSetNamed',index=1,name='d'),op('listSetNamed',index=1,name='missing'),op('listUnregisterNamed',name='a'),op('listAddNamed',name='c'),op('newListener',name='a',label=8),op('retireListener',name='a'),op('refreshNamed',name='a'),op('fail')]:
   add(init+[op('add',item='b',fire=False),op(event),op('sameListener',name='a',other='b'),op('listValues'),op('refresh')],scripts={'7':[dict(when=event,at=1,action=action)]})
 for index in [-1,0,1,2,3,2147483647]:
  for name in ['a','c','missing']:
   add(init+[op('listSetNamed',index=index,name=name),op('listValues'),op('listUnregisterNamed',name=name),op('refresh')])
 add(init+[op('retireListener',name='a'),op('retireListener',name='b'),op('retireListener',name='c'),op('retireContainer',value=0),op('listAddNamed',name='d'),op('listValues'),op('listUnregisterNamed',name='d'),op('listValues')])
 rng=random.Random(530)
 choices=[op('newListener',name=n,label=v)for n in ['a','b','c','d']for v in [7,8]]
 choices += [op(name,name=n)for name in ['registerNamed','unregisterNamed','refreshNamed','retireListener','listAddNamed','listUnregisterNamed']for n in ['a','b','c','d','missing']]
 choices += [op('listSetNamed',index=i,name=n)for i in [0,1,2,3]for n in ['a','c','missing']]
 choices += [op('aliasListener',name=n,as_='b')for n in ['a','b','c','missing']]+[op('refresh'),op('listClear'),op('listValues')]
 for _ in range(64):add(init+[copy.deepcopy(rng.choice(choices))for _ in range(16)])
 return rows

EXTRA='''
   case "newListener":namedListeners.put((String)a.get("name"),new Listener(integer(a,"label",0)));return null;
   case "aliasListener":namedListeners.put((String)a.get("as_"),namedListeners.get((String)a.get("name")));return null;
   case "retireListener":namedListeners.remove((String)a.get("name"));return null;
   case "sameListener":return namedListeners.get((String)a.get("name"))==namedListeners.get((String)a.get("other"));
   case "registerNamed":c.register(namedListeners.get((String)a.get("name")));return null;
   case "unregisterNamed":return c.getListeners().remove(namedListeners.get((String)a.get("name")));
   case "refreshNamed":c.refresh(namedListeners.get((String)a.get("name")));return null;
   case "listAddNamed":exposedLists.get(integer(a,"list",0)).add(namedListeners.get((String)a.get("name")));return null;
   case "listSetNamed":exposedLists.get(integer(a,"list",0)).set(integer(a,"index",0),namedListeners.get((String)a.get("name")));return null;
   case "listUnregisterNamed":return exposedLists.get(integer(a,"list",0)).remove(namedListeners.get((String)a.get("name")));
'''

def generate(check):
 before={p:hashlib.sha256(p.read_bytes()).hexdigest()for p in [R/'fixtures/inventory/oracle.jsonl',R/'fixtures/container-ownership/oracle.jsonl']}
 base.generate(True);assert all(hashlib.sha256(p.read_bytes()).hexdigest()==v for p,v in before.items())
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True)
 source=c.read(R/'oracle/inventory/InventoryOracle.java')
 assert source.count('public class InventoryOracle {')==source.count('case "fail":')==source.count('request=value;containers=')==1
 source=source.replace('public class InventoryOracle {','public class InventoryOracle {static Map<String,Listener> namedListeners;static List<List<ContainerListener>> exposedLists;').replace('case "fail":',base.EXTRA+EXTRA+'case "fail":').replace('request=value;containers=','request=value;namedListeners=new TreeMap<>();exposedLists=new ArrayList<>();containers=')
 driver=O/'adapters/oracle/InventoryOracle.java';driver.parent.mkdir(parents=True,exist_ok=True);driver.write_text(source,encoding='utf-8')
 cp=os.pathsep.join(map(str,[classes,R/'runs/inventory-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar']))
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,driver])
 cases=requests();data=''.join(json.dumps(x)+'\n'for x in cases).encode();(O/'input.jsonl').write_bytes(data)
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 result=subprocess.run([str(exe),'-Xmx768m','-Dfile.encoding=UTF-8','-cp',cp,'oracle.InventoryOracle'],cwd=R,input=data,capture_output=True,timeout=120)
 (O/'stderr.log').write_bytes(result.stderr);assert result.returncode==0,result.stderr.decode(errors='replace')
 rows=[json.loads(line)for line in result.stdout.splitlines()if line];assert len(rows)==len(cases)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();c.read(pathlib.Path(__file__))
 provenance=dict(format=1,upstream=json.loads(c.read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),commandSnapshots=sum(len(x['expected'])for x in rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),unchangedBaselines={p.relative_to(R).as_posix():v for p,v in before.items()},scope='Complete unchanged original Item/Container/Event/Listener interface and enums, freshly checked389-case inventory and207-case ownership baselines. Recording driver adds distinct real listener instances with equal labels, named aliases, retirement/rebinding and exposed list edits. Callback receiver identity, selective removal, direct refresh, late reads/reentry and failed prefixes are compared. No full Player/profile, listener graph collection, bank/equipment subclasses or accepted gameplay claim.')
 folder=R/'fixtures/container-listeners';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text())==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
 print(len(rows),'original native-listener scenarios reproduced'if check else'original native-listener scenarios generated',provenance['commandSnapshots'],'snapshots')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
