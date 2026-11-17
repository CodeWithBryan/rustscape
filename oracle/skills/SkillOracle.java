import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.skill.Skills;
import core.game.node.entity.player.link.request.assist.AssistSession;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import oracle.Model;
import java.io.*;
import java.util.*;
import java.lang.reflect.*;
public class SkillOracle {
 interface Work {Object run()throws Exception;}
 static int integer(Map<?,?> m,String key){return ((Number)m.get(key)).intValue();}static double number(Map<?,?> m,String key){return Double.parseDouble((String)m.get(key));}
 static Map<String,Object> map(Object... e){Map<String,Object> out=new TreeMap<>();for(int i=0;i<e.length;i+=2)out.put((String)e[i],e[i+1]);return out;}
 static Object attempt(Work work){try{return map("ok",work.run());}catch(Exception e){return map("error",e instanceof IndexOutOfBoundsException?"index":e instanceof NumberFormatException?"number":e instanceof NullPointerException?"null":e instanceof ClassCastException?"type":"binding");}}
 static Object evaluate(JSONObject input)throws Exception {
  Model.config=(JSONObject)input.get("config");if(Model.config==null)Model.config=new JSONObject();Model.ticks=Model.integer("tick",0);Model.failure="";Model.trace.clear();Model.equipment.clear();List<?> equipped=(List<?>)Model.config.get("equipment");if(equipped!=null)for(Object id:equipped)Model.equipment.add(((Number)id).intValue());
  String type=(String)Model.config.getOrDefault("entity","player");Entity entity=type.equals("npc")?new NPC():type.equals("other")?new Entity():new Player();Model.owner=entity;Skills skills=new Skills(entity);entity.skills=skills;
  if(entity instanceof Player){Player player=(Player)entity;player.version=Model.integer("version",6);if(Model.config.containsKey("permadeath"))player.attributes.put("permadeath",Model.config.get("permadeath"));}
  if(Model.flag("assist")){AssistSession assist=new AssistSession();assist.current=entity instanceof Player?(Player)entity:null;assist.other=new Player();assist.other.assister=true;assist.other.skills=new Skills(assist.other);Arrays.fill(assist.other.skills.getDynamicLevels(),Model.integer("assistLevel",1));entity.assist=assist;}
  if(input.get("kind").equals("name"))return Skills.getSkillByName((String)input.get("name"));
  if(input.get("kind").equals("curve"))return skills.getExperienceByLevel(integer(input,"level"));
  if(input.get("kind").equals("xp_level"))return skills.levelFromXP(number(input,"value"));
  List<Object> outputs=new ArrayList<>();for(Object item:(List<?>)input.get("actions")){Map<?,?> action=(Map<?,?>)item;JSONObject root=new JSONObject();Model.trace.clear();Object result=attempt(()->{int slot=action.containsKey("skill")?integer(action,"skill"):0;switch((String)action.get("op")){
   case "configure":skills.configure();break;case "combat":return skills.updateCombatLevel();case "pulse":skills.pulse();break;case "refresh":skills.refresh();break;case "restore":skills.restore();break;
   case "setLevel":skills.setLevel(slot,integer(action,"value"));break;case "getLevel":return skills.getLevel(slot);case "getLevelDiscard":return skills.getLevel(slot,true);case "staticLevel":skills.setStaticLevel(slot,integer(action,"value"));break;
   case "life":skills.setLifepoints(integer(action,"value"));break;case "maxLife":return skills.getMaximumLifepoints();case "heal":return skills.heal(integer(action,"value"));case "hit":return skills.hit(integer(action,"value"));case "healRaw":skills.healNoRestrictions(integer(action,"value"));break;
   case "prayerSet":skills.setPrayerPoints(number(action,"value"));break;case "prayerInc":skills.incrementPrayerPoints(number(action,"value"));break;case "prayerDec":skills.decrementPrayerPoints(number(action,"value"));break;case "recharge":skills.rechargePrayerPoints();break;
   case "update":return skills.updateLevel(slot,integer(action,"amount"),integer(action,"maximum"));case "updateDefault":return skills.updateLevel(slot,integer(action,"amount"));case "drain":skills.drainLevel(slot,number(action,"drain"),number(action,"maximum"));break;
   case "xp":skills.addExperience(slot,number(action,"value"),Boolean.TRUE.equals(action.get("playerMod")));break;case "rawXp":Field xp=Skills.class.getDeclaredField("experience");xp.setAccessible(true);((double[])xp.get(skills))[slot]=number(action,"value");break;
   case "tick":Model.ticks=integer(action,"value");break;case "fail":Model.failure=(String)action.get("value");break;case "save":new SkillSaver((Player)entity).saveSkills(root);break;case "parse":skills.parse((JSONArray)action.get("value"));break;
   case "copy":Entity other=new Entity();Skills source=new Skills(other);other.skills=source;Model.setup(source,(JSONObject)action.get("value"));skills.copy(source);break;
   case "profile":SkillParser parser=new SkillParser((Player)entity);parser.setSaveFile((JSONObject)action.get("value"));parser.parseSkills();break;case "correct":skills.correct(number(action,"value"));break;
   case "rawLevels":List<?> levels=(List<?>)action.get("static"),dynamic=(List<?>)action.get("dynamic");for(int i=0;i<24;i++){skills.getStaticLevels()[i]=((Number)levels.get(i)).intValue();skills.getDynamicLevels()[i]=((Number)dynamic.get(i)).intValue();}break;
   default:throw new IllegalArgumentException();}return null;});
   outputs.add(map("result",result,"state",Model.snapshot(skills),"trace",new ArrayList<>(Model.trace),"root",root,"combat",entity.getProperties().getCombatLevel(),"summon",entity instanceof Player?((Player)entity).familiar.combat:0,"tick",Model.ticks,"equipment",new TreeSet<>(Model.equipment),"totals",Arrays.asList(skills.getTotalLevel(),skills.getTotalXp(),skills.getMasteredSkills(),skills.getHighestCombatSkillId())));
  }return outputs;
 }
 public static void main(String[] args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));String line;while((line=reader.readLine())!=null){JSONObject input=(JSONObject)new JSONParser().parse(line);input.put("expected",evaluate(input));System.out.println(JSONValue.toJSONString(input));}}
}
