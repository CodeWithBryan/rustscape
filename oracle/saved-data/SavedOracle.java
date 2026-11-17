import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.*;
import core.game.node.item.Item;
import content.data.GodBook;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import java.util.*;
import java.lang.reflect.*;
import java.io.*;
public final class SavedOracle {
 interface Work {Object run()throws Exception;}
 static Map<String,Object> map(Object... entries){Map<String,Object> out=new TreeMap<>();for(int i=0;i<entries.length;i+=2)out.put((String)entries[i],entries[i+1]);return out;}
 static String bits(double value){return String.format("%016x",Double.doubleToLongBits(value));}
 static Object snapshot(Object value)throws Exception {
  if(value==null||value instanceof String||value instanceof Number||value instanceof Boolean){return value instanceof Double?bits((Double)value):value;}
  if(value.getClass().isArray()){List<Object> out=new ArrayList<>();for(int i=0;i<Array.getLength(value);i++)out.add(snapshot(Array.get(value,i)));return out;}
  Map<String,Object> out=new TreeMap<>();for(Field f:value.getClass().getDeclaredFields()){if(Modifier.isStatic(f.getModifiers())||f.getName().equals("player"))continue;f.setAccessible(true);out.put(f.getName(),snapshot(f.get(value)));}return out;
 }
 static Object attempt(Work work){try{return map("ok",work.run());}catch(Exception e){while(e instanceof InvocationTargetException)e=(Exception)((InvocationTargetException)e).getCause();return map("error",e instanceof NumberFormatException?"number":e instanceof IndexOutOfBoundsException?"index":e instanceof NullPointerException?"null":e instanceof ClassCastException?"type":e.getClass().getSimpleName());}}
 static Object target(SavedData data,String name){switch(name){case "activity":return data.getActivityData();case "quest":return data.getQuestData();case "global":return data.getGlobalData();default:throw new IllegalArgumentException();}}
 static Object call(Object target,Map<?,?> action)throws Exception {
  List<?> types=(List<?>)action.get("types"),args=(List<?>)action.get("args");Object[] values=new Object[args.size()];Class<?>[] classes=new Class<?>[types.size()];
  for(int i=0;i<types.size();i++){String type=(String)types.get(i);Object value=args.get(i);switch(type){case "int":classes[i]=int.class;value=((Number)value).intValue();break;case "long":classes[i]=long.class;value=((Number)value).longValue();break;case "boolean":classes[i]=boolean.class;break;case "String":classes[i]=String.class;value=Boolean.TRUE.equals(action.get("literal"))?((String)value).intern():new String((String)value);break;case "GodBook":classes[i]=GodBook.class;value=GodBook.values()[((Number)value).intValue()];break;default:throw new IllegalArgumentException(type);}values[i]=value;}
  Object result=target.getClass().getMethod((String)action.get("method"),classes).invoke(target,values);return snapshot(result);
 }
 static Object evaluate(JSONObject input)throws Exception {
  if(input.get("kind").equals("double_format"))return Double.toString(Double.longBitsToDouble(Long.parseUnsignedLong((String)input.get("bits"),16)));
  if(input.get("kind").equals("double_parse"))return attempt(()->bits(Double.parseDouble((String)input.get("text"))));
  Player player=new Player();SavedData data=player.getSavedData();List<Object> outputs=new ArrayList<>();
  for(Object item:(List<?>)input.get("actions")){Map<?,?> action=(Map<?,?>)item;JSONObject root=new JSONObject();Object result=attempt(()->{switch((String)action.get("op")){
   case "parse":Object target=target(data,(String)action.get("target"));target.getClass().getMethod("parse",JSONObject.class).invoke(target,action.get("value"));break;
   case "sections":SavedParser parser=new SavedParser(player);parser.setSaveFile((JSONObject)action.get("value"));parser.parseSavedData();break;
   case "save":new SavedSaver(player).saveSavedData(root);break;
   case "call":return call(target(data,(String)action.get("target")),action);
   case "item":data.getQuestData().setDesertTreasureItem(((Number)action.get("index")).intValue(),action.get("id")==null?null:new Item(((Number)action.get("id")).intValue(),((Number)action.get("amount")).intValue()));break;
   case "clock":core.game.node.entity.player.link.System.now=((Number)action.get("value")).longValue();break;
   default:throw new IllegalArgumentException();}return null;});
   outputs.add(map("result",result,"state",map("activity",snapshot(data.getActivityData()),"quest",snapshot(data.getQuestData()),"global",snapshot(data.getGlobalData())),"root",root,"flags",Arrays.asList(data.getActivityData().hasStartedKolodion(),data.getActivityData().hasKilledKolodion(),data.getActivityData().hasRecievedKolodionReward(),data.getActivityData().hasKilledBork(),data.getGlobalData().hasReadPlaques())));
  }return outputs;
 }
 public static void main(String[] args)throws Exception {if(!java.lang.System.getProperty("java.specification.version").equals("11"))throw new IllegalStateException("Use JDK 11 for saved numeric strings");BufferedReader reader=new BufferedReader(new InputStreamReader(java.lang.System.in,"UTF-8"));String line;while((line=reader.readLine())!=null){JSONObject input=(JSONObject)new JSONParser().parse(line);input.put("expected",evaluate(input));java.lang.System.out.println(JSONValue.toJSONString(input));}}
}
