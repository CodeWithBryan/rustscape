package oracle;
import core.ServerConstants;
import core.cache.def.impl.ItemDefinition;
import core.game.system.config.ItemConfigParser;
import core.game.world.update.flag.context.Animation;
import core.game.node.entity.player.link.audio.Audio;
import java.io.*;
import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.*;

public class ItemConfigOracle {
 static Path files;static int serial;
 static Map<String,Object> map(Object... pairs){Map<String,Object> m=new TreeMap<>();for(int i=0;i<pairs.length;i+=2)m.put((String)pairs[i],pairs[i+1]);return m;}
 static Field field(Class<?> type,String name)throws Exception{while(type!=null){try{Field f=type.getDeclaredField(name);f.setAccessible(true);return f;}catch(NoSuchFieldException e){type=type.getSuperclass();}}throw new NoSuchFieldException(name);}
 static Object get(Object o,String name){try{return field(o.getClass(),name).get(o);}catch(Exception e){throw new RuntimeException(e);}}
 static int number(Object value){return ((Number)value).intValue();}
 static Object ref(Object value,IdentityHashMap<Object,Integer> ids){return ids.computeIfAbsent(value,k->ids.size());}
 static Object property(Object value,IdentityHashMap<Object,Integer> ids){
  if(value==null)return map("type","null");
  if(value instanceof Integer)return map("type","int","value",value);
  if(value instanceof Long)return map("type","long","value",value);
  if(value instanceof Double)return map("type","double","bits",String.format("%016x",Double.doubleToLongBits((Double)value)));
  if(value instanceof Boolean)return map("type","bool","value",value);
  if(value instanceof String)return map("type","text","value",value);
  Object tag=ref(value,ids);
  if(value instanceof int[]){List<Integer> items=new ArrayList<>();for(int item:(int[])value)items.add(item);return map("type","ints","ref",tag,"values",items);}
  if(value instanceof Animation){Animation a=(Animation)value;return map("type","animation","ref",tag,"id",a.getId(),"delay",a.getDelay(),"priority",a.getPriority().toString());}
  if(value instanceof Audio){Audio a=(Audio)value;return map("type","audio","ref",tag,"id",a.id,"delay",a.delay,"loops",a.loops,"radius",a.radius);}
  if(value instanceof Animation[]||value instanceof Audio[]){List<Object> values=new ArrayList<>();for(Object v:(Object[])value)values.add(property(v,ids));return map("type",value instanceof Animation[]?"animations":"audios","ref",tag,"values",values);}
  if(value instanceof Map){Map<String,Object> values=new TreeMap<>();List<Integer> order=new ArrayList<>();for(Object key:((Map<?,?>)value).keySet()){order.add((Integer)key);values.put(key.toString(),((Map<?,?>)value).get(key));}return map("type","requirements","ref",tag,"values",values,"order",order);}
  throw new AssertionError(value.getClass());
 }
 static Object state(ItemConfigParser parser,List<Object> retained){
  IdentityHashMap<Object,Integer> ids=new IdentityHashMap<>();Map<String,Object> definitions=new TreeMap<>();
  Map<Integer,ItemDefinition> sorted=new TreeMap<>(ItemDefinition.getDefinitions());
  for(Map.Entry<Integer,ItemDefinition> entry:sorted.entrySet()){ItemDefinition def=entry.getValue();Map<String,Object> handlers=new TreeMap<>();for(Map.Entry<String,Object> e:new TreeMap<>(def.getHandlers()).entrySet())handlers.put(e.getKey(),property(e.getValue(),ids));definitions.put(entry.getKey().toString(),map("id",def.getId(),"name",def.getName(),"examine",get(def,"examine"),"value",get(def,"value"),"handlers",handlers));}
  List<Object> captures=new ArrayList<>();for(Object value:retained)captures.add(property(value,ids));
  return map("reader",parser.getReader()!=null,"definitions",definitions,"retained",captures,"logs",new ArrayList<>(Events.logs));
 }
 static void initialize(Map<?,?> values)throws Exception{
  ItemDefinition.getDefinitions().clear();for(Map.Entry<?,?> entry:values.entrySet()){
   ItemDefinition def=new ItemDefinition();for(Map.Entry<?,?> e:((Map<?,?>)entry.getValue()).entrySet()){
    String key=(String)e.getKey();Object value=e.getValue();if(key.equals("handlers")){for(Map.Entry<?,?> h:((Map<?,?>)value).entrySet()){Object v=h.getValue();def.getHandlers().put((String)h.getKey(),v instanceof Long?number(v):v);}}
    else if(Arrays.asList("id","name","value","examine").contains(key)){Field f=field(ItemDefinition.class,key);f.set(def,value instanceof Long?number(value):value);}
   }ItemDefinition.getDefinitions().put(Integer.parseInt((String)entry.getKey()),def);
  }
 }
 static Object evaluate(Map<?,?> request)throws Exception{
  if(request.get("kind").equals("json")){try{Object value=new JSONParser().parse((String)request.get("text"));String type=value==null?"null":value instanceof JSONArray?"array":value instanceof JSONObject?"object":value instanceof String?"text":value instanceof Long?"long":value instanceof Double?"double":"bool";return map("error",null,"position",null,"type",type,"text",String.valueOf(value));}catch(Exception e){return map("error",e.getClass().getSimpleName(),"position",e instanceof ParseException?((ParseException)e).getPosition():null);}}
  if(request.get("kind").equals("constants")){Map<String,Object> constants=new TreeMap<>();for(Field f:ItemConfigParser.class.getDeclaredFields())if(Modifier.isStatic(f.getModifiers())&&f.getType()==String.class)constants.put(f.getName(),f.get(null));return constants;}
  Map<?,?> initial=(Map<?,?>)request.get("initial");if(initial==null)initial=map("0",map("id",0,"name","zero"),"1",map("id",1,"name","original","value",100,"examine","cached","handlers",map("old",true)),"99",map("id",99));initialize(initial);
  Events.logs.clear();Events.fail=Boolean.TRUE.equals(request.get("logFail"));ItemConfigParser parser=new ItemConfigParser();List<Object> retained=new ArrayList<>(),output=new ArrayList<>();
  for(Object row:(List<?>)request.get("steps")){
   Map<?,?> step=(Map<?,?>)row;String error=null;Integer position=null;
   try{
    if(step.containsKey("capture")){for(Object key:(List<?>)step.get("capture"))retained.add(ItemDefinition.forId(1).getHandlers().get(key));}
    else if(step.containsKey("mutate")){((Map<Integer,Integer>)retained.get(number(step.get("mutate")))).put(number(step.get("key")),number(step.get("value")));}
    else{
     Path directory=files.resolve("input-"+(serial++));Files.createDirectories(directory);ServerConstants.CONFIG_PATH=directory.toString()+File.separator;
     if(!Boolean.TRUE.equals(step.get("missing")))Files.write(directory.resolve("item_configs.json"),((String)step.get("text")).getBytes(java.nio.charset.StandardCharsets.UTF_8));parser.load();
    }
   }catch(Exception e){error=e.getClass().getSimpleName();if(e instanceof ParseException)position=((ParseException)e).getPosition();}
   output.add(map("error",error,"position",position,"state",state(parser,retained)));
  }
  if(parser.getReader()!=null)parser.getReader().close();return output;
 }
 public static void main(String[] args)throws Exception{files=Paths.get(args[0]);Files.createDirectories(files);BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));JSONParser parser=new JSONParser();String line;while((line=reader.readLine())!=null){Map<String,Object> request=(Map<String,Object>)parser.parse(line);request.put("expected",evaluate(request));System.out.println(JSONValue.toJSONString(request));}}
}
