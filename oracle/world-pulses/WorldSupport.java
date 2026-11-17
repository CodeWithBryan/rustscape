package oracle;
import java.lang.System;import java.util.*;import java.io.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.*;import core.game.node.entity.player.Player;import core.game.node.scenery.*;import core.game.node.item.*;import core.game.world.*;import core.game.world.map.*;import core.game.system.task.Pulse;import core.game.world.update.flag.chunk.ObjectUpdateFlag;
public class WorldSupport extends RuntimeSupport {
 static Map<String,Scenery>stored;static List<Scenery>held;public static List<ObjectUpdateFlag>flags;static Scenery closed,expiry;static int handleCalls;
 static String key(Scenery s){return s.location.getX()+":"+s.location.getY()+":"+s.location.getZ()+":"+(s.type==22?3:s.type<=3||s.type==9?0:s.type<=8?1:2);}
 public static Scenery remove(Scenery s){Scenery current=stored.get(key(s));if(current==null||current.id!=s.id)return null;stored.remove(key(s));current.active=false;s.active=false;return current;}
 // Original LandscapeParser.flagScenery activates its argument, while an empty
 // dynamic RegionChunk.add slot stores object.asConstructed(), not that argument.
 public static void add(Scenery s){s.active=true;Scenery current=stored.get(key(s));if(current!=null&&current.equals(s)){current.active=true;}else stored.put(key(s),s.asConstructed());}
 public static boolean active(Scenery s){if(s==expiry&&b(input,"failChecks"))throw new Failure();return s.active;}
 public static void ground(GroundItem i){Support.emit("ground",i==null?null:i.id);if(b(input,"groundFails"))throw new Failure();if(b(input,"groundHandles"))new core.game.interaction.InteractPlugin(scenery).handle(player,option);}
 public static class Handler extends RuntimeSupport.Handler{
  public Handler(int t){super(t);}public boolean handle(Player p,Node n,String o){boolean answer=super.handle(p,n,o);if(++handleCalls==1&&b(input,"sceneOnHandle"))replace(v(input,"sceneDelay",1));return answer;}
 }
 static void replace(int delay){Scenery current=stored.get(key(closed));Scenery target=new Scenery(10327,closed.location,0,2);SceneryBuilder.replace(current,target,delay);held.add(stored.get(key(closed)));}
 static void act(Map a){switch((String)a.get("op")){
  case"sceneReplace":replace(v(a,"ticks",1));break;
  case"sceneClient":SceneryBuilder.replace(closed,new Scenery(10327,closed.location,0,2),v(a,"ticks",1),false);break;
  case"sceneExpire":expiry=new Scenery(10327,Location.create(3252,3250,0),10,2);held.add(expiry);held.add(SceneryBuilder.add(expiry,v(a,"ticks",1),new GroundItem(42)));break;
  case"sceneRespawn":SceneryBuilder.remove(closed,v(a,"ticks",1));break;
  case"sceneStop":{Pulse p=stored.get(key(closed)).getRestorePulse();if(p!=null)p.stop();break;}
  case"sceneClose":SceneryBuilder.replace(stored.get(key(closed)),closed);break;
  case"sceneInactive":stored.get(key(closed)).active=false;break;
  case"sceneCheckActive":expiry.active=b(a,"active");break;
  case"ticks":for(int i=0;i<v(a,"count",1);i++)RuntimeSupport.act(Collections.singletonMap("op","tick"));break;
  default:RuntimeSupport.act(a);
 }}
 static Object object(Scenery s){if(s==null)return null;Scenery previous=s instanceof Constructed?((Constructed)s).getReplaced():null;return Arrays.asList(s.id,s.type,s.rotation,coord(s.location),s.active,s instanceof Constructed,previous==null?null:previous.id);}
 static Object worldState(String error)throws Exception{
  Map result=(Map)RuntimeSupport.state(error);List<Object>objects=new ArrayList<>();for(Scenery s:stored.values())objects.add(object(s));result.put("worldObjects",objects);
  List<Object>references=new ArrayList<>();for(Scenery s:held)references.add(object(s));result.put("worldHeld",references);
  List<Object>updates=new ArrayList<>();for(ObjectUpdateFlag f:flags)updates.add(Arrays.asList(f.object.id,f.remove,coord(f.object.location),f.object.type,f.object.rotation));result.put("worldUpdates",updates);flags.clear();
  List<Object>queue=new ArrayList<>();Field ticks=Pulse.class.getDeclaredField("ticksPassed");ticks.setAccessible(true);for(Pulse p:GameWorld.getPulser().getCurrentPulses())queue.add(Arrays.asList(p.getClass().getName(),p.getDelay(),ticks.get(p),p.isRunning()));result.put("worldQueue",queue);return result;
 }
 public static void main(String[]args)throws Exception{
  Locale.setDefault(Locale.ROOT);System.setErr(new PrintStream(new ByteArrayOutputStream()){public void println(Object o){if(o instanceof NullPointerException)Support.emit("exception","Null");}});
  try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){
   Map c=(Map)new JSONParser().parse(line);Map original=new LinkedHashMap(c);RuntimeSupport.setup(c);stored=new TreeMap<>();held=new ArrayList<>();flags=new ArrayList<>();handleCalls=0;expiry=null;closed=new Scenery(12348,Location.create(3250,3250,0),0,1);stored.put(key(closed),closed);held.add(closed);
   List<Object>states=new ArrayList<>();for(Object command:(List)c.get("actions")){events.clear();String error=null;try{act((Map)command);}catch(Failure e){error="Binding";}states.add(worldState(error));}original.put("expected",states);System.out.println(JSONValue.toJSONString(original));
  }}
 }
}
