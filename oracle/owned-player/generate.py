"""Original InventoryListener and Settings with actual distinct player receivers."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,subprocess
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).resolve().parent;O=R/'runs/owned-player-oracle'
spec=importlib.util.spec_from_file_location('bound',R/'oracle/player-inventory/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.c
def requests():
 rows=[]
 def add(**kwargs):
  row=dict(weights={'1':4.25,'2':-5.0,'12047':1.75},A=dict(inventory=[dict(id=1,amount=100)],equipment={'0':dict(id=2)},had=False,counter=12),B=dict(inventory=[dict(id=1,amount=3)],equipment={},had=False,counter=31),target='A',listener='B',equipmentTarget=False,op='add',item=dict(id=12047));row.update(kwargs);rows.append(copy.deepcopy(row))
 for target in ['A','B']:
  for listener in ['A','B']:
   for eq in [False,True]:
    for op in ['refresh','add','replace','clear']:
     for had in [False,True]:
      add(target=target,listener=listener,equipmentTarget=eq,op=op)
      rows[-1][listener]['had']=had
 for value in [None,0.0,-2.5,dict(bits='7ff0000000000000'),dict(bits='fff0000000000000'),dict(bits='7ff8000000000001'),dict(integer=3)]:
  for op in ['refresh','add','clear']:
   add(op=op,weights={'1':value,'2':-5.0,'12047':1.75})
 for counter in [-2147483648,-1,2147483647]:
  add();rows[-1]['B']['counter']=counter
 return rows
def generate(check):
 baseline=R/'fixtures/player-inventory/oracle.jsonl';before=hashlib.sha256(baseline.read_bytes()).hexdigest();base.generate(True);assert hashlib.sha256(baseline.read_bytes()).hexdigest()==before
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True)
 # The parent generated adapter contains the same pinned verbatim Settings,
 # pouch/counter/Appearance methods; only Session.write gains a receiver tag.
 source=(base.O/'adapters/core/game/node/entity/player/Player.java').read_text()
 assert source.count('new Session()')==1 and source.count('public static class Session{')==1 and source.count('oracle.BoundOracle.frame(buffer);')==1
 source=source.replace('new Session()','new Session(this)').replace('public static class Session{','public static class Session{private final Player player;public Session(Player p){player=p;}').replace('oracle.BoundOracle.frame(buffer);','oracle.OwnedOracle.frame(player,buffer);')
 adapter=O/'adapters/core/game/node/entity/player/Player.java';adapter.parent.mkdir(parents=True,exist_ok=True);adapter.write_text(source,encoding='utf-8')
 driver=H/'OwnedOracle.java';c.read(driver)
 cp=os.pathsep.join(map(str,[classes,base.O/'classes',R/'runs/inventory-listener-oracle/classes',R/'runs/inventory-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar']))
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,adapter,driver])
 cases=requests();data=''.join(json.dumps(row)+'\n'for row in cases).encode();(O/'input.jsonl').write_bytes(data)
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 p=subprocess.run([str(exe),'-Dfile.encoding=UTF-8','-cp',cp,'oracle.OwnedOracle'],cwd=R,input=data,capture_output=True,timeout=120)
 (O/'stderr.log').write_bytes(p.stderr);assert p.returncode==0,p.stderr.decode(errors='replace')
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();c.read(pathlib.Path(__file__))
 provenance=dict(format=1,upstream=json.loads(c.read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),unchangedPlayerInventoryBaseline=before,scope='Complete original InventoryListener, Container/Event/Item and packet/context/writer classes from freshly reproduced player-inventory assembly, plus verbatim Settings.updateWeight/pouch/counter/Appearance.sync. Two distinct recording Player adapters have actual final containers/settings/counters. InventoryListener is attached to its own or another player inventory/equipment data container; exact receiver-tagged frames, weights, pouch flags, syncs and partial errors compare. EquipmentContainer subclass, full Player/profile/lifecycle/GC/content, world startup and accepted gameplay remain separate; native world lifetime tests do not run JVM GC.')
 folder=R/'fixtures/owned-player';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text())==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
 print(len(rows),'original distinct-player receiver scenarios reproduced'if check else'original distinct-player receiver scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
