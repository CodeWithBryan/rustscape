"""Original listener+Settings.updateWeight+packet writers over actual containers."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,re
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).resolve().parent;O=R/'runs/player-inventory-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('listener',R/'oracle/inventory-listener/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.c
def requests():
 out=[]
 def add(**kw):
  row=dict(op='refresh',weights={'1':2.5,'2':-5.0,'12047':1.75},inventory=[dict(id=1,amount=100),dict(id=2)],equipment={'0':dict(id=2),'12':dict(id=1)},item=dict(id=12047),had=False,counter=0,hooks=[]);row.update(kw);out.append(row)
 for op in ['refresh','add','clear','replace']:
  for had in [False,True]:
   for v in [None,0,1.25,-5.5,dict(bits='8000000000000000'),dict(bits='7ff0000000000000'),dict(bits='fff0000000000000'),dict(bits='7ff8000000000001'),dict(integer=3)]:
    add(op=op,had=had,weights={'1':v,'2':-5.0,'12047':1.75})
   for at in [0,1]:
    for action in ['clear_inventory','clear_equipment','pouch','weight','fail','refresh']:
     add(op=op,had=had,hooks=[dict(at=at,action=action)])
 for counter in [-2147483648,-1,2147483647]:
  add(counter=counter);add(counter=counter,op='add')
 for slot in [0,10,11,12,13]:
  add(equipment={str(slot):dict(id=2)},inventory=[])
 for ident in [1,12047]:
  add(inventory=[dict(id=ident,amount=100,charge=-1)],equipment={},had=True)
 add(inventory=[dict(id=1,amount=0),dict(id=2,amount=-3)],equipment={})
 add(weights={'1':dict(bits='7ff0000000000000'),'2':dict(bits='fff0000000000000'),'12047':1.75})
 add(weights={'1':2.5,'2':dict(bits='7ff8000000000001'),'12047':1.75})
 return out
def generate(check):
 base.generate(True);read=c.read;O.mkdir(parents=True,exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 for name in ['core/game/container/impl/InventoryListener.java','core/net/packet/out/ContainerPacket.java','core/net/packet/context/ContainerContext.java','core/net/packet/out/StringPacket.java','core/net/packet/context/StringContext.java']:
  p=S/name;read(p);java.append(p)
 settings=read(S/'core/game/node/entity/player/link/Settings.java')
 adapter('core/game/node/entity/player/link/Settings.java','package core.game.node.entity.player.link;import core.game.node.entity.player.Player;import core.game.node.item.Item;import core.game.system.config.ItemConfigParser;public class Settings{private final Player player;private double weight;public Settings(Player p){player=p;}'+c.method(settings,'public void updateWeight(')+c.method(settings,'public double getWeight(')+'}')
 config=read(S/'core/game/system/config/ItemConfigParser.kt')
 constants=''.join('public static final String '+name+'='+re.search(r'const val '+name+r' = ("[^"]+")',config).group(1)+';'for name in ['WEIGHT','HIGH_ALCHEMY','LOW_ALCHEMY'])
 adapter('core/game/system/config/ItemConfigParser.java','package core.game.system.config;public class ItemConfigParser{'+constants+'}')
 familiar=read(S/'content/global/skill/summoning/familiar/FamiliarManager.java');manager=read(S/'core/game/node/entity/player/link/InterfaceManager.java');dispatch=read(S/'core/game/node/entity/player/link/PacketDispatch.java');appearance=read(S/'core/game/node/entity/player/link/appearance/Appearance.java')
 player=read(H/'Player.java').replace('/*FAMILIAR*/',c.method(familiar,'public boolean isHasPouch(')+c.method(familiar,'public void setHasPouch(')).replace('/*COUNTER*/',c.method(manager,'public int getPacketCount(')).replace('/*TEXT*/',c.method(dispatch,'public void sendString(')).replace('/*APPEARANCE*/',c.method(appearance,'public void sync('))
 adapter('core/game/node/entity/player/Player.java',player)
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;public class PacketRepository{public static void send(Class<?>type,Context context){if(type==core.net.packet.out.ContainerPacket.class)new core.net.packet.out.ContainerPacket().send((core.net.packet.context.ContainerContext)context);else if(type==core.net.packet.out.StringPacket.class)new core.net.packet.out.StringPacket().send((core.net.packet.context.StringContext)context);else throw new UnsupportedOperationException();}}')
 read(H/'BoundOracle.java');java.append(H/'BoundOracle.java')
 cp=os.pathsep.join(map(str,[classes,R/'runs/inventory-listener-oracle/classes',R/'runs/inventory-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar']))
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 rows=requests();data=''.join(json.dumps(row)+'\n'for row in rows).encode();(O/'input.jsonl').write_bytes(data);exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 p=subprocess.run([str(exe),'-Dfile.encoding=UTF-8','-cp',cp,'oracle.BoundOracle'],cwd=R,input=data,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()];assert len(out)==len(rows);fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in out).encode();read(pathlib.Path(__file__))
 prov=dict(format=1,upstream=json.loads(read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(fixture).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete original InventoryListener/Container/Event/Item/ContainerContext/ContainerPacket/StringContext/StringPacket/IoBuffer, plus verbatim Settings.updateWeight/getWeight, FamiliarManager pouch accessors, Appearance.sync, InterfaceManager.getPacketCount and PacketDispatch.sendString. Actual containers and actual definition configuration objects join listener, weight and wire effects. Player lifecycle/session/dispatch are explicit controlled adapters, zero ISAAC; no EquipmentListener/full familiar/profile/login/session/world/Take claim.')
 folder=R/'fixtures/player-inventory';folder.mkdir(exist_ok=True)
 if check:
  assert(folder/'oracle.jsonl').read_bytes()==fixture
  assert json.loads((folder/'provenance.json').read_text())==prov
 else:
  (folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(prov,indent=2,sort_keys=True)+'\n')
 print(len(out),'original joined player inventory scenarios reproduced'if check else'original joined player inventory scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
