package core.game.node.entity.npc;
import core.game.node.entity.Entity;import core.game.node.entity.player.Player;import core.game.world.GameWorld;import core.game.world.map.Location;import core.game.world.map.path.Pathfinder;import core.game.interaction.MovementPulse;import core.tools.RandomFunction;import core.api.utils.Vector;import java.util.*;
public class NPC extends Entity{
 public NPC definition=this;
 public int walkRadius=11,nextWalk,respawnTick=-1,movementIndex;public boolean walks,pathBoundMovement,aggressive,neverWalks,isRespawning,movementLocked,interactionLocked;public String forceTalk;public Location[] movementPath;public Player dialoguePlayer;public Behavior behavior=new Behavior();public Aggression aggressiveHandler;public Props props=new Props();public Map<String,Object>attributes=new HashMap<>();
 public class Props{public Location spawnLocation=location;public Combat combat=new Combat();public Location getSpawnLocation(){return spawnLocation;}public Combat getCombatPulse(){return combat;}}
 public class Combat{public boolean attacking,inCombat;public boolean isAttacking(){return oracle.Support.query("Attacking",attacking);}public boolean isInCombat(){return oracle.Support.query("InCombat",inCombat);}public void stop(){attacking=false;oracle.Support.emit("StopCombat");}}
 public Props getProperties(){return props;}public int getWalkRadius(){return walkRadius;}public boolean isNeverWalks(){return neverWalks;}
 public Locks getLocks(){return new Locks();}public class Locks{public boolean isInteractionLocked(){return oracle.Support.query("InteractionLocked",interactionLocked);}public boolean isMovementLocked(){return oracle.Support.query("MovementLocked",movementLocked);}public void lockMovement(int n){movementLocked=true;oracle.Support.emit("LockMovement",n);}public void unlockMovement(){movementLocked=false;oracle.Support.emit("UnlockMovement");}}
 public class Behavior{public boolean tick(NPC n){return oracle.Support.query("Behavior",true);}public void onRespawn(NPC n){oracle.Support.emit("BehaviorRespawn");}}
 public static class Aggression{public int pause;public void setPauseTicks(int n){pause=n;oracle.Support.emit("PauseAggression",n);}public boolean selectTarget(){return oracle.Support.query("SelectTarget",false);}}
 public NPC getPulseManager(){return this;}public boolean hasPulseRunning(){return oracle.Support.query("PulseRunning",false);}public NPC getImpactHandler(){return this;}public void setDisabledTicks(int n){oracle.Support.impact=n;oracle.Support.emit("ImpactDisabled",n);}
 public NPC getDefinition(){return this;}public Map<String,Object>getHandlers(){Map<String,Object>m=new HashMap<>();m.put("water_npc",oracle.Support.query("WaterNpc",false));return m;}
 public boolean canMove(Location l){return oracle.Support.query("CanMove",true,oracle.Support.coord(l));}public void sendChat(String s){oracle.Support.emit("Chat",s);}public void onRegionInactivity(){oracle.Support.emit("Inactivity");}protected void onRespawn(){oracle.Support.emit("Respawn");}
 public <T>T getAttribute(String key){return(T)attributes.get(key);}public <T>T getAttribute(String key,T fallback){return(T)attributes.getOrDefault(key,fallback);}public void setAttribute(String key,Object v){attributes.put(key,v);oracle.Support.emit("Attribute",key);}public void removeAttribute(String key){attributes.remove(key);oracle.Support.emit("Attribute","remove:"+key);}
 public Location destination(){return getMovementDestination();}
 /* ORIGINAL_METHODS */
}
