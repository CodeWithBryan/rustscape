package oracle;
import java.util.*;import java.io.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;import core.cache.def.impl.ItemDefinition;import core.game.container.*;import core.game.container.impl.InventoryListener;import core.game.node.entity.player.Player;import core.game.node.item.Item;import core.net.packet.IoBuffer;
public class BoundOracle{
 static Player player;static Map row;static List<Object>frames;static Set<Integer>done;
 static int n(Map m,String k,int d){Object v=m.get(k);return v==null?d:((Number)v).intValue();}
 static Object weight(Object v){if(v==null)return null;if(v instanceof Map){Map m=(Map)v;if(m.containsKey("integer"))return Integer.valueOf(n(m,"integer",0));return Double.longBitsToDouble(Long.parseUnsignedLong((String)m.get("bits"),16));}return Double.valueOf(((Number)v).doubleValue());}
 static Item item(Object v){if(v==null)return null;Map m=(Map)v;return new Item(n(m,"id",0),n(m,"amount",1),n(m,"charge",1000));}
 static void changeWeight(int id,Object v){ItemDefinition.forId(id).getHandlers().put(core.game.system.config.ItemConfigParser.WEIGHT,weight(v));}
 public static void frame(IoBuffer buffer){
  java.nio.ByteBuffer b=buffer.toByteBuffer();b.flip();StringBuilder hex=new StringBuilder();while(b.hasRemaining())hex.append(String.format("%02x",b.get()&255));int index=frames.size();frames.add(Map.of("opcode",buffer.opcode()&255,"payloadHex",hex.toString()));
  List hooks=(List)row.get("hooks");for(int i=0;i<hooks.size();i++){Map h=(Map)hooks.get(i);if(done.contains(i)||n(h,"at",0)!=index)continue;done.add(i);switch((String)h.get("action")){case"clear_inventory":player.inventory.clear(false);break;case"clear_equipment":player.equipment.clear(false);break;case"pouch":player.inventory.toArray()[0]=new Item(12047);break;case"weight":changeWeight(1,4.75);break;case"fail":throw new IllegalStateException("channel");case"refresh":player.inventory.refresh();break;default:throw new AssertionError();}}
 }
 static Object items(Container c){List<Object>out=new ArrayList<>();for(Item i:c.toArray())out.add(i==null?null:Arrays.asList(i.getId(),i.getAmount(),i.getCharge()));return out;}
 static Object evaluate(Map input)throws Exception{
  row=input;frames=new ArrayList<>();done=new HashSet<>();ItemDefinition.getDefinitions().clear();
  for(Object raw:((Map)row.get("weights")).entrySet()){Map.Entry e=(Map.Entry)raw;int id=Integer.parseInt((String)e.getKey());ItemDefinition d=new ItemDefinition();d.setId(id);Field f=ItemDefinition.class.getDeclaredField("stackable");f.setAccessible(true);f.setBoolean(d,id==1);d.getHandlers().put(core.game.system.config.ItemConfigParser.WEIGHT,weight(e.getValue()));ItemDefinition.getDefinitions().put(id,d);}
  player=new Player();player.inventory.register(new InventoryListener(player));player.manager.packetCount=n(row,"counter",0);player.familiar.setHasPouch(Boolean.TRUE.equals(row.get("had")));
  List inv=(List)row.get("inventory");for(int i=0;i<inv.size();i++)player.inventory.toArray()[i]=item(inv.get(i));
  for(Object raw:((Map)row.get("equipment")).entrySet()){Map.Entry e=(Map.Entry)raw;player.equipment.toArray()[Integer.parseInt((String)e.getKey())]=item(e.getValue());}
  Field field=Container.class.getDeclaredField("event");field.setAccessible(true);Object event=field.get(player.inventory);String error=null;
  try{switch((String)row.get("op")){case"refresh":player.inventory.refresh();break;case"add":player.inventory.add(item(row.get("item")),true);break;case"clear":player.inventory.clear(true);break;case"replace":player.inventory.replace(item(row.get("item")),0,true);break;default:throw new AssertionError();}}catch(ClassCastException e){error="Type";}catch(NullPointerException e){error="Null";}catch(IllegalStateException e){error="Binding";}
  Map<String,Object>out=new LinkedHashMap<>();out.put("frames",frames);out.put("weightBits",String.format("%016x",Double.doubleToLongBits(player.settings.getWeight())));out.put("pouch",player.familiar.isHasPouch());out.put("syncs",player.syncs);out.put("counter",player.manager.packetCount);out.put("inventory",items(player.inventory));out.put("equipment",items(player.equipment));out.put("sameEvent",event==field.get(player.inventory));out.put("error",error);return out;
 }
 public static void main(String[]args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(row));}}}
}
