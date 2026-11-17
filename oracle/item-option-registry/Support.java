package oracle;
import java.io.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.cache.def.impl.ItemDefinition;import core.game.interaction.OptionHandler;
public class Support{
 public static List<ItemDefinition>definitions;public static boolean nullDefinition;static Map input;static List<Object>events;static Map<String,Integer>calls;
 static OptionHandler[]handlers;static List<Map<String,OptionHandler>>maps;static Set<String>keys;
 public static class Failure extends RuntimeException{}
 static int n(Object o){return((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 public static void emit(String event,Object...args){List<Object>row=new ArrayList<>();row.add(event);row.addAll(Arrays.asList(args));events.add(row);int at=calls.merge(event,1,Integer::sum);for(Object o:(List)input.getOrDefault("hooks",Collections.emptyList())){Map h=(Map)o;if(event.equals(h.get("event"))&&at==v(h,"at",1))for(Object a:(List)h.get("actions"))act((Map)a);}}
 static OptionHandler handler(Map a){return a.get("handler")==null?null:handlers[n(a.get("handler"))];}
 static Integer id(OptionHandler h){return h==null?null:h.id;}
 static Object act(Map a){String name=(String)a.get("name");Map<String,OptionHandler>map=maps.isEmpty()?ItemDefinition.getOptionHandlers():maps.get(v(a,"map",0));switch((String)a.get("op")){
  case"fail":throw new Failure();case"capture":maps.add(ItemDefinition.getOptionHandlers());return maps.size()-1;
  case"register":keys.add(name);return ItemDefinition.setOptionHandler(name,handler(a));case"put":keys.add(name);return id(map.put(name,handler(a)));case"get":return id(map.get(name));case"contains":return map.containsKey(name);case"remove":return id(map.remove(name));case"clear":map.clear();return null;
  case"lookup":return id(ItemDefinition.getOptionHandler(v(a,"id",1),name));
  case"config":Object value=a.get("value");if(value instanceof Number)value=handlers[n(value)];definitions.get(v(a,"definition",0)).handlers.put((String)a.get("key"),value);return null;
  case"set":if(a.containsKey("nullDefinition"))nullDefinition=Boolean.TRUE.equals(a.get("nullDefinition"));return null;
  case"cache":ItemDefinition.DEFINITIONS.put(v(a,"id",1),a.get("definition")==null?null:definitions.get(n(a.get("definition"))));return null;
  default:throw new IllegalArgumentException();}}
 static Object snapshot(Object result,String error){List<Object>global=new ArrayList<>(),defs=new ArrayList<>(),cache=new ArrayList<>();Map<String,OptionHandler>map=ItemDefinition.getOptionHandlers();for(String key:keys)if(map.containsKey(key))global.add(Arrays.asList(key,id(map.get(key))));
  for(ItemDefinition d:definitions){List<Object>values=new ArrayList<>();for(String key:new TreeSet<>(d.handlers.keySet())){Object value=d.handlers.entrySet().stream().filter(e->e.getKey().equals(key)).findFirst().get().getValue();values.add(Arrays.asList(key,value instanceof OptionHandler?Arrays.asList("handler",id((OptionHandler)value)):value));}defs.add(values);}
  for(Integer key:new TreeSet<>(ItemDefinition.DEFINITIONS.keySet())){ItemDefinition d=ItemDefinition.DEFINITIONS.get(key);cache.add(Arrays.asList(key,d==null?null:d.key));}
  Map<String,Object>r=new TreeMap<>();r.put("result",result);r.put("error",error);r.put("events",new ArrayList<>(events));r.put("global",global);r.put("size",map.size());r.put("empty",map.isEmpty());r.put("definitions",defs);r.put("cache",cache);r.put("captures",maps.size());r.put("nullDefinition",nullDefinition);return r;}
 public static void main(String[]args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){input=(Map)new JSONParser().parse(line);Map initial=new TreeMap(input);events=new ArrayList<>();calls=new HashMap<>();definitions=new ArrayList<>();maps=new ArrayList<>();keys=new TreeSet<>(Comparator.nullsFirst(Comparator.naturalOrder()));handlers=new OptionHandler[]{new OptionHandler(0),new OptionHandler(1),new OptionHandler(2)};nullDefinition=Boolean.TRUE.equals(input.get("nullDefinition"));
  ItemDefinition.getOptionHandlers().clear();ItemDefinition.DEFINITIONS.clear();new ItemDefinition();new ItemDefinition();ItemDefinition.DEFINITIONS.put(1,definitions.get(0));ItemDefinition.DEFINITIONS.put(2,definitions.get(1));
  List<Object>states=new ArrayList<>();for(Object a:(List)input.get("actions")){events.clear();Object result=null;String error=null;try{result=act((Map)a);}catch(Failure e){error="Binding";}catch(ClassCastException e){error="Type";}states.add(snapshot(result,error));}initial.put("expected",states);System.out.println(JSONValue.toJSONString(initial));
 }}}
}
