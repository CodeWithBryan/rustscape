package oracle;
import java.util.*;import java.io.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.cache.def.impl.*;import core.game.container.Container;import core.game.node.entity.player.Player;import core.game.node.entity.player.link.appearance.*;import core.game.node.entity.combat.equipment.WeaponInterface;import core.game.node.entity.impl.Animator.Priority;import core.game.node.item.Item;import core.game.world.update.flag.context.Animation;
public class AppearanceOracle {
 public static Map row;public static Object override;static Player[] players;static Map<String,Appearance> apps;static Map<String,Item> items;static Map<Integer,NPCDefinition> npcs;static List<Object> trace;static Map<String,Integer> hits;static Map<String,Object> held;
 static IdentityHashMap<Object,Integer> arrays,parts,animations;
 static Map map(Object...v){Map m=new TreeMap();for(int i=0;i<v.length;i+=2)m.put(v[i],v[i+1]);return m;}
 static int n(Map m,String k,int d){Object v=m.get(k);return v==null?d:((Number)v).intValue();}
 static int ref(IdentityHashMap<Object,Integer> m,Object v){if(!m.containsKey(v))m.put(v,m.size());return m.get(v);}
 static Object part(BodyPart p){return p==null?null:map("ref",ref(parts,p),"look",p.getLook(),"color",p.getColor());}
 public static Object animation(Animation a){return a==null?null:map("ref",ref(animations,a),"id",a.getId(),"delay",a.getDelay(),"priority",a.getPriority()==null?null:a.getPriority().name());}
 static Object ints(int[] a){List<Integer> v=new ArrayList<>();for(int i:a)v.add(i);return v;}
 static Object equipment(Container c){List<Object> v=new ArrayList<>();for(Item i:c.toArray())v.add(i==null?null:map("id",i.getId(),"amount",i.getAmount(),"charge",i.getCharge(),"index",i.getIndex()));return v;}
 static Object partList(BodyPart[] p){List<Object> v=new ArrayList<>();for(BodyPart b:p)v.add(part(b));return v;}
 static Object raw(Appearance a){try{int cache=ref(arrays,a.getAppearanceCache());Object pv=partList(a.getAppearanceCache());int anim=ref(arrays,a.getAnimationCache());int body=ref(arrays,a.getBodyParts());int owner=-1;for(Player p:players)if(a.getPlayer()==p)owner=p.id;Field f=Appearance.class.getDeclaredField("renderAnimationId");f.setAccessible(true);return map("partsRef",cache,"parts",pv,"animationsRef",anim,"animations",ints(a.getAnimationCache()),"bodyRef",body,"body",ints(a.getBodyParts()),"gender",a.getGender()==null?null:(int)a.getGender().toByte(),"npc",a.getNpcId(),"riding",a.isRidingMinecart(),"render",f.getInt(a),"skull",a.getSkullIcon(),"head",a.getHeadIcon(),"player",owner);}catch(Exception e){throw new AssertionError(e);}}
 static Object state(){Map av=new TreeMap();for(Map.Entry<String,Appearance> e:apps.entrySet())av.put(e.getKey(),raw(e.getValue()));List<Object> pv=new ArrayList<>();for(Player p:players)pv.add(map("equipment",equipment(p.equipment),"size",p.size,"prepared",Arrays.asList(p.renders[0].prepared,p.renders[1].prepared),"activeRender",p.activeRender,"attack",animation(p.properties.attack),"defence",animation(p.properties.defence),"death",animation(p.properties.death)));Map hv=new TreeMap();for(Map.Entry<String,Object> e:held.entrySet()){Object v=e.getValue();hv.put(e.getKey(),map("ref",ref(arrays,v),"values",v instanceof BodyPart[]?partList((BodyPart[])v):ints((int[])v)));}return map("apps",av,"players",pv,"held",hv);}
 public static void hit(String key,Object...args){List<Object> event=new ArrayList<>();event.add(key);event.addAll(Arrays.asList(args));trace.add(event);int at=hits.getOrDefault(key,0)+1;hits.put(key,at);
  for(Object raw:(List)row.get("hooks")){Map h=(Map)raw;if(!key.equals(h.get("when"))||n(h,"at",1)!=at)continue;switch((String)h.get("action")){
   case "fail":throw new IllegalStateException("controlled");
   case "clearEquipment":Arrays.fill(players[0].equipment.toArray(),null);break;
   case "redirect":players[0].appearance=apps.get("B");break;
   case "part":apps.get("A").getHair().changeLook(77);break;
   case "override":override=null;break;
   case "prepared":players[0].activeRender=1;break;
   case "reenter":apps.get("A").transformNPC(-1);break;
  }}
 }
 public static NPCDefinition npc(int id){hit("npc",id);return npcs.computeIfAbsent(id,k->new NPCDefinition());}
 static Object value(Object v){if(v instanceof Long)return((Long)v).intValue();return v;}
 static Object arrayConfig(Object v){if(!(v instanceof List))return value(v);Animation[] a=new Animation[((List)v).size()];for(int i=0;i<a.length;i++){Object x=((List)v).get(i);a[i]=x==null?null:new Animation(((Number)x).intValue(),Priority.HIGH);}return a;}
 static Item make(Map m)throws Exception{if(m==null)return null;Item i=new Item(n(m,"id",0),n(m,"amount",1),n(m,"charge",1000));if(m.containsKey("definition"))i.setDefinition(m.get("definition")==null?null:ItemDefinition.forId(n(m,"definition",0)));return i;}
 static void reset(){int[][] looks={{0,10,18,26,33,36,42},{45,1000,56,61,68,70,79}};for(Gender g:Gender.values())for(int i=0;i<7;i++)g.getAppearanceCache()[i]=new BodyPart(looks[g.toByte()][i]);int[] a={808,823,819,820,821,822,824};System.arraycopy(a,0,Appearance.AppearanceCache.ANIMATIONS,0,7);int[] ids={422,423,422,422};for(int i=0;i<4;i++)WeaponInterface.DEFAULT_ANIMS[i]=new Animation(ids[i],Priority.HIGH);}
 static Object evaluate(Map input)throws Exception{row=input;reset();override=value(row.get("override"));trace=new ArrayList<>();hits=new HashMap<>();arrays=new IdentityHashMap<>();parts=new IdentityHashMap<>();animations=new IdentityHashMap<>();held=new TreeMap<>();items=new TreeMap<>();npcs=new TreeMap<>();ItemDefinition.getDefinitions().clear();
  for(Object raw:((Map)row.get("definitions")).entrySet()){Map.Entry e=(Map.Entry)raw;int id=Integer.parseInt((String)e.getKey());Map data=(Map)e.getValue();ItemDefinition d=new ItemDefinition();d.setId(id);Field f=ItemDefinition.class.getDeclaredField("equipId");f.setAccessible(true);f.setInt(d,n(data,"equip",0));for(Object rr:data.entrySet()){Map.Entry x=(Map.Entry)rr;String key=(String)x.getKey();d.getHandlers().put(key,key.equals("attack_anims")?arrayConfig(x.getValue()):value(x.getValue()));}ItemDefinition.getDefinitions().put(id,d);}
  for(Object raw:((Map)row.get("npcs")).entrySet()){Map.Entry e=(Map.Entry)raw;Map d=(Map)e.getValue();NPCDefinition npc=new NPCDefinition();npc.size=n(d,"size",1);npc.renderAnimationId=n(d,"render",0);npc.standAnimation=n(d,"stand",-1);npc.walkAnimation=n(d,"walk",-1);npc.turnAnimation=n(d,"turn",-1);npc.turn180Animation=n(d,"turn180",-1);npc.turnCWAnimation=n(d,"cw",-1);npc.turnCCWAnimation=n(d,"ccw",-1);for(String key:new String[]{"melee","defence","death"})if(d.containsKey(key)){Object v=d.get(key);npc.getHandlers().put(key.equals("melee")?"melee_animation":key+"_animation",v instanceof Number?new Animation(((Number)v).intValue(),Priority.HIGH):v);}npcs.put(Integer.parseInt((String)e.getKey()),npc);}
  players=new Player[]{new Player(0),new Player(1)};apps=new TreeMap<>();for(int i=0;i<2;i++){Appearance a=new Appearance(i==0&&Boolean.TRUE.equals(row.get("nullOwner"))?null:players[i]);players[i].appearance=a;apps.put(i==0?"A":"B",a);}if(Boolean.TRUE.equals(row.get("redirect")))players[0].appearance=apps.get("B");
  for(Object raw:((Map)row.get("items")).entrySet()){Map.Entry e=(Map.Entry)raw;items.put((String)e.getKey(),make((Map)e.getValue()));}for(Object raw:((Map)row.get("equipment")).entrySet()){Map.Entry e=(Map.Entry)raw;players[0].equipment.toArray()[Integer.parseInt((String)e.getKey())]=items.get(e.getValue());}
  List<Object> results=new ArrayList<>();
  for(Object raw:(List)row.get("actions")){Map a=(Map)raw;Appearance app=apps.get(a.getOrDefault("target","A"));Object result=null;String error=null;try{int index=n(a,"index",0);switch((String)a.get("op")){
   case "prepare":app.prepareBodyData(players[n(a,"player",0)]);break;
   case "construct":apps.put((String)a.get("name"),new Appearance(players[0]));break;
   case "gender":app.setGender(a.get("value")==null?null:n(a,"value",0)==0?Gender.MALE:Gender.FEMALE);break;
   case "changeGender":app.changeGender(n(a,"value",0)==0?Gender.MALE:Gender.FEMALE);break;
   case "parse":app.parse((JSONObject)a.get("value"));break;
   case "part":BodyPart p=app.getAppearanceCache()[index];if(a.containsKey("look"))p.changeLook(n(a,"look",0));if(a.containsKey("color"))p.changeColor(n(a,"color",0));break;
   case "icons":app.setSkullIcon(n(a,"skull",0));app.setHeadIcon(n(a,"head",0));break;
   case "animation":app.getAnimationCache()[index]=n(a,"value",0);break;
   case "getter":String name=(String)a.get("name");result=part(name.equals("hair")?app.getHair():name.equals("beard")?app.getBeard():name.equals("torso")?app.getTorso():name.equals("arms")?app.getArms():name.equals("wrists")?app.getWrists():name.equals("skin")?app.getSkin():name.equals("legs")?app.getLegs():app.getFeet());break;
   case "nullPart":app.getAppearanceCache()[index]=null;break;
   case "capture":held.put((String)a.get("name"),a.get("kind").equals("parts")?app.getAppearanceCache():a.get("kind").equals("animations")?app.getAnimationCache():app.getBodyParts());break;
   case "retainedPart":p=((BodyPart[])held.get(a.get("name")))[index];if(a.containsKey("look"))p.changeLook(n(a,"look",0));if(a.containsKey("color"))p.changeColor(n(a,"color",0));break;
   case "heldSet":((int[])held.get(a.get("name")))[index]=n(a,"value",0);break;
   case "copy":app.copy(apps.get(a.get("source")));break;
   case "riding":app.setRidingMinecart(Boolean.TRUE.equals(a.get("value")));break;
   case "defaultAnimations":app.setDefaultAnimations();break;
   case "default":String kind=(String)a.get("kind");if(kind.equals("gender"))Gender.MALE.getAppearanceCache()[index].changeLook(n(a,"value",0));else if(kind.equals("animations"))Appearance.AppearanceCache.ANIMATIONS[index]=n(a,"value",0);else WeaponInterface.DEFAULT_ANIMS[index].setId(n(a,"value",0));break;
   case "defaultPart":p=Gender.MALE.getAppearanceCache()[index];p.changeLook(n(a,"look",0));p.changeColor(n(a,"color",0));break;
   case "defaultNull":Gender.MALE.getAppearanceCache()[index]=null;break;
   case "drawClothes":app.drawClothes(index,n(a,"value",0));break;
   case "drawItem":app.drawItem(index,items.get(a.get("item")));break;
   case "clear":app.clearBodyPart(index);break;
   case "clip":app.flagHatClipping();break;
   case "setAnimations":app.setAnimations();break;
   case "transform":app.transformNPC(n(a,"value",-1));break;
   case "setRender":app.setAnimations(new Animation(n(a,"value",0)));break;
   case "render":result=app.getRenderAnimation();break;
   case "configAnimation":((Animation)npcs.get(1).getConfiguration("melee_animation")).setId(n(a,"value",0));break;
   case "cart":app.rideCart(Boolean.TRUE.equals(a.get("value")));break;
   case "copter":app.flyCopter();break;
   default:throw new AssertionError(a.get("op"));
  }}catch(ClassCastException e){error="Type";}catch(NullPointerException e){error="Null";}catch(IndexOutOfBoundsException e){error="Index";}catch(NumberFormatException e){error="Number";}catch(IllegalStateException e){error="Binding";}
   results.add(map("result",result,"error",error,"state",state()));
  }return map("results",results,"trace",trace);
 }
 public static void main(String[]args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=reader.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(row));}}
}
