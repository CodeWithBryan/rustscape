package oracle;
import java.util.*;import java.io.*;import java.nio.file.*;import java.lang.reflect.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.scenery.*;import core.game.node.entity.*;import core.game.node.entity.player.*;import core.game.world.map.*;import core.game.global.action.DoorActionHandler;import core.game.system.config.DoorConfigLoader;import core.game.system.task.Pulse;
public class Support {
 static Entity entity;static TreeMap<String,Scenery> nodes;static List<String> region;static List<Pulse> queue,tasks;static List<Object> events;static Map<String,Integer> calls;static List<Map> hooks;static JSONObject input;static boolean enabled;public static boolean moveNear;static int generated,fileCounter;static Map<String,DoorConfigLoader.Door> refs;static DoorConfigLoader loader;
 public static class Failure extends RuntimeException{public void printStackTrace(){events.add(Arrays.asList("exception"));}}
 static int n(Object v){return((Number)v).intValue();}static int val(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}static boolean b(Map m,String k){return Boolean.TRUE.equals(m.get(k));}
 static String str(Map m,String k,String d){return m.get(k)==null?d:(String)m.get(k);}
 public static List<Integer> coords(Location l){return Arrays.asList(l.getX(),l.getY(),l.getZ());}
 static Location location(Object v){List a=(List)v;return new Location(n(a.get(0)),n(a.get(1)),n(a.get(2)));}
 static String label(Scenery o){if(o==null)return null;for(Map.Entry<String,Scenery> e:nodes.entrySet())if(e.getValue()==o)return e.getKey();String name="g"+generated++;nodes.put(name,o);return name;}
 public static void emit(String event,Object...args){if(!enabled)return;List<Object> row=new ArrayList<>();row.add(event);Collections.addAll(row,args);events.add(row);int count=calls.merge(event,1,Integer::sum);for(Map hook:hooks)if(event.equals(hook.get("event"))&&count==val(hook,"at",1))for(Object action:(List)hook.get("actions"))act((Map)action);}
 public static Scenery object(Location l,int type){emit("object",coords(l),type);for(String key:region){Scenery o=nodes.get(key);if(o.getType()==type&&o.getLocation().equals(l))return o;}return null;}
 public static boolean replace(Scenery a,Scenery b,Integer ticks,boolean clip){String first=label(a),second=label(b);emit("replace",first,second,ticks,clip);return !Boolean.FALSE.equals(input.get("replaceResult"));}
 public static void submit(Pulse p){emit("submit",tasks.size());tasks.add(p);queue.add(p);}
 static void load(String text){try{Path dir=Paths.get("runs/doors-oracle/config-"+(fileCounter++));Files.createDirectories(dir);Files.write(dir.resolve("door_configs.json"),text.getBytes(java.nio.charset.StandardCharsets.UTF_8));core.ServerConstants.CONFIG_PATH=dir.toString()+File.separator;loader.load();}catch(RuntimeException e){throw e;}catch(Exception e){throw new Wrapped(e);}}
 static class Wrapped extends RuntimeException{final Exception original;Wrapped(Exception e){original=e;}}
 static DoorConfigLoader.Door config(int id){return DoorConfigLoader.Companion.forId(id);}
 static Scenery lookup(Object name){return name==null?null:nodes.get(name);}
 static Object act(Map a){String op=(String)a.get("op");Scenery o=lookup(a.get("node")),second=lookup(a.get("second"));switch(op){
 case "handle":DoorActionHandler.handleDoor((Player)entity,o);return null;
 case "end":return coords(DoorActionHandler.getEndLocation(entity,o));
 case "destination":return coords(DoorActionHandler.getDestination(entity,o));
 case "second":return label(DoorActionHandler.getSecondDoor(o,entity));
 case "point":{java.awt.Point p=DoorActionHandler.getRotationPoint(n(a.get("value")));return p==null?null:Arrays.asList((int)p.getX(),(int)p.getY());}
 case "rotations":{List p=(List)a.get("point");return Arrays.stream(DoorActionHandler.getRotation(o,second,new java.awt.Point(n(p.get(0)),n(p.get(1))))).boxed().collect(java.util.stream.Collectors.toList());}
 case "open":DoorActionHandler.open(o,second,val(a,"id",1),val(a,"otherId",2),!Boolean.FALSE.equals(a.get("clip")),val(a,"ticks",500),b(a,"fence"));return null;
 case "autowalk":return DoorActionHandler.handleAutowalkDoorWithLocation(entity,o,location(a.get("end")));
 case "autowalkFence":return DoorActionHandler.autowalkFence(entity,o,val(a,"id",1),val(a,"otherId",2));
 case "tick":{int count=queue.size();for(int i=0;i<count;i++){Pulse p=queue.remove(0);if(!p.update()&&p.isRunning())queue.add(p);}return null;}
 case "logout":{if(entity instanceof Player){Player p=(Player)entity;kotlin.jvm.functions.Function1<Player,kotlin.Unit> callback=p.logoutListeners.get("autowalk");if(callback!=null)callback.invoke(p);}return null;}
 case "fail":throw new Failure();
 case "rotate":o.setRotation(n(a.get("value")));return null;
 case "charge":o.setCharge(n(a.get("value")));return null;
 case "own":entity.setLocation(location(a.get("value")));return null;
 case "region":region=new ArrayList<>((List<String>)a.get("value"));return null;
 case "load":load((String)a.get("text"));return null;
 case "retain":refs.put((String)a.get("name"),config(n(a.get("id"))));return null;
 case "configSet":{DoorConfigLoader.Door d=config(n(a.get("id")));if(a.get("field").equals("replaceId"))d.setReplaceId(n(a.get("value")));else if(a.get("field").equals("metal"))d.setMetal((Boolean)a.get("value"));else if(a.get("field").equals("fence"))d.setFence((Boolean)a.get("value"));else d.setAutoWalk((Boolean)a.get("value"));return null;}
 default:throw new IllegalArgumentException(op);
 }}
 static Map<String,Object> map(Object...pairs){Map<String,Object> m=new TreeMap<>();for(int i=0;i<pairs.length;i+=2)m.put((String)pairs[i],pairs[i+1]);return m;}
 static Object node(Scenery o){return map("id",o.getId(),"shape",o.getType(),"rotation",o.getRotation(),"direction",o.getDirection().value,"location",coords(o.getLocation()),"name",o.getName(),"close",o.getDefinition().hasAction("close"),"charge",o.getCharge(),"constructed",o instanceof Constructed,"replaced",o instanceof Constructed?label(((Constructed)o).getReplaced()):null,"wrapper",o.getWrapper()==o?null:label(o.getWrapper()));}
 static Object door(DoorConfigLoader.Door d){if(d==null)return null;return map("id",d.getId(),"replaceId",d.getReplaceId(),"fence",d.isFence(),"metal",d.isMetal(),"autowalk",d.isAutoWalk(),"quest",d.getQuestRequirement());}
 static Object snapshot(){boolean before=enabled;enabled=false;Map<String,Object> ns=new TreeMap<>();for(String key:new ArrayList<>(nodes.keySet()))ns.put(key,node(nodes.get(key)));Map<String,Object> ds=new TreeMap<>(),rs=new TreeMap<>();for(Integer key:DoorConfigLoader.Companion.getDOORS().keySet())ds.put(key.toString(),door(config(key)));for(String key:refs.keySet())rs.put(key,door(refs.get(key)));List<Object> ps=new ArrayList<>();for(Pulse p:tasks){try{Field ticks=Pulse.class.getDeclaredField("ticksPassed");ticks.setAccessible(true);Field opened=p.getClass().getDeclaredField("opened");opened.setAccessible(true);ps.add(map("running",p.isRunning(),"ticks",ticks.get(p),"opened",opened.get(p)));}catch(Exception e){throw new RuntimeException(e);}}List<Integer> qs=new ArrayList<>();for(Pulse p:queue)qs.add(tasks.indexOf(p));Object s=map("nodes",ns,"configs",ds,"refs",rs,"queue",qs,"pulses",ps,"own",coords(entity.getLocation()),"listener",entity instanceof Player&&((Player)entity).logoutListeners.containsKey("autowalk"),"events",new ArrayList<>(events));enabled=before;return s;}
 static String error(Throwable e){if(e instanceof Wrapped)return error(((Wrapped)e).original);if(e instanceof NullPointerException)return "Null";if(e instanceof NumberFormatException)return "Number";if(e instanceof ClassCastException)return "Type";if(e instanceof Failure)return "Binding";if(e instanceof IllegalArgumentException)return "Direction";if(e instanceof org.json.simple.parser.ParseException)return "Json";return e.getClass().getSimpleName();}
 static void setup(JSONObject c){input=c;enabled=false;nodes=new TreeMap<>();queue=new ArrayList<>();tasks=new ArrayList<>();events=new ArrayList<>();calls=new HashMap<>();hooks=(List<Map>)c.getOrDefault("hooks",new ArrayList<>());refs=new TreeMap<>();region=new ArrayList<>((List<String>)c.getOrDefault("region",new ArrayList<>()));generated=0;moveNear=b(c,"moveNear");entity=Boolean.FALSE.equals(c.get("player"))?new Entity():new Player();entity.setLocation(location(c.getOrDefault("own",Arrays.asList(3207,3217,0))));core.cache.def.impl.SceneryDefinition.defs.clear();DoorConfigLoader.Companion.getDOORS().clear();loader=new DoorConfigLoader();
  Map<String,Map> rows=new TreeMap<>((Map)c.get("nodes"));for(Map row:rows.values()){core.cache.def.impl.SceneryDefinition d=core.cache.def.impl.SceneryDefinition.forId(n(row.get("id")));d.name=str(row,"name","Door");d.close=b(row,"close");}
  for(Map.Entry<String,Map> e:rows.entrySet()){Map row=e.getValue();Scenery o=b(row,"constructed")?new Constructed(n(row.get("id")),location(row.get("location")),val(row,"shape",0),val(row,"rotation",0)):new Scenery(n(row.get("id")),location(row.get("location")),val(row,"shape",0),val(row,"rotation",0));o.setCharge(val(row,"charge",1000));nodes.put(e.getKey(),o);}
  for(Map.Entry<String,Map> e:rows.entrySet()){Scenery o=nodes.get(e.getKey());Map row=e.getValue();o.setWrapper(lookup(row.get("wrapper")));if(o instanceof Constructed)((Constructed)o).setReplaced(lookup(row.get("replaced")));}
  load(JSONValue.toJSONString(c.get("configs")));enabled=true;
 }
 public static void main(String[] args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){JSONObject c=(JSONObject)new JSONParser().parse(line);setup(c);List<Object> out=new ArrayList<>();for(Object action:(List)c.get("actions")){events.clear();Object result=null;String failure=null;try{result=act((Map)action);}catch(Throwable e){failure=error(e);}Map<String,Object> state=(Map<String,Object>)snapshot();state.put("result",result);state.put("error",failure);out.add(state);}c.put("expected",out);System.out.println(JSONValue.toJSONString(c));}}}
}
