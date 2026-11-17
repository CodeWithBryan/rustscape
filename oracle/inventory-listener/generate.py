"""Unchanged InventoryListener/SummoningPouch/ContainerContext/ContainerPacket."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).resolve().parent;O=R/'runs/inventory-listener-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('inventory',R/'oracle/inventory/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.compiler
def requests():
 out=[]
 def add(**kw):
  row=dict(op='update',had=False,otherHad=False,items=[None,dict(id=1,amount=3),None,None],event=[None,'NULL',dict(id=1,amount=300),None],hooks=[]);row.update(kw);out.append(row)
 for op in ['update','refresh','event','fire']:
  for had in [False,True]:
   for ident in [None,-2147483648,-1,0,1,12047,65535,65536,77583,2147483647]:
    add(op=op,had=had,items=[None,None if ident is None else dict(id=ident,amount=1),None,None])
 for ident in range(12000,12900):
  add(items=[dict(id=ident,amount=0),None,None,None])
 for ident in [14422,14424,14426,14428,14430,14623]:
  add(items=[dict(id=ident,amount=-1),None,None,None])
 for op in ['update','refresh','event']:
  for had in [False,True]:
   for when in ['send','settings','weight','familiar','has','set','appearance','sync']:
    for action in ['clear','pouch','ordinary','swap','had','fail','reenter']:
     add(op=op,had=had,hooks=[dict(when=when,action=action)])
   for action in ['clear','pouch','ordinary','swap','had','fail']:
    add(op=op,had=had,otherHad=True,hooks=[dict(when='familiar',at=2,action=action)])
   for null in ['player','event','settings','familiar1','familiar2','appearance']:
    add(op=op,had=had,null=null,items=[dict(id=12047),None,None,None])
 for op in ['refresh','event']:
  for charge in [-2147483648,-1,0,65535,65536,2147483647]:
   for amount in [-2147483648,-1,0,254,255,2147483647]:
    items=[dict(id=12047,amount=amount,charge=charge),'NULL',None,dict(id=-1)]
    add(op=op,items=items,event=items)
 for op in ['refresh','event']:
  items=[dict(id=0,amount=0),'NULL',None,dict(id=12047)]
  add(op=op,items=items,event=items,eventClear=True)
 return out
def generate(check):
 # Recompile/check complete original Item/Container/Event classes, then compile
 # this oracle in a separate directory; no shared original class is overwritten.
 base.generate(True);read=c.read;O.mkdir(parents=True,exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 for name in ['core/game/container/impl/InventoryListener.java','content/global/skill/summoning/SummoningPouch.java','core/net/packet/Context.java','core/net/packet/OutgoingPacket.java','core/net/packet/context/ContainerContext.java','core/net/packet/out/ContainerPacket.java','core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java']:
  p=S/name;read(p);java.append(p)
 for name in ['core/cache/crypto/ISAACCipher.java','core/cache/misc/buffer/ByteBufferUtils.java']:adapter(name,read(R/'oracle/scenery/adapters'/name))
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;public class PacketRepository{public static void send(Class<?>type,core.net.packet.context.ContainerContext c){oracle.ListenerOracle.send(c);}}')
 adapter('core/game/node/entity/player/Player.java',read(H/'Player.java'))
 read(H/'ListenerOracle.java');java.append(H/'ListenerOracle.java')
 cp=os.pathsep.join(map(str,[classes,R/'runs/inventory-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar']))
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java';cases=requests();data=''.join(json.dumps(row)+'\n'for row in cases).encode();(O/'input.jsonl').write_bytes(data)
 p=subprocess.run([str(exe),'-Dfile.encoding=UTF-8','-cp',cp,'oracle.ListenerOracle'],cwd=R,input=data,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in p.stdout.splitlines()];assert len(rows)==len(cases)
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();read(pathlib.Path(__file__))
 provenance=dict(format=1,upstream=json.loads(read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged InventoryListener, SummoningPouch, ContainerContext, ContainerPacket and IoBuffer; full unchanged Item/Container/Event from freshly reproduced inventory oracle classpath. Required player Settings/Familiar/Appearance/Session/PacketRepository adapters record receiver/callback/null/failure order; ISAAC is zero and external string codecs throw. Real original packet bytes, event arrays/slots, array replacement/retention, manager rebinding, recursive updates and all65536 Item-ID pouch keys are compared. This does not establish full Player/profile/familiar/content, weight implementation, session delivery or authoritative live Take.')
 folder=R/'fixtures/inventory-listener';folder.mkdir(exist_ok=True)
 if check:
  assert(folder/'oracle.jsonl').read_bytes()==fixture
  assert json.loads((folder/'provenance.json').read_text())==provenance
 else:
  (folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n')
 print(len(rows),'original inventory listener scenarios reproduced'if check else'original inventory listener scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
