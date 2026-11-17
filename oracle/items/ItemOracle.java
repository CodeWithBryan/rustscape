import core.cache.Cache;
import core.cache.def.Definition;
import core.cache.def.impl.ItemDefinition;
import java.io.*;
import java.lang.reflect.*;
import java.nio.ByteBuffer;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;

public class ItemOracle {
    static final List<Field> FIELDS=new ArrayList<>();
    static final Set<String> SHARED=new HashSet<>(Arrays.asList("options","groundActions","originalModelColors","modifiedModelColors","textureColour1","textureColour2","unknownArray1","unknownArray2","unknownArray3","stackIds","stackAmounts","itemRequirements","clientScriptData"));
    static {for(Class<?> type:Arrays.asList(Definition.class,ItemDefinition.class))for(Field field:type.getDeclaredFields())if(!Modifier.isStatic(field.getModifiers())){field.setAccessible(true);FIELDS.add(field);}FIELDS.sort(Comparator.comparing(Field::getName));}
    static Field field(String name){return FIELDS.stream().filter(f->f.getName().equals(name)).findFirst().orElseThrow();}
    static Map<String,Object> map(Object... values){Map<String,Object> m=new TreeMap<>();for(int i=0;i<values.length;i+=2)m.put((String)values[i],values[i+1]);return m;}
    static int num(Map<?,?> m,String key){return ((Number)m.get(key)).intValue();}
    static byte[] bytes(String hex){byte[] b=new byte[hex.length()/2];for(int i=0;i<b.length;i++)b[i]=(byte)Integer.parseInt(hex.substring(2*i,2*i+2),16);return b;}
    static Object json(Object value){
        if(value==null||value instanceof String||value instanceof Number||value instanceof Boolean)return value;
        if(value.getClass().isArray()){List<Object> result=new ArrayList<>();for(int i=0;i<Array.getLength(value);i++)result.add(json(Array.get(value,i)));return result;}
        if(value instanceof Map){Map<String,Object> result=new TreeMap<>();((Map<?,?>)value).forEach((key,v)->result.put(key.toString(),json(v)));return result;}
        throw new AssertionError(value.getClass());
    }
    static Object convert(Object value,Class<?> type){
        if(value==null)return null;
        if(type.isArray()){List<?> list=(List<?>)value;Object result=Array.newInstance(type.getComponentType(),list.size());for(int i=0;i<list.size();i++)Array.set(result,i,convert(list.get(i),type.getComponentType()));return result;}
        if(type==int.class||type==Integer.class)return ((Number)value).intValue();
        if(type==short.class)return ((Number)value).shortValue();
        if(type==byte.class)return ((Number)value).byteValue();
        if(value instanceof Map){Map<Integer,Object> result=new HashMap<>();((Map<?,?>)value).forEach((k,v)->result.put(Integer.parseInt((String)k),scalar(v)));return result;}
        return scalar(value);
    }
    static Object scalar(Object value){return value instanceof Long?((Long)value).intValue():value;}
    static ItemDefinition make(Map<?,?> fields)throws Exception{
        ItemDefinition def=new ItemDefinition();
        for(Map.Entry<?,?> entry:fields.entrySet()){
            Field f=field((String)entry.getKey());
            if(f.getName().equals("handlers")){((Map<Object,Object>)f.get(def)).clear();((Map<?,?>)entry.getValue()).forEach((k,v)->((Map<Object,Object>)get(f,def)).put(k,scalar(v)));}
            else f.set(def,convert(entry.getValue(),f.getType()));
        }
        return def;
    }
    static Object get(Field f,Object def){try{return f.get(def);}catch(Exception e){throw new RuntimeException(e);}}
    static Map<String,Object> snapshot(ItemDefinition def){Map<String,Object> result=new TreeMap<>();for(Field f:FIELDS)result.put(f.getName(),json(get(f,def)));return result;}
    static Object state(Map<?,?> request){
        Map<Integer,ItemDefinition> definitions=ItemDefinition.getDefinitions();
        List<Integer> ids=new ArrayList<>();if(request.containsKey("select"))for(Object id:(List<?>)request.get("select"))ids.add(((Number)id).intValue());else ids.addAll(definitions.keySet());Collections.sort(ids);
        Map<String,Object> records=new TreeMap<>(),identities=new TreeMap<>();IdentityHashMap<Object,Integer> identity=new IdentityHashMap<>();
        for(int id:ids){ItemDefinition def=definitions.get(id);if(def==null)continue;String key=""+id;records.put(key,snapshot(def));Map<String,Object> refs=new TreeMap<>();refs.put("self",identity.computeIfAbsent(def,k->identity.size()));for(Field f:FIELDS)if(SHARED.contains(f.getName())){Object value=get(f,def);refs.put(f.getName(),value==null?null:identity.computeIfAbsent(value,k->identity.size()));}identities.put(key,refs);}
        return map("definitions",records,"identities",identities,"size",definitions.size());
    }
    static Object query(ItemDefinition d){return map("stackable",d.isStackable(),"playerType",d.isPlayerType(),"destroy",d.hasDestroyAction(),"wear",d.hasWearAction(),"special",d.hasSpecialBar(),"quest",d.getQuestId(),"archive",d.getArchiveId(),"file",d.getFileId(),"nullAction",d.hasAction(null),"take",d.hasAction("take"));}
    static Object action(Map<?,?> a)throws Exception{
        String op=(String)a.get("op");
        if(op.equals("note")||op.equals("lend")||op.equals("recolour")){
            ItemDefinition target=ItemDefinition.forId(num(a,"to")),base=ItemDefinition.forId(num(a,"base")),template=ItemDefinition.forId(num(a,"template"));
            if(op.equals("note"))target.transferNoteDefinition(base,template);else if(op.equals("lend"))target.transferLendDefinition(base,template);else target.transferRecolourDefinition(base,template);
        }else if(op.equals("query"))return query(ItemDefinition.forId(num(a,"slot")));
        else if(op.equals("templates")){Cache.count=num(a,"count");ItemDefinition.defineTemplates();}
        else if(op.equals("load")){
            Cache.count=num(a,"count");Cache.files.clear();((Map<?,?>)a.get("files")).forEach((k,v)->{if(v!=null)Cache.files.put(Integer.parseInt((String)k),bytes((String)v));});ItemDefinition.parse();
        }else if(op.equals("missing")){
            ItemDefinition first=ItemDefinition.forId(num(a,"id")),second=ItemDefinition.forId(num(a,"id"));field("name").set(first,"changed");return map("same",first==second,"first",snapshot(first),"second",snapshot(second));
        }else if(op.equals("alias"))ItemDefinition.getDefinitions().put(num(a,"slot"),ItemDefinition.forId(num(a,"source")));
        else if(op.equals("mutate")){
            Field f=field((String)a.get("field"));Object value=f.get(ItemDefinition.forId(num(a,"slot")));
            if(value instanceof Map)((Map<Object,Object>)value).put(Integer.parseInt((String)a.get("key")),scalar(a.get("value")));
            else Array.set(value,num(a,"index"),convert(a.get("value"),value.getClass().getComponentType()));
        }else throw new AssertionError(op);
        return null;
    }
    static Object evaluate(Map<String,Object> request)throws Exception{
        if(request.get("kind").equals("casePairs")){List<Object> result=new ArrayList<>();for(int c=0;c<=65535;c++){char folded=Character.toLowerCase(Character.toUpperCase((char)c));if(folded!=c)result.add(Arrays.asList(c,(int)folded));}return result;}
        if(request.get("kind").equals("caseCompare"))return ((String)request.get("left")).equalsIgnoreCase((String)request.get("right"));
        if(request.get("kind").equals("decode")){
            ByteBuffer buffer=ByteBuffer.wrap(bytes((String)request.get("data")));ItemDefinition def=null;String error=null;
            try{def=ItemDefinition.parseDefinition(num(request,"id"),buffer);}catch(Exception e){error=e.getClass().getSimpleName();}
            return map("definition",def==null?null:snapshot(def),"position",buffer.position(),"error",error);
        }
        ItemDefinition.getDefinitions().clear();Cache.files.clear();
        for(Object row:(List<?>)request.get("initial")){Map<?,?> r=(Map<?,?>)row;ItemDefinition.getDefinitions().put(num(r,"slot"),make((Map<?,?>)r.get("fields")));}
        List<Object> trace=new ArrayList<>();
        for(Object a:(List<?>)request.get("actions")){
            Object result=null;String error=null;try{result=action((Map<?,?>)a);}catch(Exception e){error=e.getClass().getSimpleName();}
            trace.add(map("result",result,"error",error,"state",state(request)));
        }
        return trace;
    }
    public static void main(String[] args)throws Exception{
        BufferedReader input=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;JSONParser parser=new JSONParser();
        while((line=input.readLine())!=null){Map<String,Object> request=(Map<String,Object>)parser.parse(line);request.put("expected",evaluate(request));System.out.println(JSONValue.toJSONString(request));}
    }
}
