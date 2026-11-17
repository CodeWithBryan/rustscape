package oracle;
import java.io.*;import java.lang.reflect.*;import java.util.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.*;import core.game.node.entity.player.Player;import core.game.node.scenery.Scenery;
import core.game.world.map.Location;import core.game.interaction.*;import core.game.system.task.Pulse;import core.game.node.entity.impl.PulseType;

public class Support {
 static Map input;static Player player;static Node target;static InteractPlugin plugin;static Option option;
 static List<Option>options;static List<Pulse>pulses;static IdentityHashMap<Option,Integer>optionIds;
 public static Manager[]managers;static Handler[]handlers;
 static List<Object>events;static Map<String,Integer>calls;
 public static class Failure extends RuntimeException{Failure(){super("failure");}public void printStackTrace(){emit("exception","Binding");}public String getMessage(){emit("errorMessage");return input.containsKey("errorMessage")?(String)input.get("errorMessage"):"failure";}}
 public static class Derived extends InteractPlugin{Derived(Node n){super(n);}}
 public static class Handler extends OptionHandler{
  int id;boolean walk=true,walkFor,delayed=true,handled=true;Handler(int n){id=n;}
  public boolean isWalk(){emit("walk",id);return walk;}
  public boolean isWalk(Player p,Node n){emit("walkFor",id,p==null?null:p.key,n==null?null:n.key);return walkFor;}
  public boolean isDelayed(Player p){emit("delayed",id,p==null?null:p.key);return delayed;}
  public boolean handle(Player p,Node n,String o){emit("handle",id,p==null?null:p.key,n==null?null:n.key,o);return handled;}
  public core.plugin.Plugin<Object>newInstance(Object a){return this;}
 }
 public static class Other extends Handler{Other(int n){super(n);}}
 public static class Manager{
  int id;Integer current;Manager(int n){id=n;}
  public void run(Pulse p,PulseType t){current=p.id;emit("run",id,p.id);}
  public void clear(PulseType t){current=null;emit("clear",id);}
  public void runUnhandledAction(Player p,PulseType t){emit("unhandled",id,p==null?null:p.key);}
 }
 static int n(Object v){return((Number)v).intValue();}
 static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 public static boolean answer(String k){return Boolean.TRUE.equals(input.get(k));}
 static Location loc(Object o){if(o==null)return null;List a=(List)o;return Location.create(n(a.get(0)),n(a.get(1)),n(a.get(2)));}
 public static Object coord(Location l){return l==null?null:Arrays.asList(l.getX(),l.getY(),l.getZ());}
 public static Object optionKey(Option o){return o==null?null:optionIds.get(o);}
 public static void emit(String name,Object...args){List<Object>row=new ArrayList<>();row.add(name);row.addAll(Arrays.asList(args));events.add(row);int count=calls.merge(name,1,Integer::sum);
  for(Object h:(List)input.getOrDefault("hooks",Collections.emptyList())){Map hook=(Map)h;if(name.equals(hook.get("event"))&&count==v(hook,"at",1))for(Object a:(List)hook.get("actions"))act((Map)a);}}
 static Object field(Object o,String name){try{Class<?>c=o.getClass();while(c!=null){try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(o);}catch(NoSuchFieldException e){c=c.getSuperclass();}}throw new IllegalArgumentException(name);}catch(ReflectiveOperationException e){throw new RuntimeException(e);}}
 public static void construct(Pulse p){p.id=pulses.size();pulses.add(p);emit("construct",p.id,p.getClass().getName(),p.delay,checks(p));}
 static Object checks(Pulse p){if(p.checks==null)return null;List<Object>a=new ArrayList<>();for(Node n:p.checks)a.add(n==null?null:n.key);return a;}
 static Handler handler(Map m){return m.containsKey("handler")&&m.get("handler")==null?null:handlers[v(m,"handler",1)-1];}
 static void replace(Map m){option=new Option((String)m.getOrDefault("name","Open"),v(m,"index",0)).setHandler(handler(m));optionIds.put(option,options.size());options.add(option);}
 static void set(Map m){for(String k:new String[]{"locked","moving","zone","errorMessage"})if(m.containsKey(k))input.put(k,m.get(k));
  if(m.containsKey("manager"))player.manager=m.get("manager")==null?null:n(m.get("manager"));
  if(m.containsKey("properties"))player.properties=m.get("properties")==null?null:n(m.get("properties"));
  if(m.containsKey("handler"))option.setHandler(handler(m));
  if(m.containsKey("location"))nodes[1].location=loc(m.get("location"));
  if(m.containsKey("playerLocation"))player.location=loc(m.get("playerLocation"));
  if(m.containsKey("size"))nodes[1].size=v(m,"size",1);
  Map hs=(Map)m.getOrDefault("handlers",Collections.emptyMap());for(Object key:hs.keySet()){Handler h=handlers[Integer.parseInt((String)key)-1];Map values=(Map)hs.get(key);for(String f:new String[]{"walk","walkFor","delayed","handled"})if(values.containsKey(f))try{Handler.class.getDeclaredField(f).setBoolean(h,Boolean.TRUE.equals(values.get(f)));}catch(ReflectiveOperationException e){throw new RuntimeException(e);}}
 }
 static Node[]nodes;
 static Player acting(){return input.containsKey("player")&&input.get("player")==null?null:player;}
 static Object act(Map a){Option selected=Boolean.TRUE.equals(a.get("nullOption"))?null:option;switch((String)a.get("op")){
  case"fail":throw new Failure();case"set":set(a);return null;case"replace":replace(a);return null;
  case"handle":plugin.handle(acting(),selected);return null;
  case"invalid":InteractPlugin.handleInvalidInteraction(acting(),target,selected);return null;
  case"walk":case"default":try{Method m=InteractPlugin.class.getDeclaredMethod(a.get("op").equals("walk")?"handleWalkOption":"handleDefaultOption",Player.class,Option.class,PulseType.class);m.setAccessible(true);m.invoke(plugin,acting(),selected,PulseType.STANDARD);return null;}catch(InvocationTargetException e){throw(RuntimeException)e.getCause();}catch(ReflectiveOperationException e){throw new RuntimeException(e);}
  case"arrival":return pulses.isEmpty()?null:pulses.get(v(a,"index",pulses.size()-1)).pulse();
  case"face":return coord(target.getFaceLocation(player.location));
  case"checksClear":if(!pulses.isEmpty())pulses.get(pulses.size()-1).checks=null;return null;
  default:throw new IllegalArgumentException();}}
 static Object snapshot(Object result,String error){Map<String,Object>m=new TreeMap<>();m.put("events",new ArrayList<>(events));m.put("result",result);m.put("error",error);
  m.put("manager",player.manager);m.put("properties",player.properties);m.put("currentManagers",Arrays.asList(managers[0].current,managers[1].current));
  m.put("location",coord(nodes[1].location));m.put("playerLocation",coord(player.location));m.put("size",nodes[1].size);m.put("locked",answer("locked"));m.put("moving",answer("moving"));m.put("zone",answer("zone"));
  List<Object>hs=new ArrayList<>();for(Handler h:handlers)hs.add(Arrays.asList(h.walk,h.walkFor,h.delayed,h.handled));m.put("handlers",hs);
  List<Object>os=new ArrayList<>();for(Option o:options)os.add(Arrays.asList(o.getName(),o.getIndex(),o.getHandler()==null?null:((Handler)o.getHandler()).id));m.put("options",os);m.put("option",optionKey(option));
  List<Object>ps=new ArrayList<>();for(Pulse p:pulses){Option capture=(Option)field(p,"val$option");Player capturePlayer=(Player)field(p,"val$player");Node captureTarget=p.getClass().getName().endsWith("$2")?(Node)field(p,"val$node"):(Node)field(field(p,"this$0"),"node");Integer routing=p instanceof MovementPulse&&((MovementPulse)p).routing!=null?((Handler)((MovementPulse)p).routing).id:null;
   ps.add(Arrays.asList(p.getClass().getName(),p.delay,checks(p),capturePlayer==null?null:capturePlayer.key,captureTarget==null?null:captureTarget.key,optionKey(capture),routing));}m.put("pulses",ps);return m;}
 public static void main(String[]args)throws Exception{
  Locale.setDefault(Locale.ROOT);System.setErr(new PrintStream(new OutputStream(){StringBuilder line=new StringBuilder();public void write(int b){if(b==10){String s=line.toString();line.setLength(0);if(s.startsWith("java.lang.NullPointerException"))emit("exception","Null");}else if(b!=13)line.append((char)b);}}));
  try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){
   input=(Map)new JSONParser().parse(line);Map initial=new TreeMap(input);events=new ArrayList<>();calls=new HashMap<>();pulses=new ArrayList<>();options=new ArrayList<>();optionIds=new IdentityHashMap<>();
   managers=new Manager[]{new Manager(0),new Manager(1)};handlers=new Handler[]{new Handler(1),new Other(2)};
   player=new Player();player.key=1;player.location=Location.create(3207,3216,0);Node object=new Scenery();object.key=2;object.location=Location.create(3211,3217,0);nodes=new Node[]{player,object};
   target=input.containsKey("target")&&input.get("target")==null?null:object;plugin=Boolean.TRUE.equals(input.get("derived"))?new Derived(target):new InteractPlugin(target);replace(input);set(input);
   List<Object>states=new ArrayList<>();for(Object action:(List)input.get("actions")){events.clear();Object result=null;String error=null;try{result=act((Map)action);}catch(Failure e){error="Binding";}catch(NullPointerException e){error="Null";}states.add(snapshot(result,error));}
   initial.put("expected",states);System.out.println(JSONValue.toJSONString(initial));
  }}
 }
}
