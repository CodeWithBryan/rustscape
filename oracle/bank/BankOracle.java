package oracle;
import java.io.*;import java.util.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.cache.def.impl.ItemDefinition;import core.game.container.*;import core.game.container.impl.BankContainer;import core.game.node.item.Item;import core.game.node.entity.player.Player;import core.game.component.*;import core.net.packet.*;import core.net.packet.context.ContainerContext;import core.net.packet.out.ContainerPacket;
public class BankOracle {
 public static Map row;public static Player[]players;static BankContainer[]banks;static Map<String,Item>named;static List<Object>log,frames;static List<ContainerContext>packets;static List<Item[]>arrays;static List<CloseEvent>closes;static Map<String,Integer>counts;
 public static List<Map<String,Integer>>vars;public static List<Map<String,Object>>attributes;
 static int n(Map m,String k,int d){Object x=m.get(k);return x==null?d:((Number)x).intValue();}
 public static boolean flag(String key){return Boolean.TRUE.equals(row.get(key));}
 static int[] ints(List list){return list.stream().mapToInt(x->((Number)x).intValue()).toArray();}
 public static void closeEvent(Component c,CloseEvent e){hit("closeEvent",c.id);closes.add(e);}
 public static void hit(String key,Object...args){
  List<Object>entry=new ArrayList<>();entry.add(key);entry.addAll(Arrays.asList(args));log.add(entry);int count=counts.merge(key,1,Integer::sum);
  for(Object value:(List)row.get("hooks")){Map hook=(Map)value;if(!key.equals(hook.get("when"))||n(hook,"at",1)!=count)continue;
   switch((String)hook.get("action")){
    case"fail":throw new IllegalStateException("controlled");
    case"swapBank":players[0].bank=players[0].bank==banks[0]?banks[1]:banks[0];break;
    case"clearInventory":Arrays.fill(players[0].inventory.toArray(),null);break;
    case"clearBank":Arrays.fill(banks[0].toArray(),null);break;
    case"replaceInventory":players[0].inventory=new Container(28);break;
    case"amount":named.get((String)hook.getOrDefault("item","a")).setAmount(n(hook,"value",7));break;
    case"event":banks[0].getEvent().flag(n(hook,"slot",0),named.get("a"));break;
    case"reenter":banks[0].refresh();break;
    case"close":banks[0].close();break;
    default:throw new AssertionError(hook);
   }
  }
 }
 static class Recording implements ContainerListener{int id;Recording(int id){this.id=id;}public void update(Container c,ContainerEvent e){hit("inventoryUpdate",id);}public void refresh(Container c){hit("inventoryRefresh",id);}}
 public static void packet(ContainerContext c){hit("packet",c.getPlayer()==null?-1:c.getPlayer().id,c.getInterfaceId(),c.getChildId(),c.getContainerId());packets.add(c);new ContainerPacket().send(c);}
 public static void frame(IoBuffer b){java.nio.ByteBuffer bytes=b.toByteBuffer();bytes.flip();StringBuilder s=new StringBuilder();while(bytes.hasRemaining())s.append(String.format("%02x",bytes.get()&255));frames.add(Arrays.asList(b.opcode()&255,s.toString()));}
 static int tag(Object o,IdentityHashMap<Object,Integer>ids){return ids.computeIfAbsent(o,v->ids.size());}
 static Object item(Item i,IdentityHashMap<Object,Integer>ids){return i==null?null:Arrays.asList(tag(i,ids),i.getId(),i.getAmount(),i.getCharge(),i.getIndex());}
 static Object array(Item[]a,IdentityHashMap<Object,Integer>ids){int ref=tag(a,ids);Map<String,Object>slots=new TreeMap<>();for(int i=0;i<a.length;i++)if(a[i]!=null)slots.put(""+i,item(a[i],ids));return Map.of("ref",ref,"length",a.length,"slots",slots);}
 static Object event(ContainerEvent e,IdentityHashMap<Object,Integer>ids){int ref=tag(e,ids);Object items=array(e.getItems(),ids);return Map.of("ref",ref,"items",items,"clear",e.isClear());}
 static Object container(Container c,IdentityHashMap<Object,Integer>ids){Object items=array(c.toArray(),ids),event=event(c.getEvent(),ids);List<Object>listeners=new ArrayList<>();for(ContainerListener l:c.getListeners()){Object name=l==null?null:l instanceof Recording?"inv"+((Recording)l).id:"unknown";for(int i=0;i<banks.length;i++){try{if(l!=null&&InventoryOracle.field(BankContainer.class,"listener").get(banks[i])==l)name="bank"+i;}catch(Exception ex){throw new RuntimeException(ex);}}listeners.add(name);}return Map.of("items",items,"event",event,"listeners",listeners);}
 static Object state(){IdentityHashMap<Object,Integer>ids=new IdentityHashMap<>();List<Object>bs=new ArrayList<>(),inv=new ArrayList<>(),as=new ArrayList<>(),ps=new ArrayList<>();Map<String,Object>names=new TreeMap<>();
  for(BankContainer b:banks){Map<String,Object>m=new TreeMap<>((Map)container(b,ids));m.put("open",b.isOpen());m.put("last",b.getLastAmountX());m.put("tab",b.getTabIndex());m.put("tabs",b.getTabStartSlot());bs.add(m);}
  for(Player p:players)inv.add(container(p.inventory,ids));for(Map.Entry<String,Item>e:named.entrySet())names.put(e.getKey(),item(e.getValue(),ids));for(Item[]a:arrays)as.add(array(a,ids));for(ContainerContext p:packets)ps.add(Map.of("items",array(p.getItems(),ids),"slots",p.getSlots()==null?Collections.emptyList():Arrays.stream(p.getSlots()).boxed().toArray(),"length",p.getLength()));
  Map<String,Object>result=new TreeMap<>(Map.of("banks",bs,"inventories",inv,"named",names,"arrays",as,"packets",ps,"active",Arrays.asList(players[0].bank==banks[0]?0:1,2),"vars",vars,"attributes",attributes,"closes",closes.size()));if(flag("ownership")){List<Object>equipment=new ArrayList<>();for(Player p:players)equipment.add(container(p.equipment,ids));result.put("equipment",equipment);}return result;
 }
 static Object action(Map a)throws Exception {BankContainer b=banks[n(a,"bank",0)];String op=(String)a.get("op");int slot=n(a,"slot",0),amount=n(a,"amount",1),value=a.get("value") instanceof Number?((Number)a.get("value")).intValue():0;Item item=named.get(a.getOrDefault("item","a"));
  switch(op){
   case"hasItem":return players[n(a,"player",0)].hasItem(item);
   case"open":b.open();break;case"openFor":b.open(n(a,"player",0)<0?null:players[n(a,"player",0)]);break;case"close":b.close();break;case"depositBox":b.openDepositBox();break;case"refreshDeposit":b.refreshDepositBoxInterface();break;
   case"deposit":b.addItem(slot,amount);break;case"withdraw":b.takeItem(slot,amount);break;case"last":b.updateLastAmountX(value);break;case"tab":b.setTabIndex(value);break;
   case"increase":b.increaseTabStartSlots(value);break;case"decrease":b.decreaseTabStartSlots(value);break;case"collapse":b.collapseTab(value);break;
   case"itemsInTab":return b.getItemsInTab(value);case"tabBySlot":return b.getTabByItemSlot(slot);case"arrayIndex":return BankContainer.getArrayIndex(value);
   case"space":b.sendBankSpace();break;case"configs":b.setTabConfigurations();break;case"canAdd":return b.canAdd(item);case"maximum":return b.getMaximumAdd(item);
   case"note":b.setNoteItems(Boolean.TRUE.equals(a.get("value")));break;case"insert":b.setInsertItems(Boolean.TRUE.equals(a.get("value")));break;case"modes":return Arrays.asList(b.isNoteItems(),b.isInsertItems());
   case"refresh":b.refresh();break;case"update":b.update();break;case"baseAdd":return b.add(item,Boolean.TRUE.equals(a.getOrDefault("fire",Boolean.TRUE)),n(a,"slot",-1));
   case"clearListeners":b.getListeners().clear();break;case"array":arrays.add(b.toArray());break;case"clear":b.clear(false);break;case"closeCallback":return closes.get(value).close(players[n(a,"player",0)],new Component(0));
   default:throw new AssertionError(op);
  }return null;
 }
 static Object evaluate(Map input)throws Exception {
  row=input;log=new ArrayList<>();frames=new ArrayList<>();packets=new ArrayList<>();arrays=new ArrayList<>();closes=new ArrayList<>();counts=new HashMap<>();vars=Arrays.asList(new TreeMap<>(),new TreeMap<>());attributes=Arrays.asList(new TreeMap<>(),new TreeMap<>());
  Map init=new HashMap();init.put("kind","sequence");init.put("definitions",row.get("definitions"));init.put("items",row.get("items"));init.put("containers",Collections.emptyList());init.put("actions",Collections.emptyList());InventoryOracle.evaluate(init);named=InventoryOracle.items;
  players=new Player[]{new Player(0),new Player(1)};banks=new BankContainer[]{new BankContainer(flag("nullPlayer")?null:players[0]),new BankContainer(players[0]),new BankContainer(players[1])};players[0].bank=banks[n(row,"activeBank",0)];players[1].bank=banks[2];
  for(int i=0;i<2;i++){players[i].inventory.register(new Recording(i));Map slots=(Map)((List)row.get("inventories")).get(i);for(Object key:slots.keySet())players[i].inventory.toArray()[Integer.parseInt((String)key)]=named.get(slots.get(key));}
  for(int i=0;i<3;i++){Map slots=(Map)((List)row.get("banks")).get(i);for(Object key:slots.keySet())banks[i].toArray()[Integer.parseInt((String)key)]=named.get(slots.get(key));List tabs=(List)((List)row.get("tabs")).get(i);for(int j=0;j<11;j++)banks[i].getTabStartSlot()[j]=((Number)tabs.get(j)).intValue();}
  if(flag("ownership")){for(int i=0;i<2;i++){Map slots=(Map)((List)row.get("equipment")).get(i);for(Object key:slots.keySet())players[i].equipment.toArray()[Integer.parseInt((String)key)]=named.get(slots.get(key));}}
  if(flag("noteMode"))vars.get(0).put("7001",1);log.clear();counts.clear();List<Object>out=new ArrayList<>();
  for(Object x:(List)row.get("actions")){Object result=null;String error=null;try{result=action((Map)x);}catch(NullPointerException e){error="Null";}catch(IndexOutOfBoundsException|NegativeArraySizeException e){error="Bounds";}catch(ClassCastException e){error="Type";}catch(ConcurrentModificationException e){error="Concurrent";}catch(IllegalStateException e){error="Binding";}out.add(InventoryOracle.map("result",result,"error",error,"state",portable(state()),"log",portable(new ArrayList<>(log)),"frames",portable(new ArrayList<>(frames))));}return out;
 }
 static Object portable(Object value){if(value==null)return null;if(value instanceof Map){Map<String,Object>out=new TreeMap<>();((Map<?,?>)value).forEach((k,v)->out.put(k.toString(),portable(v)));return out;}if(value instanceof Collection){List<Object>out=new ArrayList<>();for(Object v:(Collection)value)out.add(portable(v));return out;}if(value.getClass().isArray()){List<Object>out=new ArrayList<>();for(int i=0;i<Array.getLength(value);i++)out.add(portable(Array.get(value,i)));return out;}return value;}
 public static void main(String[]args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=reader.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(portable(row)));}}
}
