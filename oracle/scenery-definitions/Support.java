package oracle;
import core.cache.Cache;import core.cache.def.Definition;import core.cache.def.impl.SceneryDefinition;
import java.io.*;import java.lang.reflect.*;import java.nio.ByteBuffer;import java.util.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
public class Support {
 static final List<Field>FIELDS=new ArrayList<>();static Map request;static TreeMap<String,SceneryDefinition>retained;static List<Object>events;static int logs;
 static{for(Class<?>c:Arrays.asList(Definition.class,SceneryDefinition.class))for(Field f:c.getDeclaredFields())if(!Modifier.isStatic(f.getModifiers())){f.setAccessible(true);FIELDS.add(f);}FIELDS.sort(Comparator.comparing(Field::getName));}
 public static class Failure extends RuntimeException{}
 static int n(Object o){return((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}static Map<String,Object>map(Object...p){Map<String,Object>m=new TreeMap<>();for(int i=0;i<p.length;i+=2)m.put((String)p[i],p[i+1]);return m;}
 public static void log(String s){events.add(Arrays.asList("log",s));if(++logs==v(request,"failLog",-1))throw new Failure();}
 public static Integer varbit(int id){events.add(Arrays.asList("varbit",id));if(Boolean.TRUE.equals(request.get("failVarbit")))throw new Failure();return Boolean.TRUE.equals(request.get("missingVarbit"))?null:v(request,"configValue",0);}
 public static int varp(int id){events.add(Arrays.asList("varp",id));if(Boolean.TRUE.equals(request.get("failVarp")))throw new Failure();return v(request,"configValue",0);}
 static Field field(String name){return FIELDS.stream().filter(f->f.getName().equals(name)).findFirst().orElseThrow();}
 static Object get(Field f,Object h){try{return f.get(h);}catch(Exception e){throw new RuntimeException(e);}}
 static Object scalar(Object x){return x instanceof Long?((Long)x).intValue():x;}
 static Object convert(Object x,Class<?>t){if(x==null)return null;if(t.isArray()){List l=(List)x;Object a=Array.newInstance(t.getComponentType(),l.size());for(int i=0;i<l.size();i++)Array.set(a,i,convert(l.get(i),t.getComponentType()));return a;}if(t==int.class)return n(x);if(t==short.class)return((Number)x).shortValue();if(t==byte.class)return((Number)x).byteValue();if(x instanceof Map){Map<String,Object>m=new HashMap<>();((Map<String,Object>)x).forEach((k,v)->m.put(k,scalar(v)));return m;}return scalar(x);}
 static Object json(Object x){if(x==null||x instanceof String||x instanceof Number||x instanceof Boolean)return x;if(x.getClass().isArray()){List<Object>a=new ArrayList<>();for(int i=0;i<Array.getLength(x);i++)a.add(json(Array.get(x,i)));return a;}if(x instanceof Map){Map<String,Object>m=new TreeMap<>();((Map<?,?>)x).forEach((k,v)->m.put(k.toString(),json(v)));return m;}throw new AssertionError(x.getClass());}
 static Map<String,Object>snapshot(SceneryDefinition h){if(h==null)return null;Map<String,Object>m=new TreeMap<>();for(Field f:FIELDS)m.put(f.getName(),json(get(f,h)));return m;}
 static Object refs(SceneryDefinition h,IdentityHashMap<Object,Integer>ids){if(h==null)return null;Map<String,Object>m=new TreeMap<>();m.put("self",ids.computeIfAbsent(h,k->ids.size()));for(Field f:FIELDS)if(f.getType().isArray()||f.getName().equals("handlers")){Object a=get(f,h);m.put(f.getName(),a==null?null:ids.computeIfAbsent(a,k->ids.size()));}return m;}
 public static Object state(){Map<String,Object>defs=new TreeMap<>(),identities=new TreeMap<>(),rs=new TreeMap<>(),ri=new TreeMap<>();IdentityHashMap<Object,Integer>ids=new IdentityHashMap<>();List<Integer>keys=new ArrayList<>(SceneryDefinition.getDefinitions().keySet());Collections.sort(keys);for(int k:keys){SceneryDefinition h=SceneryDefinition.getDefinitions().get(k);defs.put(""+k,snapshot(h));identities.put(""+k,refs(h,ids));}for(String k:retained.keySet()){rs.put(k,snapshot(retained.get(k)));ri.put(k,refs(retained.get(k),ids));}return map("definitions",defs,"identities",identities,"size",keys.size(),"retained",rs,"retainedIdentities",ri);}
 static void set(SceneryDefinition h,String name,Object value)throws Exception{Field f=field(name);if(name.equals("handlers")){Map m=(Map)f.get(h);m.clear();((Map<String,Object>)value).forEach((k,v)->m.put(k,scalar(v)));}else f.set(h,convert(value,f.getType()));}
 static SceneryDefinition make(Map<String,Object>m)throws Exception{SceneryDefinition h=new SceneryDefinition();for(String k:m.keySet())set(h,k,m.get(k));return h;}
 static SceneryDefinition target(Map a){return a.containsKey("ref")?retained.get(a.get("ref")):SceneryDefinition.forId(v(a,"slot",0));}
 public static byte[]bytes(String s){byte[]b=new byte[s.length()/2];for(int i=0;i<b.length;i++)b[i]=(byte)Integer.parseInt(s.substring(i*2,i*2+2),16);return b;}
 static Object action(Map a)throws Throwable{String op=(String)a.get("op");switch(op){
 case"load":{Cache.count=v(a,"count",0);Cache.files.clear();Map<String,String>files=(Map)a.get("files");files.forEach((k,v)->Cache.files.put(Integer.parseInt(k),bytes(v)));Cache.failFile=a.containsKey("failFile")?n(a.get("failFile")):null;SceneryDefinition.parse();return null;}
 case"put":SceneryDefinition.getDefinitions().put(n(a.get("slot")),make((Map)a.get("fields")));return null;
 case"decode":SceneryDefinition.getDefinitions().put(n(a.get("slot")),SceneryDefinition.parseDefinition(v(a,"id",0),ByteBuffer.wrap(bytes((String)a.get("data")))));return null;
 case"retain":retained.put((String)a.get("name"),target(a));return null;
 case"alias":SceneryDefinition.getDefinitions().put(n(a.get("to")),target(a));return null;
 case"set":{Map<String,Object>m=(Map)a.get("fields");for(String k:m.keySet())set(target(a),k,m.get(k));return null;}
 case"share":{Field f=field((String)a.get("field"));f.set(SceneryDefinition.forId(n(a.get("to"))),f.get(target(a)));return null;}
 case"array":{Object ar=field((String)a.get("field")).get(target(a));int index=n(a.get("index"));Array.set(ar,index,convert(a.get("value"),ar.getClass().getComponentType()));return null;}
 case"configure":{Method m=SceneryDefinition.class.getDeclaredMethod("configureObject");m.setAccessible(true);m.invoke(target(a));return null;}
 case"hasOptions":return target(a).hasOptions(Boolean.TRUE.equals(a.get("examine")));
 case"hasActions":return target(a).hasActions();case"hasAction":return target(a).hasAction((String)a.get("value"));
 case"childIndex":{SceneryDefinition h=target(a).getChildObjectAtIndex(n(a.get("index")));if(a.containsKey("name"))retained.put((String)a.get("name"),h);return snapshot(h);}
 case"child":{SceneryDefinition h=target(a).getChildObject(Boolean.TRUE.equals(a.get("player"))?new core.game.node.entity.player.Player():null);if(a.containsKey("name"))retained.put((String)a.get("name"),h);return snapshot(h);}
 case"container":return SceneryDefinition.getContainerId(n(a.get("id")));
 default:throw new IllegalArgumentException(op);}}
 static String error(Throwable e){while(e instanceof InvocationTargetException)e=((InvocationTargetException)e).getCause();return e instanceof Failure?"Binding":e.getClass().getSimpleName();}
 public static Object evaluate(Map input)throws Exception{request=input;logs=0;retained=new TreeMap<>();events=new ArrayList<>();SceneryDefinition.getDefinitions().clear();Cache.files.clear();Cache.failFile=null;
 if(input.get("kind").equals("decode")){ByteBuffer r=ByteBuffer.wrap(bytes((String)input.get("data")));SceneryDefinition h=null;String error=null;try{h=SceneryDefinition.parseDefinition(v(input,"id",0),r);}catch(Throwable e){error=error(e);}return map("definition",snapshot(h),"state",state(),"position",r.position(),"error",error,"events",new ArrayList<>(events));}
 for(Object r:(List)input.getOrDefault("initial",new ArrayList<>())){Map row=(Map)r;SceneryDefinition.getDefinitions().put(n(row.get("slot")),make((Map)row.get("fields")));}
 List<Object>out=new ArrayList<>();for(Object a:(List)input.get("actions")){events.clear();Object result=null;String error=null;try{result=action((Map)a);}catch(Throwable e){error=error(e);}out.add(map("result",result,"error",error,"state",state(),"events",new ArrayList<>(events)));}return out;}
 public static void main(String[]args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));BufferedWriter w=new BufferedWriter(new OutputStreamWriter(System.out,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){JSONObject c=(JSONObject)new JSONParser().parse(line);c.put("expected",evaluate(c));JSONValue.writeJSONString(c,w);w.newLine();w.flush();}}}
}
