package oracle;
import org.json.simple.*;
import core.game.node.entity.skill.Skills;
import core.game.node.entity.Entity;
import java.lang.reflect.*;
import java.util.*;
public class Model {
 public static JSONObject config;public static Entity owner;public static int ticks;public static String failure="";public static List<Object> trace=new ArrayList<>();public static Set<Integer> equipment=new HashSet<>();
 public static String bits(double value){return String.format("%016x",Double.doubleToLongBits(value));}
 public static boolean flag(String key){return Boolean.TRUE.equals(config.get(key));}public static int integer(String key,int fallback){return config.get(key)==null?fallback:((Number)config.get(key)).intValue();}public static double number(String key,double fallback){return config.get(key)==null?fallback:((Number)config.get(key)).doubleValue();}
 public static void call(String tag,Object... values){List<Object> row=new ArrayList<>();row.add(tag);row.addAll(Arrays.asList(values));trace.add(row);if(failure.equals(tag))throw new RuntimeException("binding");}
 public static void mutate(Skills state,String stage){JSONObject all=(JSONObject)config.get("mutations");if(all==null||!all.containsKey(stage))return;JSONObject mutation=(JSONObject)all.get(stage);try{if(mutation.containsKey("skill")){Field field=Skills.class.getDeclaredField("experience");field.setAccessible(true);((double[])field.get(state))[((Number)mutation.get("skill")).intValue()]=Double.parseDouble((String)mutation.get("xp"));}if(mutation.containsKey("tick"))ticks=((Number)mutation.get("tick")).intValue();}catch(ReflectiveOperationException e){throw new RuntimeException(e);}}
 public static Object snapshot(Skills state)throws Exception {Map<String,Object> out=new TreeMap<>();for(Field f:Skills.class.getDeclaredFields()){if(Modifier.isStatic(f.getModifiers())||f.getName().equals("entity"))continue;f.setAccessible(true);out.put(f.getName(),data(f.get(state)));}return out;}
 public static void setup(Skills state,JSONObject values)throws Exception {for(Object raw:values.keySet()){String key=(String)raw;Field field=Skills.class.getDeclaredField(key);field.setAccessible(true);Object value=values.get(key);if(field.getType().isArray()){Object array=field.get(state);List<?> input=(List<?>)value;for(int i=0;i<input.size();i++){if(array instanceof double[])((double[])array)[i]=Double.parseDouble((String)input.get(i));else ((int[])array)[i]=((Number)input.get(i)).intValue();}}else if(field.getType()==double.class)field.setDouble(state,Double.parseDouble((String)value));else if(field.getType()==int.class)field.setInt(state,((Number)value).intValue());else if(field.getType()==boolean.class)field.setBoolean(state,(Boolean)value);}}
 public static Object data(Object value){if(value instanceof Double)return bits((Double)value);if(value!=null&&value.getClass().isArray()){List<Object> out=new ArrayList<>();for(int i=0;i<Array.getLength(value);i++)out.add(data(Array.get(value,i)));return out;}return value;}
}
