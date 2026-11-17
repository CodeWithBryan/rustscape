"""Explicit external services for the joined original navigation classes."""
import re
def compose(adapter,read,c,server,run=None):
    source=read(server/'core/game/world/map/path/Pathfinder.java')
    constants=re.findall(r'public static final int [^;]+;',source)
    overloads=[c.method(source,m.group())for m in re.finditer(r'public static Path (?:find|findWater)\([^)]*\)',source)]
    reach=[c.method(source,'public static '+('int 'if name=='flagForDirection'else'boolean ')+name+'(')for name in ['canDecorationInteract','canDoorInteract','isStandingIn','canInteract','canInteractSized','flagForDirection']]
    adapter('core/game/world/map/path/Pathfinder.java','package core.game.world.map.path;import core.game.node.*;import core.game.node.entity.*;import core.game.node.entity.npc.*;import core.game.node.scenery.*;import core.game.node.item.*;import core.game.world.map.*;public class Pathfinder{public int id;public Pathfinder(){this(0);}public Pathfinder(int i){id=i;}public Path find(Location a,int s,Location b,int sx,int sy,int r,int t,int f,boolean n,ClipMaskSupplier c){throw new UnsupportedOperationException();}public static final Pathfinder SMART=new SmartPathfinder(),DUMB=new DumbPathfinder();static{DUMB.id=1;}'+''.join(constants+overloads+reach)+'}')
    adapter('core/game/world/map/path/DumbPathfinder.java',read(server/'core/game/world/map/path/DumbPathfinder.java'))
    # Entity, Settings and world services remain explicit adapters. Actual queue,
    # search, pulse and destination algorithms compile unchanged from the pin.
    adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;
