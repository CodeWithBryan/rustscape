"""Original player appearance, animation, spot and audio payload bytes."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request,re
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/appearance-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 out=[];rng=random.Random(530)
 for gender in [0,1]:
  for size in [0,1,2,8,33,2**31-1]:
   for npc in [-1,0,65535]:out.append(dict(gender=gender,size=size,npcId=npc))
 for pvp in [False,True]:
  for wild in [False,True]:
   for wilderness in [False,True]:
    for disabled in [False,True]:
     for using in [False,True]:out.append(dict(pvp=pvp,wildPvp=wild,wilderness=wilderness,wildernessDisabled=disabled,usingSummoning=using,combatLevel=51,currentCombatLevel=57,summoningCombatLevel=6,totalLevel=2001,wildernessLevel=17))
 for text in ['self','Zezima','a_b__','0123456789012345','☃Rune','\U0001f600abc','İI','_____','']:
  out.append(dict(username=text))
 for _ in range(50):out.append(dict(username='Rune_530',gender=rng.randrange(2),bodyParts=[rng.choice([0,-1,256,32768,65535,2**31-1])for _ in range(12)],colors=[rng.randrange(-1,257)for _ in range(5)],skullIcon=rng.randrange(-1,257),headIcon=rng.randrange(-1,257),renderAnimation=rng.randrange(-1,65537),npcId=rng.choice([-1,0,111]),size=rng.randrange(-1,5),pvp=bool(rng.randrange(2)),wilderness=bool(rng.randrange(2)),wildernessDisabled=bool(rng.randrange(2)),wildPvp=bool(rng.randrange(2)),usingSummoning=bool(rng.randrange(2)),combatLevel=rng.randrange(-1,257),currentCombatLevel=rng.randrange(-1,257),totalLevel=rng.randrange(-1,65537)))
 for ident in [-2**31,-1,0,1,863,866,65535,65536,2**31-1]:
  for delay in [-1,0,1,255,256]:out.append(dict(kind='animation',id=ident,delay=delay))
 for ident in [-1,0,127,65535,65536]:
  for delay in [-1,0,255,256,65535,65536]:
   for area in [False,True]:out.append(dict(kind='audio',id=ident,delay=delay,loops=9,radius=17,location=[-1,8,0]if area else None))
 for ident in [-2**31,-1,0,111,369,65535,65536,2**31-1]:
  for height in [-2**31,-32768,-1,0,1,128,32767,32768,65535,2**31-1]:
   for delay in [-2**31,-65537,-65536,-1,0,1,30,65535,65536,65537,2**31-1]:out.append(dict(kind='spot',id=ident,height=height,delay=delay))
 return out
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 for name in ['core/game/node/entity/player/link/appearance/Gender.java','core/game/node/entity/player/link/appearance/BodyPart.java','core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java']:p=SERVER/name;read(p);java.append(p)
 for name in ['core/cache/crypto/ISAACCipher.java','core/cache/misc/buffer/ByteBufferUtils.java']:adapter(name,read(ROOT/'oracle/scenery/adapters'/name))
 flags=read(SERVER/'core/game/world/update/flag/PlayerFlags530.kt')
 adapter('core/game/world/update/flag/Appearance530.kt','''package core.game.world.update.flag
import core.net.packet.IoBuffer
import core.game.node.entity.player.Player
import core.game.world.GameWorld
import core.tools.StringUtils
import core.game.world.update.flag.context.Animation
import core.game.world.update.flag.context.Graphics
import kotlin.reflect.*
enum class EntityFlag{Appearance,Animate,SpotAnim}
abstract class PlayerFlags530(p:Int,o:Int,f:EntityFlag){abstract fun writeTo(buffer:IoBuffer,context:Any?);fun logInvalidType(context:Any?,expected:KType){throw IllegalArgumentException()}}
'''+c.method(flags,'class Appearance : PlayerFlags530')+'\n'+c.method(flags,'class Animate : PlayerFlags530')+'\n'+c.method(flags,'class SpotAnim : PlayerFlags530'))
 p=SERVER/'core/game/world/update/flag/context/Graphics.java';read(p);java.append(p)
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static Chunk getRegionChunk(Location l){throw new UnsupportedOperationException();}public static class Chunk{public void flag(Object o){throw new UnsupportedOperationException();}}}')
 adapter('core/game/world/update/flag/chunk/GraphicUpdateFlag.java','package core.game.world.update.flag.chunk;public class GraphicUpdateFlag{public GraphicUpdateFlag(core.game.world.update.flag.context.Graphics g,core.game.world.map.Location l){throw new UnsupportedOperationException();}}')
 adapter('core/game/world/update/flag/context/Animation.java','package core.game.world.update.flag.context;public class Animation{private final int id,delay;public Animation(int id,int delay){this.id=id;this.delay=delay;}public int getId(){return id;}public int getDelay(){return delay;}}')
 audio=read(SERVER/'core/net/packet/out/AudioPacket.java')
 adapter('core/net/packet/out/AudioPacket.java','package core.net.packet.out;import core.game.node.entity.player.link.audio.Audio;import core.game.world.map.Location;import core.net.packet.IoBuffer;public class AudioPacket{'+c.method(audio,'public static IoBuffer write(')+'}')
 adapter('core/game/node/entity/player/link/audio/Audio.java','package core.game.node.entity.player.link.audio;public class Audio{public int id,loops,delay,radius;public Audio(int i,int l,int d,int r){id=i;loops=l;delay=d;radius=r;}}')
 adapter('core/game/world/map/Location.java','package core.game.world.map;public class Location{public int x,y;public Location(int x,int y){this.x=x;this.y=y;}public int getChunkOffsetX(){return x&7;}public int getChunkOffsetY(){return y&7;}}')
 app=read(SERVER/'core/game/node/entity/player/link/appearance/Appearance.java')
 methods=['public void prepareBodyData(','public void drawItem(','public void drawClothes(','public void clearBodyPart(','public void flagHatClipping(','public BodyPart getHair()','public BodyPart getBeard()','public BodyPart getTorso()','public BodyPart getArms()','public BodyPart getWrists()','public BodyPart getSkin()','public BodyPart getLegs()','public BodyPart getFeet()','public boolean isMale()']
 adapter('core/game/node/entity/player/link/appearance/App.java','''package core.game.node.entity.player.link.appearance;import core.game.node.entity.player.Player;import core.game.node.item.Item;import core.game.container.impl.EquipmentContainer;import core.game.system.config.ItemConfigParser;public class App{public Gender gender=Gender.MALE;public BodyPart[]appearanceCache=gender.generateCache();public int[]bodyParts=new int[14];public int skullIcon=-1,headIcon=-1,npcId=-1,renderAnimation=1426;private boolean ridingMinecart;public Gender getGender(){return gender;}'''+''.join(c.method(app,s)for s in methods)+'public static class AppearanceCache{public static final int HAIR=0,FACIAL_HAIR=1,TORSO=2,ARMS=3,WRISTS=4,LEGS=5,FEET=6;}}')
 container=read(SERVER/'core/game/container/impl/EquipmentContainer.java');constants=''.join(re.findall(r'public static final int SLOT_[^;]+;',container))
 adapter('core/game/container/impl/EquipmentContainer.java','package core.game.container.impl;public class EquipmentContainer{'+constants+'public core.game.node.item.Item get(int i){return null;}}')
 adapter('core/game/node/item/Item.java','package core.game.node.item;public class Item{public int getId(){throw new UnsupportedOperationException();}public Item getDefinition(){return this;}public int getEquipId(){throw new UnsupportedOperationException();}public boolean getConfiguration(String k,boolean d){return d;}}')
 adapter('core/game/system/config/ItemConfigParser.java','package core.game.system.config;public class ItemConfigParser{public static final String REMOVE_SLEEVES="a",REMOVE_HEAD="b",IS_HAT="c",REMOVE_BEARD="d";}')
 adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;public class Player{public String username="self";public int bodySize=1;public core.game.node.entity.player.link.appearance.App appearance=new core.game.node.entity.player.link.appearance.App();public Skull skullManager=new Skull();public Props properties=new Props();public Familiar familiarManager=new Familiar();public Skills skills=new Skills();public Render renderInfo=new Render();public int size(){return bodySize;}public core.game.container.impl.EquipmentContainer getEquipment(){return new core.game.container.impl.EquipmentContainer();}public Render getRenderInfo(){return renderInfo;}public static class Render{public boolean prepared;public boolean preparedAppearance(){return prepared;}public void setPreparedAppearance(boolean b){prepared=b;}}public static class Skull{public boolean wilderness,disabled;public int level;public boolean isWilderness(){return wilderness;}public boolean isWildernessDisabled(){return disabled;}}public static class Props{public int combatLevel=3,currentCombatLevel=3;}public static class Familiar{public boolean using;public int summoningCombatLevel;public boolean isUsingSummoning(){return using;}}public static class Skills{public int total=33;public int getTotalLevel(){return total;}}}''')
 adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static Settings settings=new Settings();public static class Settings{public boolean pvp,wild_pvp_enabled;public boolean isPvp(){return pvp;}}}')
 strings=read(SERVER/'core/tools/StringUtils.java');adapter('core/tools/StringUtils.java','package core.tools;public class StringUtils{'+c.method(strings,'public static long stringToLong(')+'}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim original PlayerFlags530.Appearance/Animate/SpotAnim subclasses, Appearance.prepareBodyData/body helpers and StringUtils.stringToLong; complete Gender/BodyPart/Graphics/IoBuffer; original AudioPacket.write global/area wire bytes. Explicit player/settings/render/equipment records. Empty-equipment preparation and arbitrary already-prepared bodies compared for all appearance field bytes, PvP/summoning/NPC/name/length branches. Spot ID truncation and wrapping height-shift-plus-delay packing include signed extremes and carry cases. Graphics.send static chunk delivery is not executed; its external RegionManager/chunk/GraphicUpdateFlag adapters throw if called. Equipped-body preparation, content/combat/familiar resolution, live graphics dispatch and full Appearance APIs remain separate.')
 folder=ROOT/'fixtures/appearance'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Appearance fixtures/provenance differ')
  print(f'{len(out)} original appearance scenarios reproduced.')
 else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original appearance scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
