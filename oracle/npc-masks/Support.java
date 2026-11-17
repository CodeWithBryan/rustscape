package oracle;
import java.io.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.world.update.flag.*;import core.game.world.update.flag.context.*;
import core.game.node.entity.Entity;import core.game.node.entity.skill.Skills;import core.net.packet.IoBuffer;
/** Runs one NPCFlags530 provider's writeTo per case over the original IoBuffer. */
public class Support {
 static List<Object>events;
 public static void emit(Object...args){events.add(new ArrayList<>(Arrays.asList(args)));}
 static int n(Object o){return((Number)o).intValue();}
 static Entity entity(Map m){
  if(m==null)return null;
  Entity e="Player".equals(m.get("class"))?new core.game.node.entity.player.Player():new core.game.node.entity.npc.NPC();
  e.clientIndexValue=m.get("index")==null?0:n(m.get("index"));
  Skills s=new Skills();s.maximumValue=m.get("max")==null?0:n(m.get("max"));
  List life=(List)m.get("life");s.lifeValues=new int[life==null?0:life.size()];for(int i=0;i<s.lifeValues.length;i++)s.lifeValues[i]=n(life.get(i));
  e.skillsValue=s;return e;}
 static Object context(Map c){
  if(c==null)return null;
  switch((String)c.get("type")){
   case"HitMark":return new HitMark(n(c.get("damage")),n(c.get("kind")),entity((Map)c.get("entity")));
   case"Animation":return new Animation(n(c.get("id")),n(c.get("delay")));
   case"Graphics":return new Graphics(n(c.get("id")),n(c.get("height")),n(c.get("delay")));
   case"Entity":return entity(c);
   case"Integer":return Integer.valueOf(n(c.get("value")));
   case"String":return (String)c.get("value");
   case"Location":return new core.game.world.map.Location(n(c.get("x")),n(c.get("y")));
   case"Long":return Long.valueOf(((Number)c.get("value")).longValue());
   default:throw new IllegalArgumentException((String)c.get("type"));}}
 public static void main(String[]args)throws Exception{
  try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){
   String line;while((line=reader.readLine())!=null){
    Map input=(Map)new JSONParser().parse(line);Map out=new TreeMap(input);events=new ArrayList<>();
    EFlagProvider provider=(EFlagProvider)Class.forName("core.game.world.update.flag.NPCFlags530$"+input.get("provider")).getDeclaredConstructor().newInstance();
    IoBuffer buffer=new IoBuffer();String error=null;
    try{provider.writeTo(buffer,context((Map)input.get("context")));}
    catch(NullPointerException e){error="Null";}catch(ArithmeticException e){error="Arithmetic";}
    java.nio.ByteBuffer b=buffer.toByteBuffer();List<Object>bytes=new ArrayList<>();for(int i=0;i<b.position();i++)bytes.add(b.get(i)&255);
    Map<String,Object>expected=new TreeMap<>();expected.put("bytes",bytes);expected.put("events",events);expected.put("error",error);
    expected.put("presence",provider.getPresenceFlag());expected.put("ordinal",provider.getOrdinal());expected.put("flag",provider.getFlag().name());expected.put("type",provider.getType().name());expected.put("revision",provider.getRevision());
    out.put("expected",expected);System.out.println(JSONValue.toJSONString(out));}}}
}
