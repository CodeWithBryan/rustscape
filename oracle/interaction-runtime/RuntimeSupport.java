package oracle;
import java.lang.System;
import java.io.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.*;import core.game.node.entity.*;import core.game.node.entity.player.*;import core.game.node.scenery.*;import core.game.world.*;import core.game.world.map.*;import core.game.interaction.*;import core.game.node.entity.impl.PulseManager;import core.game.system.task.Pulse;
public class RuntimeSupport extends Support {
 static Location face;
 public static void emit(String phase,Object...args){
  Support.emit(phase,args);int count=calls.get(phase);
  for(Object o:(List)input.getOrDefault("interactionHooks",Collections.emptyList())){
   Map h=(Map)o;if(phase.equals(h.get("event"))&&count==v(h,"at",1))for(Object a:(List)h.get("actions"))act((Map)a);
  }
 }
 public static boolean interactionLocked(){emit("interactionLocked");return b(input,"interactionLocked");}
 public static boolean zone(Node target,Option o){emit("interactionZone",target==null?null:target.name,o==null?null:o.getName(),o==null?null:o.getIndex(),o==null||o.getHandler()==null?null:((Handler)o.getHandler()).token);return b(input,"interactionZone");}
 public static void face(Location l){face=l;emit("facePlayer",coord(l));}
 public static class Handler extends OptionHandler {
  final int token;public Handler(int t){token=t;}
  Map config(){return token==1?(Map)input.getOrDefault("handlerConfig",Collections.emptyMap()):(Map)((Map)input.getOrDefault("handlers",Collections.emptyMap())).getOrDefault(""+token,Collections.emptyMap());}
  boolean flag(String key,boolean d){Map c=config();return c.containsKey(key)?b(c,key):d;}
  public Handler newInstance(Object ignored){return this;}
  public boolean isWalk(){emit("handlerWalk",token);return flag("walk",true);}
  public boolean isWalk(Player p,Node n){emit("handlerWalkFor",token);return flag("walkFor",false);}
  public boolean isDelayed(Player p){emit("handlerDelayed",token);return flag("delayed",true);}
  public boolean handle(Player p,Node n,String option){emit("handlerHandle",token,p==null?null:p.name,n==null?null:n.name,option);return flag("handled",true);}
  public Location getDestination(Node m,Node d){return Support.destination("option",token,(Entity)m,d);}
 }
 static Object coord(Location l){return l==null?null:Arrays.asList(l.getX(),l.getY(),l.getZ());}
 static void act(Map a){switch((String)a.get("op")){
  case"handle":new InteractPlugin(scenery).handle(player,option);break;
  case"invalid":InteractPlugin.handleInvalidInteraction(player,scenery,option);break;
  case"set":for(Object k:a.keySet())if(!"op".equals(k))input.put(k,a.get(k));break;
  case"managerReplace":player.manager=new PulseManager(player);break;
  case"sceneryReplace":{Scenery old=scenery;scenery=new Scenery();scenery.name="replacement";scenery.location=old.location;scenery.size=old.size;scenery.direction=old.direction;scenery.type=old.type;scenery.rotation=old.rotation;scenery.active=old.active;scenery.definition=old.definition;break;}
  case"checksClear":{Pulse p=player.getPulseManager().getCurrent();if(p!=null)field(p,"checks",null);break;}
  default:Support.act(a);
 }}
 static Object state(String error){
  Map o=(Map)Support.snapshot(error);o.put("faceLocation",coord(face));
  Pulse current=player.getPulseManager().getCurrent();o.put("managedClass",current==null?null:current.getClass().getName());
  List<Object>names=new ArrayList<>();for(Pulse p:GameWorld.getPulser().getCurrentPulses())names.add(p.getClass().getName());o.put("pendingClasses",names);
  o.put("interactionLocked",b(input,"interactionLocked"));o.put("interactionZone",b(input,"interactionZone"));
  return o;
 }
 static void setup(Map c){
    input=c;events=new ArrayList<>();calls=new HashMap<>();GameWorld.setPulser(new PulseRunner());GameWorld.setTicks(0);GameWorld.INSTANCE.getSettings().setSmartpathfinder_bfs(b(input,"bfs"));locked=b(input,"locked");player=new Player("1",Location.create(3207,3216,0));face=null;
    Map m=(Map)input.get("scenery");scenery=new Scenery();scenery.name="object";scenery.location=Location.create(3211,3217,0);scenerySet(m);List size=(List)m.getOrDefault("size",Arrays.asList(1L,1L));scenery.definition.sizeX=n(size.get(0));scenery.definition.sizeY=n(size.get(1));scenery.size=scenery.definition.sizeX;scenery.direction=Direction.get(scenery.rotation);option=new Option("Open",0).setHandler(handler(input));
    player.settings.energy=input.containsKey("energy")?((Number)input.get("energy")).doubleValue():100.0;
 }
 public static void main(String[]args)throws Exception{
  Locale.setDefault(Locale.ROOT);
  System.setErr(new PrintStream(new ByteArrayOutputStream()){
   public void println(Object o){if(o instanceof NullPointerException)Support.emit("exception","Null");}
  });
  try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;
   while((line=r.readLine())!=null){
    Map c=(Map)new JSONParser().parse(line);Map original=new LinkedHashMap(c);setup(c);
    List<Object>states=new ArrayList<>();for(Object a:(List)input.get("actions")){events.clear();String error=null;try{act((Map)a);}catch(Failure e){error="Binding";}states.add(state(error));}
    original.put("expected",states);System.out.println(JSONValue.toJSONString(original));
   }
  }
 }
}
