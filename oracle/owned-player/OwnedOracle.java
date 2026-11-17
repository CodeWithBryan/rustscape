package oracle;
import java.util.*;
import java.io.*;
import java.lang.reflect.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.cache.def.impl.ItemDefinition;
import core.game.container.*;
import core.game.container.impl.InventoryListener;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import core.net.packet.IoBuffer;
public class OwnedOracle {
 static List<Object> frames;
 static IdentityHashMap<Player,String> owners;
 static Map map(Object...values){Map out=new TreeMap();for(int i=0;i<values.length;i+=2)out.put(values[i],values[i+1]);return out;}
 public static void frame(Player player,IoBuffer buffer){
  java.nio.ByteBuffer data=buffer.toByteBuffer();data.flip();StringBuilder hex=new StringBuilder();while(data.hasRemaining())hex.append(String.format("%02x",data.get()&255));
  frames.add(map("owner",owners.get(player),"opcode",buffer.opcode()&255,"payloadHex",hex.toString()));
 }
 static Object observe(Player p){return map("weightBits",String.format("%016x",Double.doubleToLongBits(p.settings.getWeight())),"pouch",p.familiar.isHasPouch(),"syncs",p.syncs,"counter",p.manager.packetCount,"inventory",BoundOracle.items(p.inventory),"equipment",BoundOracle.items(p.equipment));}
 static Object evaluate(Map row)throws Exception{
  frames=new ArrayList<>();owners=new IdentityHashMap<>();ItemDefinition.getDefinitions().clear();
  for(Object raw:((Map)row.get("weights")).entrySet()){
   Map.Entry e=(Map.Entry)raw;int id=Integer.parseInt((String)e.getKey());ItemDefinition d=new ItemDefinition();d.setId(id);
   Field stack=ItemDefinition.class.getDeclaredField("stackable");stack.setAccessible(true);stack.setBoolean(d,id==1);
   d.getHandlers().put(core.game.system.config.ItemConfigParser.WEIGHT,BoundOracle.weight(e.getValue()));ItemDefinition.getDefinitions().put(id,d);
  }
  Map<String,Player> players=new TreeMap<>();
  for(String name:new String[]{"A","B"}){
   Player p=new Player();players.put(name,p);owners.put(p,name);Map data=(Map)row.get(name);
   p.manager.packetCount=BoundOracle.n(data,"counter",0);p.familiar.setHasPouch(Boolean.TRUE.equals(data.get("had")));
   List inv=(List)data.get("inventory");for(int i=0;i<inv.size();i++)p.inventory.toArray()[i]=BoundOracle.item(inv.get(i));
   for(Object raw:((Map)data.get("equipment")).entrySet()){Map.Entry e=(Map.Entry)raw;p.equipment.toArray()[Integer.parseInt((String)e.getKey())]=BoundOracle.item(e.getValue());}
  }
  Player targetPlayer=players.get(row.get("target")),listenerPlayer=players.get(row.get("listener"));
  Container target=Boolean.TRUE.equals(row.get("equipmentTarget"))?targetPlayer.equipment:targetPlayer.inventory;
  target.register(new InventoryListener(listenerPlayer));String error=null;
  try{switch((String)row.get("op")){
   case "refresh":target.refresh();break;
   case "add":target.add(BoundOracle.item(row.get("item")),true);break;
   case "replace":target.replace(BoundOracle.item(row.get("item")),0,true);break;
   case "clear":target.clear(true);break;
   default:throw new AssertionError(row.get("op"));
  }}catch(ClassCastException e){error="Type";}catch(NullPointerException e){error="Null";}catch(IllegalStateException e){error="Binding";}
  return map("frames",frames,"A",observe(players.get("A")),"B",observe(players.get("B")),"error",error);
 }
 public static void main(String[]args)throws Exception{
  BufferedReader input=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;
  while((line=input.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(row));}
 }
}
