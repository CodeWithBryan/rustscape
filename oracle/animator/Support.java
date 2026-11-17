package oracle;
import java.io.*;import java.lang.reflect.*;import java.nio.*;import java.util.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.cache.def.impl.AnimationDefinition;
import core.game.node.entity.Entity;import core.game.node.entity.impl.Animator;import core.game.node.entity.impl.Animator.Priority;
import core.game.world.update.UpdateMasks;import core.game.world.update.flag.EntityFlag;
import core.game.world.update.flag.context.*;import core.game.world.update.flag.chunk.GraphicUpdateFlag;
import core.game.world.map.*;import core.game.node.scenery.Scenery;
public class Support {
 static Map input;static Animator animator;static Animation[]animations;static Graphics[]graphics;static Entity[]entities;static Location[]locations;static Scenery[]objects;
 static List<AnimationDefinition>definitions;static Map<Integer,byte[]>files;static List<Object>events,registrations,queued;static Map<String,Integer>calls;static int tick,selected,position;static boolean accepted;
 static class Failure extends RuntimeException{}
 static int n(Object o){return ((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 static Object field(Object o,String name){try{Field f=o.getClass().getDeclaredField(name);f.setAccessible(true);return f.get(o);}catch(Exception e){throw new RuntimeException(e);}}
 static void set(Object o,String name,Object value){try{Field f=o.getClass().getDeclaredField(name);f.setAccessible(true);f.set(o,value);}catch(Exception e){throw new RuntimeException(e);}}
 static Map<Integer,AnimationDefinition>cache(){return (Map<Integer,AnimationDefinition>)fieldStatic(AnimationDefinition.class,"animDefs");}
 static Object fieldStatic(Class c,String name){try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(null);}catch(Exception e){throw new RuntimeException(e);}}
 static Object copy(Object o){if(o==null)return null;if(o.getClass().isArray()){List<Object>r=new ArrayList<>();for(int i=0;i<Array.getLength(o);i++)r.add(copy(Array.get(o,i)));return r;}return o;}
 static int[]ints(Object o){if(o==null)return null;List l=(List)o;int[]a=new int[l.size()];for(int i=0;i<a.length;i++)a[i]=n(l.get(i));return a;}
 static byte[]bytes(Object o){if(o==null)return null;String s=(String)o;byte[]r=new byte[s.length()/2];for(int i=0;i<r.length;i++)r[i]=(byte)Integer.parseInt(s.substring(2*i,2*i+2),16);return r;}
 static String hex(byte[]a){if(a==null)return null;StringBuilder s=new StringBuilder();for(byte b:a)s.append(String.format("%02x",b&255));return s.toString();}
 static Integer def(AnimationDefinition d){if(d==null)return null;for(int i=0;i<definitions.size();i++)if(definitions.get(i)==d)return i;definitions.add(d);return definitions.size()-1;}
 static Integer animation(Animation a){if(a==null)return null;for(int i=0;i<animations.length;i++)if(animations[i]==a)return i;throw new AssertionError();}
 static Integer graphic(Graphics g){if(g==null)return null;for(int i=0;i<graphics.length;i++)if(graphics[i]==g)return i;throw new AssertionError();}
 static Integer location(Location l){if(l==null)return null;for(int i=0;i<locations.length;i++)if(locations[i]==l)return i;throw new AssertionError();}
 static Animation anim(Map a){return a.get("animation")==null?null:animations[n(a.get("animation"))];}static Graphics gfx(Map a){return a.get("graphic")==null?null:graphics[n(a.get("graphic"))];}
 static Priority priority(Object o){return o==null?null:Priority.values()[n(o)];}
 static Object definition(AnimationDefinition d){if(d==null)return null;Map<String,Object>q=new TreeMap<>();for(Field f:AnimationDefinition.class.getDeclaredFields())if(!Modifier.isStatic(f.getModifiers()))try{f.setAccessible(true);q.put(f.getName(),copy(f.get(d)));}catch(Exception e){throw new RuntimeException(e);}q.put("milliseconds",d.getDuration());q.put("cycles",d.getCycles());q.put("ticks",d.getDurationTicks());return q;}
 public static void emit(String event,Object...args){List<Object>r=new ArrayList<>();r.add(event);r.addAll(Arrays.asList(args));events.add(r);int at=calls.merge(event,1,Integer::sum);for(Object o:(List)input.getOrDefault("hooks",Collections.emptyList())){Map h=(Map)o;if(event.equals(h.get("event"))&&at==v(h,"at",1))for(Object a:(List)h.get("actions"))act((Map)a);}}
 public static int now(){int value=tick;emit("tick",value);return value;}
 public static byte[]file(int g,int f){int id=(g<<7)|f;byte[]value=files.get(id);emit("file",g,f,hex(value));return value;}
 public static boolean register(UpdateMasks m,EntityFlag f,Object c){registrations.add(Arrays.asList(m.id,f.name(),c));emit("register",m.id,f.name(),c instanceof Animation?animation((Animation)c):graphic((Graphics)c));return accepted;}
 public static RegionManager.Chunk chunk(Location l){int value=selected;emit("chunk",location(l),value);return value<0?null:new RegionManager.Chunk(value);}
 public static void made(Graphics g,Location l){emit("make_flag",graphic(g),location(l));}
 public static void enqueue(RegionManager.Chunk c,GraphicUpdateFlag f){queued.add(Arrays.asList(c.id,f.graphic,f.location));emit("enqueue",c.id,graphic(f.graphic),location(f.location));}
 static Object act(Map a){Object value=a.get("value");switch((String)a.get("op")){
 case"decode":{ByteBuffer b=ByteBuffer.wrap(bytes(a.get("data")));try{Method m=AnimationDefinition.class.getDeclaredMethod("readValueLoop",ByteBuffer.class);m.setAccessible(true);m.invoke(definitions.get(v(a,"definition",0)),b);}catch(InvocationTargetException e){Throwable t=e.getCause();if(t instanceof RuntimeException)throw (RuntimeException)t;throw new RuntimeException(t);}catch(ReflectiveOperationException e){throw new RuntimeException(e);}finally{position=b.position();}break;}
 case"finish":definitions.get(v(a,"definition",0)).method2394();break;
 case"audit":{List<Object>rows=new ArrayList<>();for(int id=0;id<v(a,"count",0);id++){AnimationDefinition d=AnimationDefinition.forId(id);def(d);rows.add(Arrays.asList(id,definition(d),d==AnimationDefinition.forId(id)));}return rows;}
 case"def_patch":{AnimationDefinition d=definitions.get(n(a.get("definition")));if(a.containsKey("durations"))d.durations=ints(a.get("durations"));break;}
 case"lookup":return def(AnimationDefinition.forId(n(a.get("id"))));
 case"cache_set":cache().put(n(a.get("id")),a.get("definition")==null?null:definitions.get(n(a.get("definition"))));break;
 case"cache_clear":cache().clear();break;
 case"duration":return anim(a).getDuration();case"get_definition":return def(anim(a).getDefinition());
 case"animate":return animator.animate(anim(a),gfx(a));case"force":animator.forceAnimation(anim(a));break;
 case"reset":animator.reset();break;case"stop":animator.stop();break;case"is_animating":return animator.isAnimating();
 case"priority":animator.setPriority(priority(value));break;case"anim_priority":anim(a).setPriority(priority(value));break;
 case"id":anim(a).setId(n(value));break;case"object":anim(a).setObject(value==null?null:objects[n(value)]);break;
 case"set_animation":animator.setAnimation(anim(a));break;case"set_graphics":animator.setGraphics(gfx(a));break;
 case"clock":tick=n(value);break;case"clocks":entities[v(a,"entity",0)].clocks=ints(value);break;
 case"mask_select":entities[v(a,"entity",0)].masks=new UpdateMasks(n(value));break;case"register_result":accepted=(Boolean)value;break;
 case"file":files.put(n(a.get("id")),bytes(a.get("data")));break;
 case"chunk_select":selected=n(value);break;case"send":Graphics.send(gfx(a),a.get("location")==null?null:locations[n(a.get("location"))]);break;
 case"move":{List c=(List)a.get("coords");locations[n(a.get("location"))].moveTo(n(c.get(0)),n(c.get(1)),n(c.get(2)));break;}
 case"fail":throw new Failure();default:throw new IllegalArgumentException((String)a.get("op"));}return null;}
 static Object snapshot(Object result,String error){Map<String,Object>r=new TreeMap<>();r.put("result",result);r.put("error",error);r.put("events",new ArrayList<>(events));r.put("tick",tick);r.put("position",position);r.put("deadline",field(animator,"ticks"));r.put("priority",animator.getPriority()==null?null:animator.getPriority().ordinal());r.put("animation",animation(animator.getAnimation()));r.put("graphic",graphic(animator.getGraphics()));
 Map<String,Object>cached=new TreeMap<>();for(int id:new TreeSet<>(cache().keySet()))cached.put(Integer.toString(id),def(cache().get(id)));r.put("cache",cached);
 List<Object>as=new ArrayList<>();for(Animation a:animations){Object o=a.getObject();as.add(Arrays.asList(a.getId(),a.getDelay(),a.getPriority()==null?null:a.getPriority().ordinal(),def((AnimationDefinition)field(a,"definition")),o==null?null:((Scenery)o).id,a.toString()));}r.put("animations",as);
 List<Object>ds=new ArrayList<>();for(AnimationDefinition d:definitions)ds.add(definition(d));r.put("definitions",ds);
 List<Object>gs=new ArrayList<>();for(Graphics g:graphics)gs.add(Arrays.asList(g.getId(),g.getHeight(),g.getDelay(),g.toString()));r.put("graphics",gs);
 List<Object>cs=new ArrayList<>(),ms=new ArrayList<>();for(Entity e:entities){cs.add(copy(e.clocks));ms.add(e.masks.id);}r.put("clocks",cs);r.put("masks",ms);r.put("accepted",accepted);r.put("selected",selected);
 List<Object>rs=new ArrayList<>();for(Object o:registrations){List q=(List)o;Object c=q.get(2);rs.add(Arrays.asList(q.get(0),q.get(1),c instanceof Animation?animation((Animation)c):graphic((Graphics)c)));}r.put("registrations",rs);
 List<Object>qs=new ArrayList<>();for(Object o:queued){List q=(List)o;qs.add(Arrays.asList(q.get(0),graphic((Graphics)q.get(1)),location((Location)q.get(2))));}r.put("queued",qs);
 List<Object>ls=new ArrayList<>();for(Location l:locations)ls.add(Arrays.asList(l.getX(),l.getY(),l.getZ()));r.put("locations",ls);return r;}
 public static void main(String[]args)throws Exception{try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null){input=(Map)new JSONParser().parse(line);Map out=new TreeMap(input);cache().clear();definitions=new ArrayList<>(Arrays.asList(new AnimationDefinition(),new AnimationDefinition()));files=new HashMap<>();Map fs=(Map)input.get("files");for(Object k:fs.keySet())files.put(Integer.parseInt((String)k),bytes(fs.get(k)));events=new ArrayList<>();calls=new HashMap<>();registrations=new ArrayList<>();queued=new ArrayList<>();tick=v(input,"tick",100);selected=v(input,"chunk",0);position=0;accepted=!Boolean.FALSE.equals(input.get("register"));
 entities=new Entity[]{new Entity(0),new Entity(1)};for(Entity e:entities){e.masks=new UpdateMasks(e.id);if(input.containsKey("clocks"))e.clocks=ints(input.get("clocks"));}
 animations=new Animation[]{new Animation(1),new Animation(2,7,Priority.HIGH),new Animation(0),new Animation(-1),new Animation(535,30,Priority.LOW),new Animation(863,0,null),Animation.RESET,Animator.RESET_A};
 for(int i=6;i<8;i++){animations[i].setId(-1);animations[i].setPriority(i==6?Priority.VERY_HIGH:Priority.MID);animations[i].setObject(null);set(animations[i],"definition",null);}
 graphics=new Graphics[]{new Graphics(111,128,60),new Graphics(369,256,30),Animator.RESET_G};locations=new Location[]{new Location(3207,3216,0),new Location(3208,3216,0)};objects=new Scenery[]{new Scenery(0),new Scenery(1)};
 animator=new Animator(input.containsKey("owner")&&input.get("owner")==null?null:entities[v(input,"owner",0)]);
 List<Object>states=new ArrayList<>();for(Object a:(List)input.get("actions")){events.clear();Object result=null;String error=null;try{result=act((Map)a);}catch(Failure e){error="Binding";}catch(NullPointerException e){error="Null";}catch(IndexOutOfBoundsException e){error="Index";}catch(BufferUnderflowException e){error="Decode";}states.add(snapshot(result,error));}out.put("expected",states);System.out.println(JSONValue.toJSONString(out));}}}
}
