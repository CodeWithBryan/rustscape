package oracle;
import java.lang.System;
import java.io.*;import java.util.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.*;import core.game.node.entity.*;import core.game.node.entity.player.*;import core.game.node.entity.npc.*;import core.game.node.scenery.*;import core.game.world.*;import core.game.world.map.*;import core.game.world.map.path.*;import core.game.interaction.*;import core.net.packet.in.Packet;
public class Support{
 static Map input;static Player player;static List<Object>events;static Map<String,Integer>calls;public static boolean locked;
 static Scenery scenery;static Option option;
 public static class Failure extends RuntimeException{public void printStackTrace(){emit("exception","Binding");}}
 static int n(Object o){return((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}static boolean b(Map m,String k){return Boolean.TRUE.equals(m.get(k));}
 public static String bits(double d){return Long.toHexString(Double.doubleToLongBits(d));}
 static Object coord(Location l){return l==null?null:Arrays.asList(l.getX(),l.getY(),l.getZ());}
 // The recording approach lives in this harness. Publish the original caller's
 // class identity in telemetry; its arrival remains an explicit service adapter.
 public static void emit(String phase,Object...args){for(int i=0;i<args.length;i++)if("oracle.Support$1".equals(args[i]))args[i]="core.game.interaction.InteractPlugin$3";List<Object>row=new ArrayList<>();row.add(phase);row.addAll(Arrays.asList(args));events.add(row);int count=calls.merge(phase,1,Integer::sum);for(Object o:(List)input.getOrDefault("hooks",Collections.emptyList())){Map h=(Map)o;if(phase.equals(h.get("event"))&&count==v(h,"at",1))for(Object a:(List)h.get("actions"))act((Map)a);}}
 public static class Region{public boolean isActive(){return true;}}
 public static Region region(Location l){emit("region",coord(l));return new Region();}
 public static int clip(int z,int x,int y){emit("clip",z,x,y);int f=0;for(Object o:(List)input.getOrDefault("flags",Collections.emptyList())){List a=(List)o;if(n(a.get(0))==x&&n(a.get(1))==y)f=n(a.get(2));}return f;}
 public static boolean timer(String s){emit("timer",s);return b(input,s);}public static boolean marathon(){emit("marathon");return false;}
 public static boolean zone(Location a,Location b){emit("zone",coord(a),coord(b));List answers=(List)input.getOrDefault("zoneAnswers",Collections.emptyList());int i=calls.get("zone")-1;return i>=answers.size()||Boolean.TRUE.equals(answers.get(i));}
 public static void move(Location a,Location b){emit("move",coord(a),coord(b));}
 static Location loc(Object value){if(value==null)return null;List a=(List)value;return Location.create(n(a.get(0)),n(a.get(1)),n(a.get(2)));}
 public static boolean permitted(Location l){emit("permitted",coord(l));return true;}
 public static Location door(Entity m,Scenery s){emit("doorDestination",m==null?null:m.name,s==null?null:s.name);return input.containsKey("doorDestination")?loc(input.get("doorDestination")):s.location.transform(1,0,0);}
 public static Pathfinder npcFinder(NPC n){throw new UnsupportedOperationException();}
 public static Location destination(String s,int t,Entity m,Node d){emit("destination",s,t,m==null?null:m.name,d==null?null:d.name);return loc(input.get("optionDestination"));}
 static void scenerySet(Map a){if(a.containsKey("location"))scenery.location=loc(a.get("location"));if(a.containsKey("active"))scenery.active=b(a,"active");if(a.containsKey("shape"))scenery.type=v(a,"shape",10);if(a.containsKey("rotation"))scenery.rotation=v(a,"rotation",0);if(a.containsKey("walking"))scenery.definition.walkingFlag=v(a,"walking",0);}
 static OptionHandler handler(Map a){return a.containsKey("handler")&&a.get("handler")==null?null:new OptionHandler(v(a,"handler",1));}
 static Object field(Object o,String name){try{Class c=o.getClass();while(c!=null){try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(o);}catch(NoSuchFieldException e){c=c.getSuperclass();}}throw new IllegalArgumentException(name);}catch(ReflectiveOperationException e){throw new RuntimeException(e);}}
 static void field(Object o,String name,Object value){try{Class c=o.getClass();while(c!=null){try{Field f=c.getDeclaredField(name);f.setAccessible(true);f.set(o,value);return;}catch(NoSuchFieldException e){c=c.getSuperclass();}}throw new IllegalArgumentException(name);}catch(ReflectiveOperationException e){throw new RuntimeException(e);}}
 static void act(Map a){switch((String)a.get("op")){case"fail":throw new Failure();case"request":{
  int x=v(a,"x",3210),y=v(a,"y",3216);boolean run=b(a,"running");Packet p;switch((String)a.getOrDefault("kind","world")){case"interact":{Packet.InteractWalk q=new Packet.InteractWalk();q.player=player;q.destX=x;q.destY=y;q.isRun=run;p=q;break;}case"minimap":{Packet.MinimapWalk q=new Packet.MinimapWalk();q.player=player;q.destX=x;q.destY=y;q.isRun=run;p=q;break;}default:{Packet.WorldspaceWalk q=new Packet.WorldspaceWalk();q.player=player;q.destX=x;q.destY=y;q.isRun=run;p=q;}}
  core.net.packet.PacketProcessor.handle(p);break;}
 case"approach":{final Scenery target=scenery;final Option selected=option;MovementPulse p=new MovementPulse(player,target,selected.getHandler()){
 public boolean pulse(){emit("interactionArrival",player.name,target.name,selected.getName(),selected.getIndex(),selected.getHandler()==null?null:field(selected.getHandler(),"token"));return true;}};player.getPulseManager().run(p);break;}
 case"scenerySet":scenerySet(a);break;
 case"optionReplace":option=new Option((String)a.getOrDefault("name","Open"),v(a,"index",0)).setHandler(handler(a));break;
 case"optionHandler":option.setHandler(handler(a));break;
 case"tick":GameWorld.getPulser().updateAll();player.tick();GameWorld.setTicks(GameWorld.getTicks()+1);break;
 case"mutatePulse":field(player.currentMovement,"mover",null);field(player.currentMovement,"forceRun",false);field(player.currentMovement,"checks",null);break;
 case"arrival":player.currentMovement.pulse();break;
 default:throw new IllegalArgumentException();}}
 static Object snapshot(String error){Map<String,Object>o=new TreeMap<>();o.put("error",error);o.put("events",new ArrayList<>(events));o.put("ticks",GameWorld.getTicks());o.put("location",coord(player.location));o.put("foot",coord(player.walking.getFootPrint()));o.put("direction",player.direction.toInteger());o.put("running",player.walking.isRunning());o.put("walk",player.walking.getWalkDir());o.put("run",player.walking.getRunDir());o.put("energy",bits(player.settings.energy));o.put("toggled",player.settings.toggled);List<Object>points=new ArrayList<>();for(Point p:player.walking.getQueue())points.add(Arrays.asList(p.getX(),p.getY()));o.put("points",points);MovementPulse p=player.currentMovement;if(p==null)o.put("current",null);else{Map<String,Object>m=new TreeMap<>();m.put("running",p.isRunning());m.put("last",coord((Location)field(p,"last")));m.put("interact",coord((Location)field(p,"interactLocation")));m.put("near",field(p,"near"));m.put("force",field(p,"forceRun"));o.put("current",m);}core.game.system.task.Pulse managed=player.getPulseManager().getCurrent();o.put("managed",managed==null?null:managed.isRunning());List<Boolean>pending=new ArrayList<>();for(core.game.system.task.Pulse q:GameWorld.getPulser().getCurrentPulses())pending.add(q.isRunning());o.put("pending",pending);
 if(input.containsKey("scenery")){o.put("scenery",Arrays.asList(coord(scenery.location),scenery.type,scenery.rotation,scenery.active,scenery.size,scenery.definition.sizeX,scenery.definition.sizeY,scenery.definition.walkingFlag));o.put("option",Arrays.asList(option.getName(),option.getIndex(),option.getHandler()==null?null:field(option.getHandler(),"token")));}
 return o;}
 public static void main(String[]args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){input=(Map)new JSONParser().parse(line);events=new ArrayList<>();calls=new HashMap<>();GameWorld.setPulser(new PulseRunner());GameWorld.setTicks(0);GameWorld.INSTANCE.getSettings().setSmartpathfinder_bfs(b(input,"bfs"));locked=b(input,"locked");player=new Player("1",Location.create(3207,3216,0));scenery=null;option=null;
 if(input.containsKey("scenery")){Map m=(Map)input.get("scenery");scenery=new Scenery();scenery.name="object";scenery.location=Location.create(3211,3217,0);scenerySet(m);List size=(List)m.getOrDefault("size",Arrays.asList(1L,1L));scenery.definition.sizeX=n(size.get(0));scenery.definition.sizeY=n(size.get(1));scenery.size=scenery.definition.sizeX;scenery.direction=Direction.get(scenery.rotation);option=new Option("Open",0).setHandler(handler(input));}
player.settings.energy=input.containsKey("energy")?((Number)input.get("energy")).doubleValue():100.0;List<Object>states=new ArrayList<>();for(Object a:(List)input.get("actions")){events.clear();String error=null;try{act((Map)a);}catch(Failure e){error="Binding";}states.add(snapshot(error));}input.put("expected",states);System.out.println(JSONValue.toJSONString(input));}}}
}
