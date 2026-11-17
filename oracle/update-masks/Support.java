package oracle;
import java.io.*;import java.lang.reflect.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.world.update.UpdateMasks;import core.game.world.update.flag.*;import core.game.world.update.flag.context.HitMark;
import core.game.node.entity.*;import core.game.node.entity.player.Player;import core.game.node.entity.combat.ImpactHandler;import core.net.packet.IoBuffer;
public class Support {
 static Map input;static UpdateMasks masks;static Entity[]entities;public static ImpactHandler[]handlers;
 static Map<String,P>providers;static List<P>all;static int next;static C[]contexts;static List<Object>events;static Map<String,Integer>calls;static IoBuffer buffer;static long clock;
 static class C{int id,value;C(int i,int v){id=i;value=v;}}
 static class Failure extends RuntimeException{}
 static int n(Object o){return((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 static class P extends EFlagProvider {int id;P(EFlagType k,EntityFlag f,Map m){super(530,k,v(m,"presence",64),v(m,"ordinal",10),EntityFlag.valueOf((String)m.getOrDefault("encodedFlag",f.name())));id=next++;all.add(this);}
 @Override public void writeToDynamic(IoBuffer b,Object c,Entity e){b.put(160);emit("encode",id,context(c),e.id);b.put(id);b.put(c==null?0:c instanceof C?((C)c).value:((HitMark)c).getDamage());b.put(e.id);}}
 public static void emit(String event,Object...args){List<Object>r=new ArrayList<>();r.add(event);r.addAll(Arrays.asList(args));events.add(r);int at=calls.merge(event,1,Integer::sum);for(Object o:(List)input.getOrDefault("hooks",Collections.emptyList())){Map h=(Map)o;if(event.equals(h.get("event"))&&at==v(h,"at",1))for(Object a:(List)h.get("actions"))act((Map)a);}}
 public static EFlagProvider provider(EFlagType k,EntityFlag f){P p=providers.get(k+":"+f);emit("provider",k.name(),f.name(),p==null?null:p.id);return p;}
 public static int presence(EFlagType k,EntityFlag f){P p=providers.get(k+":"+f);int value=p==null?0:p.getPresenceFlag();emit("presence",k.name(),f.name(),value);return value;}
 public static void missing(String s){emit("missing",s);}
 public static long now(){long value=clock++;emit("clock",value);return value;}
 static Object context(Object c){if(c==null)return null;if(c instanceof C){C x=(C)c;return Arrays.asList("context",x.id,x.value);}HitMark h=(HitMark)c;return Arrays.asList("hit",h.getDamage(),h.getType(),h.getEntity().id,h.showHealthBar);}
 static Object field(Object object,String name){try{Field f=object.getClass().getDeclaredField(name);f.setAccessible(true);return f.get(object);}catch(Exception e){throw new RuntimeException(e);}}
 static Object slots(String name){List<Object>r=new ArrayList<>();for(Object element:(Object[])field(masks,name))r.add(element==null?null:Arrays.asList(((P)field(element,"encoder")).id,context(field(element,"context"))));return r;}
 static void queue(int id,Object value){handlers[id].impactQueue.clear();for(Object o:(List)value){List l=(List)o;handlers[id].impactQueue.add(new ImpactHandler.Impact(n(l.get(0)),n(l.get(1))));}}
 static Object act(Map a){Object value=a.get("value");switch((String)a.get("op")){
 case"register":return masks.register(EntityFlag.valueOf((String)a.getOrDefault("flag","FaceLocation")),a.get("ctx")==null?null:contexts[n(a.get("ctx"))],Boolean.TRUE.equals(a.get("sync")));
 case"reset":masks.reset();break;case"unregister":return masks.unregisterSynced(v(a,"ordinal",10));
 case"stamp":masks.setAppearanceStamp(((Number)value).longValue());break;
 case"write":Entity e=a.containsKey("entity")&&a.get("entity")==null?null:entities[v(a,"entity",0)];Player p=a.containsKey("viewer")&&a.get("viewer")==null?null:(Player)entities[v(a,"viewer",0)];if(Boolean.TRUE.equals(a.get("synced")))masks.writeSynced(p,e,buffer,!Boolean.FALSE.equals(a.get("appearance")));else masks.write(p,e,buffer);break;
 case"prepare":masks.prepare(entities[v(a,"entity",0)]);break;
 case"context":contexts[v(a,"id",0)].value=n(value);break;
 case"handler":entities[v(a,"entity",0)].handler=n(value);break;
 case"queue":queue(v(a,"handler",0),value);break;
 case"provider":EFlagType k=EFlagType.valueOf((String)input.getOrDefault("kind","Player"));EntityFlag f=EntityFlag.valueOf((String)a.get("flag"));if(value==null)providers.remove(k+":"+f);else providers.put(k+":"+f,new P(k,f,(Map)value));break;
 case"fail":throw new Failure();default:throw new IllegalArgumentException();}return null;}
 static Object snapshot(Object result,String error){Map<String,Object>r=new TreeMap<>();r.put("result",result);r.put("error",error);r.put("events",new ArrayList<>(events));r.put("stamp",masks.getAppearanceStamp());r.put("updating",masks.isUpdating());r.put("required",masks.isUpdateRequired());r.put("hasSynced",masks.hasSynced());r.put("presence",field(masks,"presenceFlags"));r.put("syncedPresence",field(masks,"syncedPresenceFlags"));r.put("current",slots("elements"));r.put("synced",slots("syncedElements"));List<Object>bytes=new ArrayList<>();for(byte b:buffer.out.toByteArray())bytes.add(b&255);r.put("bytes",bytes);List<Object>queues=new ArrayList<>();for(ImpactHandler h:handlers){List<Object>q=new ArrayList<>();for(ImpactHandler.Impact i:h.impactQueue)q.add(Arrays.asList(i.amount,i.type.ordinal()));queues.add(q);}r.put("queues",queues);r.put("handlers",Arrays.asList(entities[0].handler,entities[1].handler));r.put("contexts",Arrays.asList(contexts[0].value,contexts[1].value));r.put("clock",clock);return r;}
 public static void main(String[]args)throws Exception{try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null){input=(Map)new JSONParser().parse(line);Map out=new TreeMap(input);entities=new Entity[]{new Player(0),new Entity(1)};handlers=new ImpactHandler[]{new ImpactHandler(0),new ImpactHandler(1)};entities[1].handler=1;contexts=new C[]{new C(0,17),new C(1,33)};events=new ArrayList<>();calls=new HashMap<>();buffer=new IoBuffer();clock=1000;providers=new HashMap<>();all=new ArrayList<>();next=0;
 Map defaults=(Map)input.get("defaults");for(String kind:new String[]{"Player","NPC"})for(Object o:(List)defaults.get(kind)){Map d=(Map)o;EntityFlag f=EntityFlag.valueOf((String)d.get("flag"));providers.put(kind+":"+f,new P(EFlagType.valueOf(kind),f,d));}
 EFlagType kind=EFlagType.valueOf((String)input.getOrDefault("kind","Player"));for(Object o:(List)input.getOrDefault("providers",Collections.emptyList())){Map d=(Map)o;EntityFlag f=EntityFlag.valueOf((String)d.get("flag"));providers.put(kind+":"+f,new P(kind,f,d));}
 masks=new UpdateMasks(entities[kind==EFlagType.Player?0:1]);queue(0,input.getOrDefault("queue",Collections.emptyList()));
 List<Object>states=new ArrayList<>();for(Object a:(List)input.get("actions")){events.clear();Object result=null;String error=null;try{result=act((Map)a);}catch(Failure e){error="Binding";}catch(NullPointerException e){error="Null";}catch(IndexOutOfBoundsException e){error="Index";}states.add(snapshot(result,error));}out.put("expected",states);System.out.println(JSONValue.toJSONString(out));}}}
}