import core.game.node.Node;import core.game.world.map.*;import core.game.interaction.MovementPulse;
public class Entity extends Node{
public MovementPulse currentMovement;public core.game.node.entity.impl.WalkingQueue walking=new core.game.node.entity.impl.WalkingQueue(this);public Props props=new Props();public Scripts scripts=new Scripts(this);public Settings settings=new Settings();public Flags flags=new Flags();public int agility=1,incrementer;public boolean draw;
public core.game.node.entity.impl.WalkingQueue getWalkingQueue(){return walking;}public Props getProperties(){return props;}public core.game.node.entity.npc.NPC asNpc(){return(core.game.node.entity.npc.NPC)this;}
public void face(Entity e){oracle.Support.emit("face",name,e==null?null:e.name);}public void setLocation(Location l){location=l;}public void setDirection(Direction d){if(d!=null)direction=d;}public Zone getZoneMonitor(){return new Zone();}public PulseManager getPulseManager(){return new PulseManager();}
public <T>T getAttribute(String key,T fallback){return(T)(key.equals("routedraw")?(Object)false:key.equals("run-incrementer")?(Object)incrementer:key.equals("draw_intersect")?(Object)draw:fallback);}public void setAttribute(String k,Object v){incrementer=((Number)v).intValue();}public void incrementAttribute(String k){incrementer++;}
public Locks getLocks(){return new Locks();}public static class Locks{public boolean isMovementLocked(){return false;}}
public static class Props{public Combat combat=new Combat();public Combat getCombatPulse(){return combat;}public Location teleport;public boolean teleporting;public Location getTeleportLocation(){return teleport;}public void setTeleportLocation(Location l){teleport=l;}public void setTeleporting(boolean b){teleporting=b;}}
public static class Combat{public boolean running;public Entity victim;public boolean isRunning(){return running;}public Entity getVictim(){return victim;}}
public static class Scripts{Entity e;Scripts(Entity e){this.e=e;}public void removeWeakScripts(){oracle.Support.emit("weak",e.name);}public void removeNormalScripts(){oracle.Support.emit("normal",e.name);}}
public static class Flags{public Location last;public boolean update;public Location getLastSceneGraph(){return last;}public void setUpdateSceneGraph(boolean b){update=b;}public boolean isUpdateSceneGraph(){return update;}}
public static class Zone{public boolean move(Location a,Location b){return oracle.Support.zone(a,b);}}public static class PulseManager{public void clear(){oracle.Support.emit("clearPulses");}}
public static class Settings{public double energy=100,weight;public boolean toggled;public double getRunEnergy(){return energy;}public double getWeight(){return weight;}public boolean isRunToggled(){return toggled;}public void setRunToggled(boolean v){toggled=v;oracle.Support.emit("toggle",v);}public void updateRunEnergy(double drain){energy-=drain;if(energy<0)energy=0;else if(energy>100)energy=100;oracle.Support.emit("energy",oracle.Support.bits(drain));}}
}''')
    read(server/'core/game/node/entity/player/link/Settings.java')
    adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;public class Player extends core.game.node.entity.Entity{public Dispatch getPacketDispatch(){return new Dispatch();}public class Dispatch{public void sendMessage(String s){oracle.Support.emit("message",name,s);}}public Settings getSettings(){return settings;}public Flags getPlayerFlags(){return flags;}public Skills getSkills(){return new Skills();}public class Skills{public int getLevel(int id){return agility;}}}''')
    adapter('core/game/node/entity/npc/NPCBehavior.java','package core.game.node.entity.npc;public class NPCBehavior{public core.game.world.map.path.Pathfinder getPathfinderOverride(NPC n){return oracle.Support.npcFinder(n);}public core.game.world.map.path.ClipMaskSupplier getClippingSupplier(NPC n){return null;}}')
    adapter('core/game/node/entity/skill/Skills.java','package core.game.node.entity.skill;public class Skills{public static final int AGILITY=16;}')
    adapter('core/game/node/item/Item.java','package core.game.node.item;public class Item{public Item(int id){}}')
    adapter('core/game/node/item/GroundItem.java','package core.game.node.item;public class GroundItem extends core.game.node.Node{public GroundItem(Item i,core.game.world.map.Location l,core.game.node.entity.player.Player p){location=l;}}')
    adapter('core/game/world/map/RegionManager.java','''package core.game.world.map;import core.game.node.item.GroundItem;public class RegionManager{public static int getClippingFlag(int z,int x,int y){return oracle.Support.clip(z,x,y);}public static int getWaterClipFlag(int z,int x,int y){return 0;}public static boolean isTeleportPermitted(Location l){return oracle.Support.permitted(l);}public static void move(core.game.node.entity.Entity e,Location a,Location b){oracle.Support.move(a,b);}public static Chunk getRegionChunk(Location l){return new Chunk();}public static class Chunk{public void add(GroundItem g){throw new UnsupportedOperationException();}public void remove(GroundItem g){throw new UnsupportedOperationException();}}}''')
    adapter('content/global/skill/skillcapeperks/SkillcapePerks.java','package content.global.skill.skillcapeperks;public class SkillcapePerks{public static final int MARATHON_RUNNER=1;public static boolean isActive(int id,core.game.node.entity.player.Player p){return oracle.Support.marathon();}}')
    adapter('core/tools/Log.java','package core.tools;public enum Log{ERR,FINE;}')
    adapter('core/game/world/GameWorld.kt','package core.game.world;object GameWorld{var settings:Settings?=Settings()};class Settings{var smartpathfinder_bfs=false}')
    # Enrich the already-authored Location/ContentAPI adapters in place. Their
    # bodies remain original where algorithms are under test.
    if run is None:run=__import__('pathlib').Path(__file__).resolve().parents[2]/'runs/navigation-oracle/adapters'
    p=run/'core/game/world/map/Location.kt'
    text=p.read_text(encoding='utf-8').replace('override fun getLocation()=this;', 'val regionX:Int get()=x shr 3;val regionY:Int get()=y shr 3;val sceneX:Int get()=x-((regionX-6)shl 3);val sceneY:Int get()=y-((regionY-6)shl 3);fun getSceneX(l:Location)=x-((l.regionX-6)shl 3);fun getSceneY(l:Location)=y-((l.regionY-6)shl 3);override fun getLocation()=this;')
    adapter('core/game/world/map/Location.kt',text)
    p=run/'core/api/ContentAPI.kt'
    adapter('core/api/ContentAPI.kt',p.read_text(encoding='utf-8')+'\nfun hasTimerActive(e:Entity,n:String)=oracle.Support.timer(n)\nfun log(c:Class<*>,l:core.tools.Log,s:String){}\n')
