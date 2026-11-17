package oracle;
import java.util.*;import java.io.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.container.*;import core.game.container.impl.InventoryListener;import core.game.node.item.Item;import core.game.node.entity.player.Player;import core.net.packet.*;import core.net.packet.context.ContainerContext;import core.net.packet.out.ContainerPacket;
public class ListenerOracle{
 static Map row;static Player player;static Container container;static InventoryListener listener;static List<Object>log,frames;static List<ContainerContext>packets;static Map<String,Integer>counts;
 static int n(Map m,String k,int d){Object v=m.get(k);return v==null?d:((Number)v).intValue();}
 public static boolean nil(String key){return key.equals(row.get("null"));}
 static Item item(Object v){if(v==null)return null;if("NULL".equals(v))return ContainerEvent.NULL_ITEM;Map m=(Map)v;return new Item(n(m,"id",0),n(m,"amount",1),n(m,"charge",1000));}
 public static void hit(String label,Object...args){
  if(args.length==0)log.add(label);else{List<Object>entry=new ArrayList<>();entry.add(label);entry.addAll(Arrays.asList(args));log.add(entry);}
  int count=counts.getOrDefault(label,0)+1;counts.put(label,count);
  for(Object o:(List)row.get("hooks")){Map h=(Map)o;if(!label.equals(h.get("when"))||count!=n(h,"at",1))continue;
   switch((String)h.get("action")){case"clear":container.clear(false);break;case"pouch":container.toArray()[0]=new Item(12047);break;case"ordinary":Arrays.fill(container.toArray(),null);container.toArray()[0]=new Item(1);break;case"swap":player.active=1-player.active;break;case"had":player.fam[0].has=!player.fam[0].has;break;case"fail":throw new IllegalStateException("hook");case"reenter":listener.update(container);break;default:throw new AssertionError();}
  }
 }
 public static void send(ContainerContext c){hit("send");packets.add(c);new ContainerPacket().send(c);}
 public static void frame(IoBuffer b){java.nio.ByteBuffer data=b.toByteBuffer();data.flip();StringBuilder hex=new StringBuilder();while(data.hasRemaining())hex.append(String.format("%02x",data.get()&255));frames.add(Map.of("opcode",b.opcode()&255,"payloadHex",hex.toString()));}
 static Object items(Item[]array){List<Object>out=new ArrayList<>();for(Item i:array)out.add(i==null?null:Arrays.asList(i.getId(),i.getAmount(),i.getCharge()));return out;}
 static Object evaluate(Map input){
  row=input;log=new ArrayList<>();frames=new ArrayList<>();packets=new ArrayList<>();counts=new HashMap<>();player=new Player();player.fam[0].has=Boolean.TRUE.equals(row.get("had"));player.fam[1].has=Boolean.TRUE.equals(row.get("otherHad"));
  container=new Container(4);List start=(List)row.get("items");for(int i=0;i<4;i++)container.toArray()[i]=item(start.get(i));listener=new InventoryListener(nil("player")?null:player);String error=null;
  try{switch((String)row.get("op")){case"update":listener.update(container);break;case"refresh":listener.refresh(container);break;case"event":ContainerEvent e=new ContainerEvent(4);e.setClear(Boolean.TRUE.equals(row.get("eventClear")));for(int i=0;i<4;i++)e.flag(i,item(((List)row.get("event")).get(i)));listener.update(container,nil("event")?null:e);break;case"fire":Item[]old=container.toArray().clone();for(int i=0;i<4;i++)container.replace(old[i],i,false);container.register(listener);container.update();break;default:throw new AssertionError();}}catch(NullPointerException e){error="Null";}catch(IllegalStateException e){error="Binding";}
  List<Object>contexts=new ArrayList<>();for(ContainerContext c:packets){Map<String,Object>p=new LinkedHashMap<>();p.put("items",items(c.getItems()));List<Integer>slots=null;if(c.getSlots()!=null){slots=new ArrayList<>();for(int n:c.getSlots())slots.add(n);}p.put("slots",slots);p.put("sameAsContainer",c.getItems()==container.toArray());contexts.add(p);}
  Map<String,Object>out=new LinkedHashMap<>();out.put("log",log);out.put("frames",frames);out.put("packets",contexts);out.put("pouches",Arrays.asList(player.fam[0].has,player.fam[1].has));out.put("activeFamiliar",player.active);out.put("syncs",player.appearance.syncs);out.put("items",items(container.toArray()));out.put("error",error);return out;
 }
 public static void main(String[]args)throws Exception{try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));List<Integer>keys=new ArrayList<>();for(int i=0;i<=65535;i++)if(content.global.skill.summoning.SummoningPouch.get(i)!=null)keys.add(i);row.put("pouchKeys",keys);System.out.println(JSONValue.toJSONString(row));}}}
}
