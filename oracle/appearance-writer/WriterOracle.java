package oracle;
import java.io.*;import java.lang.reflect.*;import java.util.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.entity.player.Player;import core.game.node.entity.player.link.appearance.Appearance;
import core.game.world.GameWorld;import core.game.world.update.flag.PlayerFlags530;
import core.net.packet.IoBuffer;

public class WriterOracle {
 static boolean active;static Map input;static IoBuffer buffer;
 static Map map(Object... a){return AppearanceOracle.map(a);}
 static int n(Map m,String k,int d){Object v=m.get(k);return v==null?d:((Number)v).intValue();}
 static boolean b(Map m,String k){return Boolean.TRUE.equals(m.get(k));}
 static Player p(){return AppearanceOracle.players[0];}
 public static void logged(String text){hit("invalidType",text);}
 public static void hit(String key,Object... args){
  AppearanceOracle.hit(key,args);if(!active)return;
  int at=AppearanceOracle.hits.get(key);
  for(Object raw:(List)input.get("wireHooks")){
   Map h=(Map)raw;if(!key.equals(h.get("when"))||n(h,"at",1)!=at)continue;
   int slot=n(h,"slot",0);String family=(String)h.get("family");
   switch((String)h.get("action")){
    case "fail":throw new IllegalStateException("controlled");
    case "redirect":p().appearanceField=AppearanceOracle.apps.get("B");break;
    case "nullAppearance":p().appearanceField=null;break;
    case "size":p().size=n(h,"value",0);break;
    case "color":AppearanceOracle.apps.get("A").getAppearanceCache()[n(h,"index",0)].changeColor(n(h,"value",0));break;
    case "override":AppearanceOracle.override=AppearanceOracle.value(h.get("value"));break;
    case "buffer":buffer.p1(n(h,"value",0));break;
    case "position":buffer.toByteBuffer().position(n(h,"value",0));break;
    case "reenter":new PlayerFlags530.Appearance().writeTo(buffer,p());break;
    case "active":switch(family){case "world":GameWorld.active=n(h,"value",0);break;case "skull":p().activeSkull=n(h,"value",0);break;case "properties":p().activeProp=n(h,"value",0);break;case "familiar":p().activeFamiliar=n(h,"value",0);break;case "skills":p().activeSkills=n(h,"value",0);break;default:throw new AssertionError(family);}break;
    case "set":try{
     Object receiver=family.equals("world")?GameWorld.slots[slot]:family.equals("skull")?p().skulls[slot]:family.equals("properties")?p().propSlots[slot]:family.equals("familiar")?p().familiars[slot]:p().skillSlots[slot];
     Field f=receiver.getClass().getField((String)h.get("field"));if(f.getType()==boolean.class)f.setBoolean(receiver,(Boolean)h.get("value"));else f.setInt(receiver,n(h,"value",0));
    }catch(ReflectiveOperationException e){throw new AssertionError(e);}break;
    default:throw new AssertionError(h);
   }
  }
 }
 static Object wireState(){
  List<Object> worlds=new ArrayList<>(),skulls=new ArrayList<>(),properties=new ArrayList<>(),familiars=new ArrayList<>(),skills=new ArrayList<>();
  for(int i=0;i<2;i++){
   worlds.add(map("pvp",GameWorld.slots[i].pvp,"wild",GameWorld.slots[i].wild));
   skulls.add(map("wilderness",p().skulls[i].wilderness,"disabled",p().skulls[i].disabled,"level",p().skulls[i].levelField));
   properties.add(map("combat",p().propSlots[i].combat,"current",p().propSlots[i].current));
   familiars.add(map("using",p().familiars[i].using,"summoning",p().familiars[i].summoning));
   skills.add(map("total",p().skillSlots[i].total));
  }
  return map("world",worlds,"skull",skulls,"properties",properties,"familiar",familiars,"skills",skills,"active",map("world",GameWorld.active,"skull",p().activeSkull,"properties",p().activeProp,"familiar",p().activeFamiliar,"skills",p().activeSkills),"username",p().usernameField);
 }
 static Object evaluate(Map row)throws Exception{
  active=false;input=row;AppearanceOracle.evaluate(row);Map wire=(Map)row.get("wire");Appearance a=AppearanceOracle.apps.get("A");
  if(wire.containsKey("gender")){try{a.setGender(wire.get("gender")==null?null:n(wire,"gender",0)==0?core.game.node.entity.player.link.appearance.Gender.MALE:core.game.node.entity.player.link.appearance.Gender.FEMALE);}catch(NullPointerException e){}}
  if(n(wire,"npc",-1)!=-1)a.transformNPC(n(wire,"npc",-1));
  if(wire.containsKey("size"))p().size=n(wire,"size",1);
  p().usernameField=wire.containsKey("username")?(String)wire.get("username"):"self";
  if(wire.containsKey("colors")){List colors=(List)wire.get("colors");for(int i=0;i<7;i++)a.getAppearanceCache()[i].changeColor(((Number)colors.get(i)).intValue());}
  if(wire.containsKey("body")){List parts=(List)wire.get("body");for(int i=0;i<14;i++)a.getBodyParts()[i]=((Number)parts.get(i)).intValue();}
  if(wire.containsKey("nullPart"))a.getAppearanceCache()[n(wire,"nullPart",0)]=null;
  if(wire.containsKey("render"))a.setAnimations(new core.game.world.update.flag.context.Animation(n(wire,"render",1426)));
  a.setSkullIcon(n(wire,"skull",-1));a.setHeadIcon(n(wire,"head",-1));
  p().renders[0].prepared=b(wire,"prepared");
  GameWorld.slots=new GameWorld.Settings[]{new GameWorld.Settings(0),new GameWorld.Settings(1)};GameWorld.active=0;
  for(int i=0;i<2;i++){
   GameWorld.slots[i].pvp=i==0?b(wire,"pvp"):!b(wire,"pvp");GameWorld.slots[i].wild=i==0?b(wire,"wild"):!b(wire,"wild");
   p().skulls[i].wilderness=i==0?b(wire,"wilderness"):!b(wire,"wilderness");p().skulls[i].disabled=i==0?b(wire,"disabled"):!b(wire,"disabled");p().skulls[i].levelField=n(wire,"level",17)+i*13;
   p().propSlots[i].combat=n(wire,"combat",51)+i*13;p().propSlots[i].current=n(wire,"current",57)+i*13;
   p().familiars[i].using=i==0?b(wire,"using"):!b(wire,"using");p().familiars[i].summoning=n(wire,"summoning",6)+i*13;p().skillSlots[i].total=n(wire,"total",2001)+i*13;
  }
  buffer=new IoBuffer();for(Object v:(List)row.get("prefix"))buffer.p1(((Number)v).intValue());AppearanceOracle.trace.clear();AppearanceOracle.hits.clear();active=true;
  List<Object> results=new ArrayList<>();
  for(Object raw:(List)row.get("writes")){
   Map w=(Map)raw;String error=null;
   try{Object context=AppearanceOracle.players[n(w,"player",0)];if(w.containsKey("invalid"))context=w.get("invalid").equals("null")?null:w.get("invalid").equals("String")?"x":Integer.valueOf(3);new PlayerFlags530.Appearance().writeTo(b(w,"nullBuffer")?null:buffer,context);}
   catch(ClassCastException e){error="Type";}catch(NullPointerException e){error="Null";}catch(IndexOutOfBoundsException e){error="Index";}catch(IllegalArgumentException e){error="Index";}catch(IllegalStateException e){error="Binding";}
   int position=buffer.toByteBuffer().position();StringBuilder hex=new StringBuilder();for(int i=0;i<position;i++)hex.append(String.format("%02x",buffer.toByteBuffer().get(i)&255));
   results.add(map("bytes",hex.toString(),"position",position,"error",error,"state",AppearanceOracle.state(),"wire",wireState()));
  }
  active=false;return map("results",results,"trace",AppearanceOracle.trace);
 }
 public static void main(String[]args)throws Exception{
  BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;
  while((line=reader.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(row));}
 }
}
