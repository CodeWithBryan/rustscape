package oracle;
import java.util.*;import java.io.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.cache.def.impl.ItemDefinition;import core.game.container.*;import core.game.container.impl.EquipmentContainer;import core.game.node.entity.player.Player;import core.game.node.item.Item;import core.net.packet.IoBuffer;import core.net.packet.context.ContainerContext;
public class EquipmentOracle{
 public static Map row;public static Player player;static EquipmentContainer equipment;static Map<String,Item> named;static List<Object> trace,frames;static Map<String,Integer> hits;
 static int n(Map m,String k,int d){Object v=m.get(k);return v==null?d:((Number)v).intValue();}
 static Map map(Object...v){Map out=new TreeMap();for(int i=0;i<v.length;i+=2)out.put(v[i],v[i+1]);return out;}
 static Object item(Item i){return i==null?null:map("id",i.getId(),"amount",i.getAmount(),"charge",i.getCharge(),"index",i.getIndex(),"name",i.getName());}
 static List<Object> items(Item[] a){List<Object> out=new ArrayList<>();for(Item i:a)out.add(item(i));return out;}
 public static void hit(String key,Object...args){List<Object> e=new ArrayList<>();e.add(key);for(Object a:args){if(a instanceof int[]){List<Object> x=new ArrayList<>();for(int v:(int[])a)x.add(v);e.add(x);}else e.add(a);}trace.add(e);int at=hits.getOrDefault(key,0)+1;hits.put(key,at);
  for(Object raw:(List)row.get("hooks")){Map h=(Map)raw;if(!key.equals(h.get("when"))||n(h,"at",1)!=at)continue;switch((String)h.get("action")){
   case "fail":throw new IllegalStateException("controlled");
   case "clearInventory":Arrays.fill(player.inventory.toArray(),null);break;
   case "clearEquipment":Arrays.fill(equipment.toArray(),null);break;
   case "amount":named.get(h.get("item")).setAmount(n(h,"value",3));break;
   case "index":named.get(h.get("item")).setIndex(n(h,"value",3));break;
   case "event":try{Field f=Container.class.getDeclaredField("event");f.setAccessible(true);((ContainerEvent)f.get(equipment)).flag(1,named.get("new"));}catch(Exception x){throw new AssertionError(x);}break;
   case "replaceArray":equipment.copy(new Container(6));break;
   case "clearBonuses":Arrays.fill(player.properties.bonuses,0);break;
   case "reenter":equipment.refresh();break;
  }}
 }
 public static boolean requirement(ItemDefinition d,Player p,boolean a,boolean b){hit("requirements",d.getId(),a,b);return Boolean.TRUE.equals(row.get("allowed"));}
 public static Object plugin(String key,Object[] args){hit("plugin",key,item((Item)args[1]));return row.get("plugin");}
 public static boolean interaction(int id,Player p,Item i,boolean e){hit("interaction",id,item(i),e);return Boolean.TRUE.equals(row.get("interaction"));}
 public static void packet(ContainerContext c){hit("packet",c.getSlots());new core.net.packet.out.ContainerPacket().send(c);}
 public static void frame(IoBuffer b){java.nio.ByteBuffer data=b.toByteBuffer();data.flip();StringBuilder h=new StringBuilder();while(data.hasRemaining())h.append(String.format("%02x",data.get()&255));frames.add(map("opcode",b.opcode()&255,"payloadHex",h.toString()));}
 static Item make(Map m)throws Exception{if(m==null)return null;Item i=new Item(n(m,"id",1),n(m,"amount",1),n(m,"charge",1000));i.setIndex(n(m,"index",-1));if(m.containsKey("definition"))i.setDefinition(ItemDefinition.forId(n(m,"definition",0)));if(m.containsKey("name")){Field f=core.game.node.Node.class.getDeclaredField("name");f.setAccessible(true);f.set(i,m.get("name"));}return i;}
 static List<Integer> ints(int[] a){List<Integer> v=new ArrayList<>();for(int i:a)v.add(i);return v;} static Object state(){return map("inventory",items(player.inventory.toArray()),"equipment",items(equipment.toArray()),"ownEquipment",items(player.equipment.toArray()),"speed",player.properties.speed,"bonuses",ints(player.properties.bonuses),"weights",player.weights,"animations",player.animations,"appearanceUpdates",player.appearanceUpdates,"defence",player.defence,"dfs",player.dfs);}
 static Object evaluate(Map input)throws Exception{row=input;trace=new ArrayList<>();frames=new ArrayList<>();hits=new HashMap<>();named=new TreeMap<>();ItemDefinition.getDefinitions().clear();
  for(Object raw:((Map)row.get("definitions")).entrySet()){Map.Entry e=(Map.Entry)raw;int id=Integer.parseInt((String)e.getKey());Map v=(Map)e.getValue();ItemDefinition d=new ItemDefinition();d.setId(id);Field f=core.cache.def.Definition.class.getDeclaredField("name");f.setAccessible(true);f.set(d,"item"+id);f=ItemDefinition.class.getDeclaredField("stackable");f.setAccessible(true);f.setBoolean(d,Boolean.TRUE.equals(v.get("stack")));
   for(Object r:v.entrySet()){Map.Entry z=(Map.Entry)r;String k=(String)z.getKey();Object value=z.getValue();if(k.equals("slot"))k="equipment_slot";else if(k.equals("two"))k="two_handed";else if(k.equals("speed"))k="attack_speed";
    if(k.equals("bonuses")&&value instanceof List){int[] b=new int[((List)value).size()];for(int x=0;x<b.length;x++)b[x]=((Number)((List)value).get(x)).intValue();value=b;}
    else if(k.equals("requirements")&&value instanceof Map){Map<Integer,Integer> b=new HashMap<>();for(Object rr:((Map)value).entrySet()){Map.Entry q=(Map.Entry)rr;b.put(Integer.parseInt((String)q.getKey()),((Number)q.getValue()).intValue());}value=b;}
    else if(k.equals("equipment")&&"plugin".equals(value))value=new core.plugin.Plugin<Object>();
    else if(value instanceof Number)value=((Number)value).intValue();d.getHandlers().put(k,value);
   }ItemDefinition.getDefinitions().put(id,d);
  }
  player=new Player();equipment=Boolean.TRUE.equals(row.get("nullPlayer"))?new EquipmentContainer(null):Boolean.TRUE.equals(row.get("detachedEquipment"))?new EquipmentContainer(player):player.equipment;
  player.dfs=Boolean.TRUE.equals(row.get("dfs"));player.special=Boolean.TRUE.equals(row.get("special"));player.extension=Boolean.TRUE.equals(row.get("extension"));
  for(Object raw:((Map)row.get("items")).entrySet()){Map.Entry e=(Map.Entry)raw;named.put((String)e.getKey(),make((Map)e.getValue()));}
  for(String key:new String[]{"inventory","equipment"}){Container target=key.equals("inventory")?player.inventory:equipment;for(Object raw:((Map)row.get(key)).entrySet()){Map.Entry e=(Map.Entry)raw;target.toArray()[Integer.parseInt((String)e.getKey())]=named.get(e.getValue());}}
  if(row.containsKey("ownEquipment"))for(Object raw:((Map)row.get("ownEquipment")).entrySet()){Map.Entry e=(Map.Entry)raw;player.equipment.toArray()[Integer.parseInt((String)e.getKey())]=named.get(e.getValue());}
  Field f=Container.class.getDeclaredField("event");f.setAccessible(true);ContainerEvent event=(ContainerEvent)f.get(equipment);for(Object raw:((Map)row.get("event")).entrySet()){Map.Entry e=(Map.Entry)raw;event.flag(Integer.parseInt((String)e.getKey()),named.get(e.getValue()));}
  List<Object> results=new ArrayList<>();
  for(Object raw:(List)row.get("actions")){Map a=(Map)raw;Item i=a.get("item")==null?null:named.get(a.get("item"));Object result=null;String error=null;try{switch((String)a.get("op")){
   case "equip":result=equipment.add(i,!Boolean.FALSE.equals(a.get("fire")));break;
   case "explicit":result=equipment.add(i,n(a,"inventorySlot",0),!Boolean.FALSE.equals(a.get("fire")),!Boolean.FALSE.equals(a.get("fromInventory")));break;
   case "one":result=equipment.add(i);break;
   case "three":result=equipment.add(i,!Boolean.FALSE.equals(a.get("fire")),-1);break;
   case "array":result=equipment.add(new Item[]{i});break;
   case "list":equipment.addList(Arrays.asList(i));break;
   case "all":Container other=new Container(1);other.toArray()[0]=i;equipment.addAll(other);break;
   case "absent":result=equipment.addIfDoesntHave(i);break;
   case "shift":equipment.shift();break;
   case "refresh":equipment.refresh();break;
   case "update":equipment.update();break;
   case "bonuses":EquipmentContainer.updateBonuses(player);break;
   case "stats":EquipmentContainer.update(player);break;
   default:throw new AssertionError(a.get("op"));
  }}catch(ClassCastException x){error="Type";}catch(NullPointerException x){error="Null";}catch(IndexOutOfBoundsException x){error="Bounds";}catch(IllegalStateException x){error="Binding";}
  results.add(map("result",result,"error",error,"state",state()));}
  Map names=new TreeMap();for(Map.Entry<String,Item> e:named.entrySet())names.put(e.getKey(),item(e.getValue()));return map("results",results,"trace",trace,"frames",frames,"named",names);
 }
 public static void main(String[]args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=reader.readLine())!=null){Map r=(Map)new JSONParser().parse(line);r.put("expected",evaluate(r));System.out.println(JSONValue.toJSONString(r));}}
}
