"""Joined original Entity facing, UpdateMasks and concrete 530 flag bytes."""
import argparse,importlib.util,itertools,os,pathlib,random
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
s=importlib.util.spec_from_file_location('mask_builder',ROOT/'oracle/update-masks/generate.py');b=importlib.util.module_from_spec(s);s.loader.exec_module(b)
b.HERE=HERE;b.RUN=ROOT/'runs/prepared-masks-oracle'
def op(kind,**kw):return dict(op=kind,**kw)
def requests():
 rows=[]
 coords=[-2147483648,-32769,-16384,-1,0,3207,16383,32768,2147483647]
 for x,y,prepared in itertools.product(coords,coords,[False,True]):
  actions=[op('move',location=0,coords=[x,y,-1]),op('location',location=0),op('write')]
  if prepared:actions.append(op('prepare'))
  actions += [op('location',location=None),op('write'),op('reset'),op('write',synced=True),op('location',location=0),op('reset'),op('move',location=0,coords=[y,x,5]),op('write',synced=True)]
  rows.append(dict(actions=actions))
 for index,target,prepared in itertools.product([-2147483648,-1,0,1,32767,32768,65535,2147483647],[1,2],[False,True]):
  actions=[op('index',entity=target,value=index),op('face',target=target),op('write')]
  if prepared:actions.append(op('prepare'))
  actions += [op('face',target=None),op('write'),op('reset'),op('write',synced=True),op('face',target=target),op('reset'),op('index',entity=target,value=index^32768),op('write',synced=True)]
  rows.append(dict(actions=actions))
 for ident,delay in itertools.product([-2147483648,-1,0,535,863,65535,65536,2147483647],[-1,0,255,256]):
  rows.append(dict(actions=[op('location',location=0),op('face',target=1),op('animate',id=ident,delay=delay),op('write'),op('prepare'),op('animate',id=-1,delay=0),op('write'),op('reset'),op('write',synced=True)]))
 for ident,height,delay in itertools.product([-1,0,111,369,65535,65536],[-2147483648,-1,0,128,65535],[-65537,-1,0,30,65536]):
  rows.append(dict(actions=[op('face',target=2),op('location',location=1),op('spot',id=ident,height=height,delay=delay),op('write'),op('prepare'),op('write',synced=True,appearance=False),op('reset'),op('write',synced=True)]))
 rng=random.Random(530)
 actions=[op('face',target=t)for t in [None,1,2]]+[op('location',location=l)for l in [None,0,1]]+[op('reset'),op('prepare'),op('write'),op('write',synced=True),op('write',synced=True,appearance=False),op('animate',id=535,delay=0),op('spot',id=-1,height=0,delay=0),op('move',location=0,coords=[-1,32768,0]),op('index',entity=1,value=65535)]
 for _ in range(64):rows.append(dict(actions=[rng.choice(actions)for _ in range(24)]))
 return rows
def extras(adapter,java,kt):
 c=b.c;read=c.read;server=b.SERVER
 flags=read(server/'core/game/world/update/flag/PlayerFlags530.kt')
 adapter('core/game/world/update/flag/PreparedFlags530.kt','''package core.game.world.update.flag
import core.net.packet.IoBuffer
import core.game.world.map.Location
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.update.flag.context.*
import kotlin.reflect.*
sealed class PlayerFlags530(p:Int,o:Int,f:EntityFlag):EFlagProvider(530,EFlagType.Player,p,o,f){
'''+ '\n'.join(c.method(flags,'class '+name+' : PlayerFlags530')for name in ['Animate','SpotAnim','FaceEntity','FaceLocation','AnimationSequence'])+'\n}')
 for name in ['core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java']:
  p=server/name;read(p);java.append(p)
 # Replace, rather than also compile, the core builder's small buffer adapter.
 java[:]=[p for p in java if p!=b.RUN/'adapters/core/net/packet/IoBuffer.java']
 for name in ['core/cache/crypto/ISAACCipher.java','core/cache/misc/buffer/ByteBufferUtils.java']:
  adapter(name,read(ROOT/'oracle/scenery/adapters'/name))
 entity=read(server/'core/game/node/entity/Entity.java');player=read(server/'core/game/node/entity/player/Player.java')
 adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;
import core.game.world.update.UpdateMasks;import core.game.world.update.flag.*;import core.game.world.map.Location;
public class Entity {public int id,handler,index;public UpdateMasks masks;
public Entity(int i){id=i;index=i+1;}public int getIndex(){return index;}
public UpdateMasks getUpdateMasks(){return masks;}
public core.game.node.entity.combat.ImpactHandler getImpactHandler(){return oracle.Support.handlers[handler];}
'''+ '\n'.join(c.method(entity,s)for s in ['public int getClientIndex()','public boolean face(Entity entity)','public boolean faceLocation(Location location)'])+'}')
 adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player;public class Player extends core.game.node.entity.Entity{public Player(int i){super(i);}@Override '+c.method(player,'public int getClientIndex()')+'}')
 adapter('core/game/world/update/flag/EntityFlags.kt','''package core.game.world.update.flag
object EntityFlags {
@JvmStatic fun getFlagFor(type:EFlagType,flag:EntityFlag):EFlagProvider?=oracle.Support.provider(type,flag)
@JvmStatic fun getPresenceFlag(type:EFlagType,flag:EntityFlag):Int=getFlagFor(type,flag)?.presenceFlag?:0
@JvmStatic fun getOrdinal(type:EFlagType,flag:EntityFlag):Int=getFlagFor(type,flag)?.ordinal?:-1
}''')
 location=read(server/'core/game/world/map/Location.kt')
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,z)
val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z
fun moveTo(x:Int,y:Int,z:Int){hash=LocationHash(x,y,z)}}
@JvmInline '''+c.method(location,'value class LocationHash('))
 adapter('core/game/world/update/flag/context/Animation.java','package core.game.world.update.flag.context;public class Animation{public int id,delay;public Animation(int i,int d){id=i;delay=d;}public int getId(){return id;}public int getDelay(){return delay;}}')
 adapter('core/game/world/update/flag/context/Graphics.java','package core.game.world.update.flag.context;public class Graphics{public int id,height,delay;public Graphics(int i,int h,int d){id=i;height=h;delay=d;}public int getId(){return id;}public int getHeight(){return height;}public int getDelay(){return delay;}}')
def generate(check):
 b.requests=requests;b.c.read(pathlib.Path(__file__))
 b.generate(check,folder=ROOT/'fixtures/prepared-masks',scope='Complete original UpdateMasks with controlled clock plus unchanged Entity.face/faceLocation and Entity/Player getClientIndex, concrete Animate/SpotAnim/FaceEntity/FaceLocation/AnimationSequence subclasses and complete IoBuffer. Original packed LocationHash/getters. Registry and immutable animation/graphics field records are explicit adapters; fresh actual impact queues are empty. Joined status/byte histories include persistent facing, clear during preparation, signed indices, context mutation and ordering. Native tests also use canonical target leases and actual prepared-body data. Full appearance/body preparation, combat/content/registry startup and live Take remain separate.',extras=extras)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();b.c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
