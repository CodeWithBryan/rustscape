package oracle;
import java.io.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.cache.def.impl.ItemDefinition;import core.game.node.item.Item;
import core.game.interaction.*;import core.game.node.entity.player.Player;import core.game.node.Node;
import core.game.world.map.Location;import core.game.system.task.Pulse;import core.game.node.entity.impl.PulseType;
public class Support{
 public static class Definition{public String[]groundActions,options;public String[]getOptions(){return options;}public int[]getChildrenIds(){return null;}}
 public static class Manager{public void run(Pulse p,PulseType t){throw new UnsupportedOperationException();}public void clear(PulseType t){throw new UnsupportedOperationException();}public void runUnhandledAction(Player p,PulseType t){throw new UnsupportedOperationException();}}
 public static Manager[]managers=new Manager[0];public static void construct(Pulse p){throw new UnsupportedOperationException();}
 public static void emit(String s,Object...args){}public static boolean answer(String s){return false;}public static Object optionKey(Option o){return null;}
 public static Object coord(Location l){return l==null?null:Arrays.asList(l.getX(),l.getY(),l.getZ());}
 static class Other extends OptionHandler{public boolean handle(Player p,Node n,String s){throw new UnsupportedOperationException();}public core.plugin.Plugin<Object>newInstance(Object a){return this;}}
 static Item node;static ItemDefinition[]defs;static OptionHandler[]handlers;static InteractPlugin table;static List<Option[]>captures;
 static int n(Object o){return((Number)o).intValue();}
 static String[]names(Object o){if(o==null)return null;List l=(List)o;String[]a=new String[l.size()];for(int i=0;i<a.length;i++)a[i]=(String)l.get(i);return a;}
 static OptionHandler handler(Object o){return o==null?null:handlers[n(o)];}
 static void config(Object value){defs[0].handlers.put("option:take",value instanceof Number?handler(value):value);}
 static void act(Map a){Object value=a.get("value");switch((String)a.get("op")){
 case"default":table.setDefault();break;case"capture":captures.add(table.getOptions());break;
 case"register":ItemDefinition.setOptionHandler((String)a.get("name"),handler(value));break;
 case"initialized":table.setInitialized(Boolean.TRUE.equals(value));break;
 case"charge":node.setCharge(n(value));break;case"cached":node.definition=value==null?null:defs[n(value)];break;
 case"located":node.location=Boolean.TRUE.equals(value)?Location.create(3207,3216,0):null;break;
 case"names":defs[0].groundActions=names(value);break;case"config":config(value);break;
 default:throw new IllegalArgumentException();}}
 static Object array(Option[]a){List<Object>r=new ArrayList<>();for(Option o:a){Integer h=null;if(o!=null&&o.getHandler()!=null){h=-1;for(int i=0;i<handlers.length;i++)if(o.getHandler()==handlers[i])h=i;}r.add(o==null?null:Arrays.asList(o.getName(),o.getIndex(),h));}return r;}
 static Object snapshot(String error){Map<String,Object>r=new TreeMap<>();List<Object>arrays=new ArrayList<>(),same=new ArrayList<>();for(Option[]a:captures){arrays.add(array(a));same.add(a==table.getOptions());}
 r.put("current",array(table.getOptions()));r.put("initialized",table.isInitialized());r.put("captures",arrays);r.put("same",same);r.put("id",node.getId());r.put("cached",node.definition==null?null:node.definition==defs[0]?0:1);r.put("error",error);return r;}
 public static void main(String[]args)throws Throwable{Locale.setDefault(Locale.ROOT);try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){Map in=(Map)new JSONParser().parse(line),out=new TreeMap(in);captures=new ArrayList<>();
 ItemDefinition.DEFINITIONS.clear();ItemDefinition.getOptionHandlers().clear();defs=new ItemDefinition[]{new ItemDefinition(),new ItemDefinition()};
 defs[0].groundActions=names(in.containsKey("names")?in.get("names"):Arrays.asList(null,null,"Take",null,null));defs[0].options=new String[]{"Use",null,null,null,"Drop"};defs[1].groundActions=new String[]{"Other",null,"Take"};defs[1].options=new String[]{"Other"};ItemDefinition.DEFINITIONS.put(995,defs[0]);
 handlers=new OptionHandler[]{new content.global.handlers.item.PickupPlugin(),new Other()};handlers[0].newInstance(null);
 if(in.containsKey("globalHandler"))ItemDefinition.setOptionHandler("take",handler(in.get("globalHandler")));if(in.containsKey("local"))config(in.get("local"));
 node=new Item();node.idHash=(995<<16)|1000;node.definition=defs[0];node.location=Boolean.FALSE.equals(in.get("located"))?null:Location.create(3207,3216,0);table=new InteractPlugin(node);
 List<Object>states=new ArrayList<>();for(Object a:(List)in.get("actions")){String error=null;try{act((Map)a);}catch(NullPointerException e){error="Null";}catch(ClassCastException e){error="Type";}catch(IndexOutOfBoundsException e){error="Index";}states.add(snapshot(error));}out.put("expected",states);System.out.println(JSONValue.toJSONString(out));
 }}}
}
