package core.game.node.entity.npc;
import java.util.*;import core.game.node.entity.Entity;import core.game.world.map.*;import core.cache.def.impl.NPCDefinition;import core.game.node.entity.skill.Skills;import core.game.system.config.NPCConfigParser;import core.game.world.update.flag.context.*;import content.global.skill.summoning.familiar.Familiar;
public class NPC extends Entity{
 public int slot,id,originalId,index,size,walkRadius=11;public String forceTalk;public boolean aggressive,walks;public NPCDefinition definition;public NPC[]children;public Props props;public Skills skills;public Direction direction;public Interact interactPlugin=new Interact(this);public Behavior behavior=new Behavior(this);
 public NPC(int id){this(id,null,Direction.SOUTH);}public NPC(int id,Location l){this(id,l,Direction.SOUTH);}public NPC(int id,Location l,Direction d){super(l);slot=oracle.Support.nodes.size();oracle.Support.nodes.add(this);this.id=this.originalId=id;definition=oracle.Support.definition(id);size=definition.size;direction=d;props=new Props(oracle.Support.properties.size());oracle.Support.properties.add(props);skills=new Skills(oracle.Support.skills.size());oracle.Support.skills.add(skills);oracle.Support.emit("base",slot,id,oracle.Support.coord(l),d==null?null:d.ordinal());}
 public Props getProperties(){Props result=props;oracle.Support.emit("properties",slot);return result;}public Skills getSkills(){Skills result=skills;oracle.Support.emit("skills",slot);return result;}
 public <T>T getAttribute(String k,T fallback){oracle.Support.emit("hp",slot);return(T)oracle.Support.hp.getOrDefault(slot,fallback);}
 public void configure(){oracle.Support.emit("configure",slot);}public void setDefaultBehavior(){oracle.Support.emit("defaultBehavior",slot);}
 public void setWalkRadius(int r){walkRadius=r;}
 public static class Interact{NPC n;Interact(NPC n){this.n=n;}public void setDefault(){oracle.Support.emit("interact",n.slot);}}
 public static class Behavior{NPC n;Behavior(NPC n){this.n=n;}public void onCreation(NPC ignored){oracle.Support.emit("created",n.slot);}}
 public static class Props{
  public int slot,combatLevel,attackSpeed;public Location spawn;public int[]bonuses;public Animation attack,defence,death,magic,range;public Graphics deathGfx;
  Props(int slot){this.slot=slot;}
  public void setSpawnLocation(Location l){spawn=l;oracle.Support.emit("spawn",slot,oracle.Support.coord(l));}
  public void setCombatLevel(int v){combatLevel=v;oracle.Support.emit("combat",slot,v);}
  public void setBonuses(int[]v){bonuses=v;oracle.Support.emit("bonuses",slot,oracle.Support.ints(v));}
  public void setAttackSpeed(int v){attackSpeed=v;oracle.Support.emit("speed",slot,v);}
  public void setAttackAnimation(Animation a){attack=a;oracle.Support.emit("animation",slot,"Attack",oracle.Support.animation(a));}public void setDefenceAnimation(Animation a){defence=a;oracle.Support.emit("animation",slot,"Defence",oracle.Support.animation(a));}public void setDeathAnimation(Animation a){death=a;oracle.Support.emit("animation",slot,"Death",oracle.Support.animation(a));}public void setMagicAnimation(Animation a){magic=a;oracle.Support.emit("animation",slot,"Magic",oracle.Support.animation(a));}public void setRangeAnimation(Animation a){range=a;oracle.Support.emit("animation",slot,"Range",oracle.Support.animation(a));}
  public Animation getAttackAnimation(){Animation result=attack;oracle.Support.emit("getAnimation",slot,"Attack");return result;}public Animation getMagicAnimation(){Animation result=magic;oracle.Support.emit("getAnimation",slot,"Magic");return result;}public Animation getRangeAnimation(){Animation result=range;oracle.Support.emit("getAnimation",slot,"Range");return result;}
 }
 /* ORIGINAL_METHODS */
}
