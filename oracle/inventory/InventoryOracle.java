package oracle;
import core.cache.def.Definition;
import core.cache.def.impl.ItemDefinition;
import core.game.container.*;
import core.game.node.Node;
import core.game.node.item.*;
import core.game.interaction.*;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;

public class InventoryOracle {
 static Map<String,Object> request;
 static Map<Integer,Container> containers;
 static Map<String,Item> items;
 static List<ContainerEvent> retainedEvents;
 static List<Item[]> retainedArrays;
 static List<Object> trace;
 static Map<String,Integer> calls;
 static Map<Integer,Listener> listenerObjects;
 static Listener listener(int id){return listenerObjects.computeIfAbsent(id,Listener::new);}
 static Map<String,Object> map(Object... pairs){Map<String,Object> m=new TreeMap<>();for(int i=0;i<pairs.length;i+=2)m.put((String)pairs[i],pairs[i+1]);return m;}
 static int number(Object v){return ((Number)v).intValue();}
 static int integer(Map<?,?> m,String k,int fallback){return m.containsKey(k)?number(m.get(k)):fallback;}
 static boolean flag(Map<?,?> m,String k,boolean fallback){return m.containsKey(k)?(Boolean)m.get(k):fallback;}
 static Field field(Class<?> type,String name){try{while(type!=null){try{Field f=type.getDeclaredField(name);f.setAccessible(true);return f;}catch(NoSuchFieldException e){type=type.getSuperclass();}}throw new NoSuchFieldException(name);}catch(Exception e){throw new RuntimeException(e);}}
 static Object scalar(Object value){return value instanceof Long?((Long)value).intValue():value;}
 static Item getItem(Map<?,?> a){Object name=a.get("item");return name==null?null:items.get((String)name);}
 static Item[] list(Map<?,?> a){List<?> names=(List<?>)a.get("items");Item[] values=new Item[names.size()];for(int i=0;i<values.length;i++)values[i]=names.get(i)==null?null:items.get((String)names.get(i));return values;}
 static Object ref(Object value,IdentityHashMap<Object,Integer> ids){return ids.computeIfAbsent(value,key->ids.size());}
 static Object item(Item item,IdentityHashMap<Object,Integer> ids){
  if(item==null)return null;ItemDefinition d=item.getDefinition();
  Map<String,Object> result=map("idHash",item.getIdHash(),"id",item.getId(),"amount",item.getAmount(),"charge",item.getCharge(),"name",item.getName(),"index",item.getIndex(),"destination",item.getDestinationFlag()==null?null:"ITEM","definition",d==null?null:map("id",d.getId(),"name",d.getName(),"value",d.getValue(),"stackable",d.isStackable(),"unnoted",d.isUnnoted(),"noteId",d.getNoteId()));
  if(ids!=null)result.put("ref",ref(item,ids));return result;
 }
 static Object array(Item[] values,IdentityHashMap<Object,Integer> ids){List<Object> data=new ArrayList<>();Object tag=ids==null?null:ref(values,ids);for(Item value:values)data.add(item(value,ids));return ids==null?data:map("ref",tag,"values",data);}
 static Object event(ContainerEvent event,IdentityHashMap<Object,Integer> ids){Object tag=ids==null?null:ref(event,ids);List<Integer> slots=new ArrayList<>();for(int slot:event.getSlots())slots.add(slot);Map<String,Object> result=map("items",array(event.getItems(),ids),"clear",event.isClear(),"changeCount",event.getChangeCount(),"slots",slots);if(ids!=null)result.put("ref",tag);return result;}
 static Object state(){
  IdentityHashMap<Object,Integer> ids=new IdentityHashMap<>();Map<String,Object> cs=new TreeMap<>(),is=new TreeMap<>();
  for(Map.Entry<Integer,Container> entry:containers.entrySet()){
   Container c=entry.getValue();List<Object> listeners=new ArrayList<>();for(ContainerListener listener:c.getListeners())listeners.add(listener==null?null:((Listener)listener).id);
   Object values=array(c.toArray(),ids),ev=event(c.getEvent(),ids);
   cs.put(entry.getKey().toString(),map("capacity",c.capacity(),"items",values,"event",ev,"listeners",listeners,"freeSlots",c.freeSlots(),"itemCount",c.itemCount()));
  }
  for(Map.Entry<String,Item> entry:items.entrySet())is.put(entry.getKey(),item(entry.getValue(),ids));
  List<Object> events=new ArrayList<>(),arrays=new ArrayList<>();for(ContainerEvent e:retainedEvents)events.add(event(e,ids));for(Item[] a:retainedArrays)arrays.add(array(a,ids));
  return map("containers",cs,"items",is,"retainedEvents",events,"retainedArrays",arrays,"nullItem",item(ContainerEvent.NULL_ITEM,ids),"trace",new ArrayList<>(trace));
 }
 static class Listener implements ContainerListener {
  int id;Listener(int id){this.id=id;}
  void call(String kind,Container c,ContainerEvent event){
   String key=id+":"+kind;int count=calls.merge(key,1,Integer::sum);
   trace.add(map("kind",kind,"listener",id,"items",array(c.toArray(),null),"event",event==null?null:InventoryOracle.event(event,null)));
   if(event!=null)retainedEvents.add(event);
   Map<?,?> scripts=(Map<?,?>)request.get("scripts");if(scripts==null)return;List<?> list=(List<?>)scripts.get(""+id);if(list==null)return;
   for(Object rule:list){Map<?,?> r=(Map<?,?>)rule;if(kind.equals(r.get("when"))&&number(r.get("at"))==count){try{action((Map<?,?>)r.get("action"),c);}catch(RuntimeException e){throw e;}catch(Exception e){throw new RuntimeException(e);}}}
  }
  public void update(Container c,ContainerEvent event){call("update",c,event);}
  public void refresh(Container c){call("refresh",c,null);}
 }
 public static int replacement(int id){trace.add(map("kind","replacement","id",id));if(request.containsKey("dropAction")){try{action((Map<?,?>)request.get("dropAction"),null);}catch(RuntimeException e){throw e;}catch(Exception e){throw new RuntimeException(e);}}Map<?,?> data=(Map<?,?>)request.get("drop");return data!=null&&data.containsKey(""+id)?number(data.get(""+id)):id;}
 public static void ground(Item item){trace.add(map("kind","ground","item",item(item,null)));}
 static Object store(Map<?,?> a,Item value){if(a.containsKey("as_"))items.put((String)a.get("as_"),value);return item(value,null);}
 static Object action(Map<?,?> a,Container local)throws Exception{
  String op=(String)a.get("op");Item item=getItem(a);Container c=local==null?containers.get(integer(a,"container",0)):local;boolean fire=flag(a,"fire",true);int slot=integer(a,"slot",0);
  switch(op){
   case "itemQuery":return map("value",item.getValue(),"alchemy",item.getAlchemyValue(),"noteChange",item.getNoteChange(),"charged",item.isCharged(),"text",item.toString());
   case "itemAlchemy":return item.getDefinition().getAlchemyValue(flag(a,"high",true));
   case "itemAmount":item.setAmount(number(a.get("value")));return null;
   case "itemCharge":item.setCharge(number(a.get("value")));return null;
   case "itemId":item.setId(number(a.get("value")));return null;
   case "itemIndex":item.setIndex(number(a.get("value")));return null;
   case "itemDefinition":item.setDefinition(a.get("value")==null?null:ItemDefinition.forId(number(a.get("value"))));return null;
   case "itemCopy":return store(a,item.copy());
   case "itemUnnote":return store(a,item.toUnnotedItem());
   case "itemDrop":return store(a,item.getDropItem());
   case "itemPlugin":{ItemPlugin plugin=item.getPlugin();return plugin==null?null:plugin.name;}
   case "itemHasPlugin":return item.hasItemPlugin();
   case "itemOperate":{OptionHandler handler=item.getOperateHandler();return handler==null?null:handler.name;}
   case "add":return c.add(item,fire,integer(a,"preferred",-1));
   case "addGround":return c.add(item,new core.game.node.entity.player.Player());
   case "addAbsent":return c.addIfDoesntHave(item);
   case "addMany":return c.add(list(a));
   case "addList":c.addList(Arrays.asList(list(a)));return null;
   case "remove":return a.containsKey("slot")?c.remove(item,slot,fire):c.remove(item,fire);
   case "removeMany":return c.remove(list(a));
   case "removeAll":{int[] ids=((List<?>)a.get("ids")).stream().mapToInt(InventoryOracle::number).toArray();return c.removeAll(ids);}
   case "replace":return store(a,c.replace(item,slot,fire));
   case "insert":c.insert(integer(a,"from_",0),integer(a,"to",0),fire);return null;
   case "get":return store(a,c.get(slot));
   case "getNew":return store(a,c.getNew(slot));
   case "getId":return c.getId(slot);
   case "getAsId":return c.getAsId(slot);
   case "getItem":return store(a,c.getItem(item));
   case "getByItem":return store(a,c.get(item));
   case "getAll":{List<Object> result=new ArrayList<>();for(Item i:c.getAll(item))result.add(item(i,null));return result;}
   case "query":return map("slot",c.getSlot(item),"hashSlot",c.getSlotHash(item),"amount",c.getAmount(item),"maximum",c.getMaximumAdd(item),"space",c.hasSpaceFor(item),"addSlot",c.getAddSlot(item),"empty",c.isEmpty(),"full",c.isFull(),"wealth",c.getWealth(),"freeSlot",c.freeSlot());
   case "contains":return c.contains(integer(a,"id",0),integer(a,"amount",0));
   case "containsItem":return c.containsItem(item);
   case "containsItems":return c.containsItems(list(a));
   case "containsOneItems":return c.containsAtLeastOneItem(list(a));
   case "containsOneId":return c.containsAtLeastOneItem(integer(a,"id",0));
   case "containsOneIds":return c.containsAtLeastOneItem(((List<?>)a.get("ids")).stream().mapToInt(InventoryOracle::number).toArray());
   case "containsAll":return c.containsAll(((List<?>)a.get("ids")).stream().mapToInt(InventoryOracle::number).toArray());
   case "containItems":return c.containItems(((List<?>)a.get("ids")).stream().mapToInt(InventoryOracle::number).toArray());
   case "update":if(a.containsKey("force"))c.update((Boolean)a.get("force"));else c.update();return null;
   case "refresh":if(a.containsKey("listener"))c.refresh(a.get("listener")==null?null:listener(number(a.get("listener"))));else c.refresh();return null;
   case "clear":c.clear(fire);return null;
   case "shift":c.shift();return null;
   case "format":return c.format();
   case "parse":c.parse((JSONArray)a.get("value"));return null;
   case "copy":c.copy(containers.get(integer(a,"source",0)));return null;
   case "addAll":c.addAll(containers.get(integer(a,"source",0)));return null;
   case "spaceContainer":return c.hasSpaceFor(containers.get(integer(a,"source",0)));
   case "spaceMany":return c.hasSpaceFor(list(a));
   case "register":c.register(a.get("value")==null?null:listener(number(a.get("value"))));return null;
   case "unregister":return c.getListeners().remove(a.get("value")==null?null:listener(number(a.get("value"))));
   case "removeListener":c.getListeners().remove(number(a.get("value")));return null;
   case "captureEvent":retainedEvents.add(c.getEvent());return null;
   case "captureArray":retainedArrays.add(c.toArray());return null;
   case "eventClear":c.getEvent().setClear((Boolean)a.get("value"));return null;
   case "rawSet":c.toArray()[slot]=item;return null;
   case "eventSet":c.getEvent().getItems()[slot]=item;return null;
   case "retainedSet":retainedArrays.get(number(a.get("array")))[slot]=item;return null;
   case "nullAmount":ContainerEvent.NULL_ITEM.setAmount(number(a.get("value")));return null;
   case "mutateSlot":{Item value=c.get(slot);String f=(String)a.get("field");if(f.equals("amount"))value.setAmount(number(a.get("value")));else value.setCharge(number(a.get("value")));return null;}
   case "fail":throw new RuntimeException("injected listener failure");
   default:throw new AssertionError(op);
  }
 }
 static Object evaluate(Map<String,Object> value)throws Exception{
  if(value.get("kind").equals("integerMap")){Map<Integer,Integer> map=new HashMap<>();for(Object key:(List<?>)value.get("keys"))map.put(number(key),1);return new ArrayList<>(map.keySet());}
  request=value;containers=new TreeMap<>();items=new TreeMap<>();retainedEvents=new ArrayList<>();retainedArrays=new ArrayList<>();trace=new ArrayList<>();calls=new HashMap<>();listenerObjects=new HashMap<>();ItemDefinition.getDefinitions().clear();
  for(Map.Entry<?,?> entry:((Map<?,?>)value.get("definitions")).entrySet()){
   ItemDefinition d=new ItemDefinition();for(Map.Entry<?,?> e:((Map<?,?>)entry.getValue()).entrySet()){
    Field f=field(ItemDefinition.class,(String)e.getKey());if(e.getKey().equals("handlers")){Map<String,Object> handlers=(Map<String,Object>)f.get(d);((Map<?,?>)e.getValue()).forEach((k,v)->handlers.put((String)k,v instanceof String&&k.equals("wrapper")?new ItemPlugin((String)v):v instanceof String&&k.equals("option:operate")?new OptionHandler((String)v):scalar(v)));}else f.set(d,scalar(e.getValue()));
   }ItemDefinition.getDefinitions().put(Integer.parseInt((String)entry.getKey()),d);
  }
  // Isolate the mutable static placeholder exactly as a fresh world/class loader.
  Item fresh=new Item(0,0);for(Class<?> type:Arrays.asList(Item.class,Node.class))for(Field f:type.getDeclaredFields())if(!Modifier.isStatic(f.getModifiers())){f.setAccessible(true);f.set(ContainerEvent.NULL_ITEM,f.get(fresh));}
  for(Map.Entry<?,?> entry:((Map<?,?>)value.get("items")).entrySet()){
   Map<?,?> fields=(Map<?,?>)entry.getValue();Item item=fields==null?null:flag(fields,"empty",false)?new Item():new Item(integer(fields,"id",0),integer(fields,"amount",1),integer(fields,"charge",1000));items.put((String)entry.getKey(),item);
  }
  for(Object row:(List<?>)value.get("containers")){Map<?,?> fields=(Map<?,?>)row;Container c=new Container(number(fields.get("capacity")),ContainerType.valueOf((String)fields.get("type")),SortType.valueOf((String)fields.get("sort")));for(Object id:(List<?>)fields.get("listeners"))c.register(id==null?null:listener(number(id)));containers.put(number(fields.get("id")),c);}
  List<Object> output=new ArrayList<>();for(Object row:(List<?>)value.get("actions")){Object result=null;String error=null;try{result=action((Map<?,?>)row,null);}catch(Exception e){error=e.getClass().getSimpleName();}output.add(map("result",result,"error",error,"state",state()));}return output;
 }
 public static void main(String[] args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));JSONParser parser=new JSONParser();String line;while((line=reader.readLine())!=null){Map<String,Object> request=(Map<String,Object>)parser.parse(line);request.put("expected",evaluate(request));System.out.println(JSONValue.toJSONString(request));}}
}
