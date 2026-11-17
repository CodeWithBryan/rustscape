package oracle;
import java.io.*;import java.util.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.Node;import core.game.node.entity.player.Player;import core.game.node.entity.npc.NPC;
import core.game.node.scenery.Scenery;import core.game.node.item.Item;import core.game.interaction.*;
import core.game.system.task.Pulse;import core.game.node.entity.impl.PulseType;import core.game.world.map.Location;
public class Support{
 static Map input;static Node[]nodes;static InteractPlugin[]tables;public static Definition[]definitions;
 public static Integer[]definitionIds;static List<Option>options;static List<Option[]>arrays;static List<String[]>names;
 static List<Object>events;static Map<String,Integer>calls;public static Manager[]managers;
 public static class Failure extends RuntimeException{}
 public static class Definition{
  public int key;public String[]value,ground,inventory;
  public String[]getOptions(){emit("names",key,"Options");return value;}
  public String[]getGroundOptions(){emit("names",key,"Ground");return ground;}
  public String[]getInventoryOptions(){emit("names",key,"Inventory");return inventory;}
  public int[]getChildrenIds(){return null;}
 }
 public static class Handler extends OptionHandler{
  int key;Handler(int k){key=k;}public boolean handle(Player p,Node n,String s){throw new UnsupportedOperationException();}
  public core.plugin.Plugin<Object>newInstance(Object arg){return this;}
 }
 static Handler[]handlers;
 public static OptionHandler lookup(String kind,int id,String name){OptionHandler h=handlers[0];emit("lookup",kind,id,name);return h;}
 public static class Manager{
  public void run(Pulse p,PulseType t){throw new UnsupportedOperationException();}
  public void clear(PulseType t){throw new UnsupportedOperationException();}
  public void runUnhandledAction(Player p,PulseType t){throw new UnsupportedOperationException();}
 }
 public static void construct(Pulse p){throw new UnsupportedOperationException();}
 public static boolean answer(String k){return false;}
 public static Object coord(Location l){return l==null?null:Arrays.asList(l.getX(),l.getY(),l.getZ());}
 public static Object optionKey(Option o){if(o==null)return null;for(int i=0;i<options.size();i++)if(options.get(i)==o)return i;options.add(o);return options.size()-1;}
 static int n(Object o){return((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 static Option selected(Map m){return m.containsKey("option")&&m.get("option")==null?null:options.get(v(m,"option",0));}
 public static void emit(String name,Object...args){List<Object>r=new ArrayList<>();r.add(name);r.addAll(Arrays.asList(args));events.add(r);int count=calls.merge(name,1,Integer::sum);
  for(Object o:(List)input.getOrDefault("hooks",Collections.emptyList())){Map h=(Map)o;if(name.equals(h.get("event"))&&count==v(h,"at",1))for(Object a:(List)h.get("actions"))act((Map)a);}}
 static String[]newNames(Object value){if(value==null)return null;List list=(List)value;String[]array=new String[list.size()];for(int i=0;i<array.length;i++)array[i]=(String)list.get(i);names.add(array);return array;}
 static Object act(Map a){InteractPlugin t=tables[v(a,"table",0)];switch((String)a.get("op")){
  case"fail":throw new Failure();
  case"new":Option o=new Option((String)a.get("name"),v(a,"index",0));optionKey(o);return null;
  case"set":t.set(selected(a));return null;
  case"get":return optionKey(t.get(v(a,"index",0)));
  case"remove":return t.remove(selected(a));
  case"removeIndex":t.remove(v(a,"index",0));return null;
  case"default":t.setDefault();return null;
  case"init":t.init(v(a,"id",995),newNames(a.get("names")));return null;
  case"handler":OptionHandler h=Option.defaultHandler(nodes[v(a,"table",0)],v(a,"id",995),(String)a.get("name"));return h==null?null:((Handler)h).key;
  case"handlerSet":selected(a).setHandler(a.get("handler")==null?null:handlers[n(a.get("handler"))-1]);return null;
  case"setInitialized":t.setInitialized(Boolean.TRUE.equals(a.get("value")));return null;
  case"capture":arrays.add(t.getOptions());return arrays.size()-1;
  case"arraySet":arrays.get(v(a,"array",0))[v(a,"index",0)]=selected(a);return null;
  case"namesSet":names.get(v(a,"array",0))[v(a,"index",0)]=(String)a.get("value");return null;
  case"setNode":Node node=nodes[v(a,"table",0)];if(a.containsKey("id"))node.id=n(a.get("id"));if(a.containsKey("located"))node.location=Boolean.TRUE.equals(a.get("located"))?Location.create(3207,3216,0):null;
   if(a.containsKey("definition"))definitionIds[node.key]=a.get("definition")==null?null:n(a.get("definition"));return null;
  default:throw new IllegalArgumentException((String)a.get("op"));}}
 static Object array(Option[]a){List<Object>r=new ArrayList<>();for(Option o:a)r.add(optionKey(o));return r;}
 static Object snapshot(Object result,String error){Map<String,Object>r=new TreeMap<>();List<Object>ts=new ArrayList<>(),as=new ArrayList<>(),ns=new ArrayList<>(),os=new ArrayList<>(),nds=new ArrayList<>();
  for(InteractPlugin t:tables)ts.add(Arrays.asList(t.isInitialized(),array(t.getOptions())));
  for(Option[]a:arrays)as.add(array(a));for(String[]a:names)ns.add(new ArrayList<>(Arrays.asList(a)));
  for(Option o:options)os.add(Arrays.asList(o.getName(),o.getIndex(),o.getHandler()==null?null:((Handler)o.getHandler()).key));
  for(Node n:nodes)nds.add(n==null?null:Arrays.asList(n.id,n.location!=null,definitionIds[n.key]));
  r.put("tables",ts);r.put("arrays",as);r.put("names",ns);r.put("options",os);r.put("nodes",nds);r.put("events",new ArrayList<>(events));r.put("result",result);r.put("error",error);return r;}
 static Node node(String kind){switch(kind){case"Player":return new Player();case"Npc":return new NPC();case"Scenery":return new Scenery();case"Item":return new Item();case"Null":return null;default:return new Node();}}
 static Definition definition(String kind){switch(kind){case"Npc":return new core.cache.def.impl.NPCDefinition();case"Scenery":return new core.cache.def.impl.SceneryDefinition();default:return new core.cache.def.impl.ItemDefinition();}}
 public static void main(String[]args)throws Exception{Locale.setDefault(Locale.ROOT);
 try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){input=(Map)new JSONParser().parse(line);Map initial=new TreeMap(input);
  events=new ArrayList<>();calls=new HashMap<>();options=new ArrayList<>();arrays=new ArrayList<>();names=new ArrayList<>();handlers=new Handler[]{new Handler(1),new Handler(2)};
  for(Option o:new Option[]{Option._P_ATTACK,Option._P_FOLLOW,Option._P_TRADE,Option._P_GIVETO,Option._P_PICKPOCKET,Option._P_EXAMINE,Option._P_ASSIST,Option.NULL}){o.setHandler(null);options.add(o);}
  String kind=(String)input.getOrDefault("kind","Item");nodes=new Node[]{node(kind),node(kind)};tables=new InteractPlugin[2];definitions=new Definition[2];definitionIds=new Integer[]{0,1};
  String[]base=newNames(input.getOrDefault("names",Arrays.asList("Take","Use",null,"null","Examine")));
  for(int i=0;i<2;i++){definitions[i]=definition(kind);definitions[i].key=i;definitions[i].value=base;definitions[i].ground=base;definitions[i].inventory=base;
   Node n=nodes[i];if(n!=null){n.key=i;n.id=995;n.location=Boolean.FALSE.equals(input.get("located"))?null:Location.create(3207,3216,0);}tables[i]=new InteractPlugin(n);}
  if(input.containsKey("definition")&&input.get("definition")==null)definitionIds[0]=null;
  List<Object>states=new ArrayList<>();for(Object action:(List)input.get("actions")){events.clear();Object result=null;String error=null;
   try{result=act((Map)action);}catch(Failure e){error="Binding";}catch(NullPointerException e){error="Null";}catch(IndexOutOfBoundsException e){error="Index";}catch(IllegalStateException e){error="Unsupported:"+e.getMessage();}
   states.add(snapshot(result,error));}initial.put("expected",states);System.out.println(JSONValue.toJSONString(initial));
 }}}
}
