package oracle;
import java.io.*;import java.lang.reflect.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.world.update.UpdateMasks;import core.game.world.update.flag.*;import core.game.world.update.flag.context.*;import core.game.world.map.Location;
import core.game.node.entity.*;import core.game.node.entity.player.Player;import core.game.node.entity.combat.ImpactHandler;import core.net.packet.IoBuffer;
public class Support {
 static Entity[]entities;public static ImpactHandler[]handlers;static Location[]locations;static UpdateMasks masks;static IoBuffer buffer;static Map<EntityFlag,EFlagProvider>providers;
 static int n(Object o){return((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 public static void emit(String event,Object...args){}public static void missing(String message){throw new IllegalStateException(message);}public static long now(){return 1000;}
 public static EFlagProvider provider(EFlagType kind,EntityFlag flag){return kind==EFlagType.Player?providers.get(flag):null;}
 public static int presence(EFlagType kind,EntityFlag flag){EFlagProvider p=provider(kind,flag);return p==null?0:p.getPresenceFlag();}
 static Object act(Map a){switch((String)a.get("op")){
 case"location":return entities[0].faceLocation(a.get("location")==null?null:locations[n(a.get("location"))]);
 case"face":return entities[0].face(a.get("target")==null?null:entities[n(a.get("target"))]);
 case"move":List c=(List)a.get("coords");locations[n(a.get("location"))].moveTo(n(c.get(0)),n(c.get(1)),n(c.get(2)));break;
 case"index":entities[n(a.get("entity"))].index=n(a.get("value"));break;
 case"animate":return masks.register(EntityFlag.Animate,new Animation(v(a,"id",0),v(a,"delay",0)));
 case"spot":return masks.register(EntityFlag.SpotAnim,new Graphics(v(a,"id",0),v(a,"height",0),v(a,"delay",0)));
 case"prepare":masks.prepare(entities[0]);break;case"reset":masks.reset();break;
 case"write":if(Boolean.TRUE.equals(a.get("synced")))masks.writeSynced((Player)entities[0],entities[0],buffer,!Boolean.FALSE.equals(a.get("appearance")));else masks.write((Player)entities[0],entities[0],buffer);break;
 default:throw new IllegalArgumentException();}return null;}
 static Object field(String name)throws Exception{Field f=UpdateMasks.class.getDeclaredField(name);f.setAccessible(true);return f.get(masks);}
 static Object snapshot(Object result)throws Exception{Map<String,Object>r=new TreeMap<>();r.put("result",result);r.put("updating",masks.isUpdating());r.put("required",masks.isUpdateRequired());r.put("synced",masks.hasSynced());r.put("stamp",masks.getAppearanceStamp());r.put("presence",field("presenceFlags"));r.put("syncedPresence",field("syncedPresenceFlags"));List<Object>bytes=new ArrayList<>();java.nio.ByteBuffer b=buffer.toByteBuffer();for(int i=0;i<b.position();i++)bytes.add(b.get(i)&255);r.put("bytes",bytes);r.put("indices",Arrays.asList(entities[0].index,entities[1].index,entities[2].index));List<Object>points=new ArrayList<>();for(Location l:locations)points.add(Arrays.asList(l.getX(),l.getY(),l.getZ()));r.put("locations",points);return r;}
 public static void main(String[]args)throws Exception{try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null){Map input=(Map)new JSONParser().parse(line);Map out=new TreeMap(input);entities=new Entity[]{new Player(0),new Player(1),new Entity(2)};handlers=new ImpactHandler[]{new ImpactHandler(0),new ImpactHandler(1)};locations=new Location[]{new Location(3207,3216,0),new Location(3208,3216,0)};masks=new UpdateMasks(entities[0]);entities[0].masks=masks;buffer=new IoBuffer();providers=new HashMap<>();for(Object o:(List)((Map)input.get("defaults")).get("Player")){Map d=(Map)o;EntityFlag f=EntityFlag.valueOf((String)d.get("flag"));providers.put(f,new EFlagProvider(530,EFlagType.Player,v(d,"presence",0),v(d,"ordinal",0),f));}
 for(EFlagProvider p:new EFlagProvider[]{new PlayerFlags530.Animate(),new PlayerFlags530.SpotAnim(),new PlayerFlags530.FaceEntity(),new PlayerFlags530.FaceLocation(),new PlayerFlags530.AnimationSequence()})providers.put(p.getFlag(),p);
 List<Object>states=new ArrayList<>();for(Object a:(List)input.get("actions"))states.add(snapshot(act((Map)a)));out.put("expected",states);System.out.println(JSONValue.toJSONString(out));}}}
}
