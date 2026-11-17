package oracle;
import java.util.*;import java.io.*;import java.nio.*;import java.lang.reflect.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.net.packet.*;import core.net.packet.in.*;import core.net.packet.out.*;import core.net.packet.context.*;
import core.game.node.entity.player.Player;import core.game.node.item.Item;import core.game.node.scenery.Scenery;import core.game.world.map.Location;
public class GamePackets {
 static int n(Object o){return ((Number)o).intValue();}static int arg(JSONObject x,int i){List a=(List)x.get("args");return i<a.size()?n(a.get(i)):0;}
 static byte[] bytes(List a){byte[] b=new byte[a.size()];for(int i=0;i<b.length;i++)b[i]=(byte)n(a.get(i));return b;}
 static List<Integer> unsigned(byte[] a){List<Integer> b=new ArrayList<>();for(byte v:a)b.add(v&255);return b;}
 static Map<String,Object> action(Object value)throws Exception{
  Map<String,Object> out=new TreeMap<>();out.put("type",value.getClass().getSimpleName());
  for(Field f:value.getClass().getDeclaredFields()){if(Modifier.isStatic(f.getModifiers())||f.getName().equals("player"))continue;f.setAccessible(true);out.put(f.getName(),f.get(value));}return out;
 }
 static Object evaluate(JSONObject x)throws Exception{
  String kind=(String)x.get("kind");Map<String,Object> out=new TreeMap<>();
  if(kind.equals("readUpdate"))return UpdateFields.decode(n(x.get("opcode")),bytes((List)x.get("data")));
  if(kind.equals("decode")){
   int opcode=n(x.get("opcode"));ByteBuffer raw=ByteBuffer.wrap(bytes((List)x.get("data")));IoBuffer b=new IoBuffer(opcode,null,raw);
   try{Decoders530 d=Arrays.stream(Decoders530.values()).filter(v->v.getOpcode()==opcode).findFirst().get();out.put("action",action(d.decode(new Player(),b)));}
   catch(BufferUnderflowException e){out.put("error","underflow");}
   out.put("position",raw.position());return out;
  }
  Player player=new Player();player.interfaces.count=arg(x,2);int a=arg(x,0),b=arg(x,1),d=arg(x,3),e=arg(x,4);int hi=a>>16,lo=a&65535;
  switch(kind){
   case "skill":{player.skills.level=b;player.skills.experience=Double.parseDouble((String)x.get("value"));new SkillLevel().send(new SkillContext(player,a));break;}
   case "energy":{player.settings.energy=Double.parseDouble((String)x.get("value"));new RunEnergy().send(new PlayerContext(player));break;}
   case "locAdd":case "locDel":{
    IoBuffer buffer=new IoBuffer(-1);Scenery obj=new Scenery(e,arg(x,2),d,a,b);
    if(kind.equals("locAdd"))ConstructScenery.write(buffer,obj);else ClearScenery.write(buffer,obj);
    player.session.write(buffer);break;
   }
   case "origin":player.session.write(UpdateAreaPosition.getBuffer(player,new Location(a,b)));break;
   case "enclosed":{IoBuffer buffer=UpdateAreaPosition.getChunkUpdateBuffer(player,new Location(a,b));Scenery obj=new Scenery(e,arg(x,2),d,a,b);ConstructScenery.write(buffer,obj);ClearScenery.write(buffer,obj);player.session.write(buffer);break;}
   case "open":new core.net.packet.out.Interface().send(new InterfaceContext(player,hi,lo,b,(d&1)!=0));break;
   case "pane":new WindowsPane().send(new WindowsPaneContext(player,a,b));break;
   case "close":new CloseInterface().send(new InterfaceContext(player,hi,lo,b,false));break;
   case "hidden":new InterfaceConfig().send(new InterfaceConfigContext(player,hi,lo,(b&1)!=0));break;
   case "text":new StringPacket().send(new StringContext(player,(String)x.get("text"),hi,lo));break;
   case "clear":new ContainerPacket().send(new ContainerContext(player,hi,lo,true));break;
   case "varc":new VarcUpdate().send(new VarcUpdateContext(player,a,b));break;
   case "varp":new Config().send(new ConfigContext(player,a,b));break;
   case "varbit":new Varbit().send(new VarbitContext(player,a,b));break;
   case "events":new AccessMask().send(new AccessMaskContext(player,e,lo,hi,b,d));break;
   case "script":{List values=(List)x.get("objects");Object[] objects=new Object[values.size()];for(int i=0;i<objects.length;i++){Map value=(Map)values.get(i);objects[i]=value.containsKey("text")?value.get("text"):Integer.valueOf(n(value.get("integer")));}new RunScriptPacket().send(new RunScriptContext(player,a,(String)x.get("types"),objects));break;}
   case "full":case "partial":{
    List values=(List)x.get("items");Item[] items=new Item[values.size()];for(int i=0;i<items.length;i++){if(values.get(i)!=null){Map m=(Map)values.get(i);items[i]=new Item(n(m.get("id")),n(m.get("amount")));}}
    ContainerContext context;if(kind.equals("full"))context=new ContainerContext(player,hi,lo,b,items,false);else{List slots=(List)x.get("slots");int[] selected=new int[slots.size()];for(int i=0;i<selected.length;i++)selected[i]=n(slots.get(i));context=new ContainerContext(player,hi,lo,b,items,false,selected);}
    new ContainerPacket().send(context);break;
   }
   default:throw new IllegalArgumentException(kind);
  }
  out.put("wire",unsigned(player.session.bytes));out.put("counter",player.interfaces.count);
  byte[] wire=player.session.bytes;int opcode=wire[0]&255;int header=(opcode==171||opcode==105||opcode==22||opcode==230||opcode==115)?3:1;
  out.put("fields",UpdateFields.decode(opcode,Arrays.copyOfRange(wire,header,wire.length)));return out;
 }
 public static void main(String[] args)throws Exception{
  try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null){JSONObject input=(JSONObject)new JSONParser().parse(line);input.put("expected",evaluate(input));System.out.println(JSONValue.toJSONString(input));}}
 }
}
