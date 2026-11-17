import core.game.node.entity.impl.GameAttributes;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import core.game.node.entity.player.info.login.AttributeParser;
import core.game.node.entity.player.info.login.AttributeSaver;
import core.api.ContentAPIKt;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import java.io.*;
import java.nio.file.*;
import java.nio.charset.Charset;
import java.util.*;
public class AttributeOracle {
 interface Work {Object run()throws Exception;}
 static Map<String,Object> map(Object... e){Map<String,Object> out=new TreeMap<>();for(int i=0;i<e.length;i+=2)out.put((String)e[i],e[i+1]);return out;}
 static String str(Map<?,?> m,String k){return (String)m.get(k);}
 static Object value(Map<?,?> object){String kind=str(object,"kind");Object value=object.get("value");switch(kind){case "int":return ((Number)value).intValue();case "long":return ((Number)value).longValue();case "short":return ((Number)value).shortValue();case "byte":return ((Number)value).byteValue();case "bool":case "str":return value;case "location":List<?> c=(List<?>)value;return new Location(((Number)c.get(0)).intValue(),((Number)c.get(1)).intValue(),((Number)c.get(2)).intValue());case "object":return new StringBuilder((String)value);case "null":return null;default:throw new IllegalArgumentException();}}
 static Object snapshotValue(Object value){if(value==null)return null;String kind=value instanceof Integer?"int":value instanceof Long?"long":value instanceof Short?"short":value instanceof Byte?"byte":value instanceof Boolean?"bool":value instanceof String?"str":value instanceof Location?"location":"object";return map("kind",kind,"value",value instanceof Location||kind.equals("object")?value.toString():value);}
 static Object snapshot(GameAttributes attrs){List<String> keys=new ArrayList<>(attrs.getAttributes().keySet());keys.sort(Comparator.nullsFirst(Comparator.naturalOrder()));List<Object> values=new ArrayList<>();for(String key:keys)values.add(Arrays.asList(key,snapshotValue(attrs.getAttributes().get(key))));keys=new ArrayList<>(attrs.keyExpirations.keySet());keys.sort(Comparator.nullsFirst(Comparator.naturalOrder()));List<Object> expirations=new ArrayList<>();for(String key:keys)expirations.add(Arrays.asList(key,attrs.keyExpirations.get(key)));return map("values",values,"saved",new ArrayList<>(attrs.getSavedAttributes()),"expirations",expirations);}
 static Object attempt(Work work){try{return map("ok",work.run());}catch(Exception e){return map("error",e instanceof NumberFormatException?"number":e instanceof IndexOutOfBoundsException?"index":e instanceof NullPointerException?"null":e instanceof ClassCastException?"type":e instanceof IllegalArgumentException?"base64":e.getClass().getSimpleName());}}
 static Object evaluate(JSONObject input,Path file)throws Exception {
  Player player=new Player();GameAttributes attrs=player.getGameAttributes();oracle.Clock.value=((Number)input.get("now")).longValue();oracle.Clock.step=((Number)input.get("step")).longValue();oracle.Clock.reads=0;Files.deleteIfExists(file);List<Object> output=new ArrayList<>();
  for(Object item:(List<?>)input.get("actions")){Map<?,?> a=(Map<?,?>)item;JSONObject root=new JSONObject();ContentAPIKt.logs.clear();ByteArrayOutputStream diagnostics=new ByteArrayOutputStream();PrintStream previous=java.lang.System.err;Object result;
   java.lang.System.setErr(new PrintStream(diagnostics,true,"UTF-8"));try{result=attempt(()->{switch(str(a,"op")){
    case "set":attrs.setAttribute(str(a,"key"),value((Map<?,?>)a.get("value")));break;
    case "raw":attrs.getAttributes().put(str(a,"key"),value((Map<?,?>)a.get("value")));break;
    case "saved":attrs.getSavedAttributes().add(str(a,"key"));break;
    case "expiry":attrs.keyExpirations.put(str(a,"key"),a.get("value")==null?null:((Number)a.get("value")).longValue());break;
    case "get":return snapshotValue(attrs.getAttribute(str(a,"key")));
    case "get_or":return snapshotValue(attrs.getAttribute(str(a,"key"),value((Map<?,?>)a.get("value"))));
    case "remove":attrs.removeAttribute(str(a,"key"));break;
    case "save":new AttributeSaver(player).saveAttributes(root);break;
    case "json":AttributeParser parser=new AttributeParser(player);parser.setSaveFile((JSONObject)a.get("root"));parser.parseAttributes();break;
    case "json_roundtrip":JSONObject saved=new JSONObject();new AttributeSaver(player).saveAttributes(saved);AttributeParser again=new AttributeParser(player);again.setSaveFile(saved);again.parseAttributes();break;
    case "xml":byte[] bytes=((String)a.get("xml")).getBytes(Charset.forName(str(a,"encoding")));StringBuilder hex=new StringBuilder();for(byte b:bytes)hex.append(String.format("%02x",b&255));((Map)a).put("bytes",hex.toString());Files.write(file,bytes);attrs.parse("player.xml");break;
    case "dump":attrs.dump("unused.xml");break;
    case "alias_location":Location loc=attrs.getAttribute(str(a,"key"));List<?> coords=(List<?>)a.get("value");loc.setX(((Number)coords.get(0)).intValue());loc.setY(((Number)coords.get(1)).intValue());loc.setZ(((Number)coords.get(2)).intValue());break;
    default:throw new IllegalArgumentException();}return null;});}finally{java.lang.System.setErr(previous);}
   output.add(map("result",result,"state",snapshot(attrs),"root",root,"warnings",new ArrayList<>(ContentAPIKt.logs),"legacyError",diagnostics.size()>0,"clockReads",oracle.Clock.reads,"clock",oracle.Clock.value));
  }return output;
 }
 public static void main(String[] args)throws Exception{Path folder=Paths.get(args[0]);core.ServerConstants.PLAYER_ATTRIBUTE_PATH=folder.toString()+File.separator;BufferedReader reader=new BufferedReader(new InputStreamReader(java.lang.System.in,"UTF-8"));String line;while((line=reader.readLine())!=null){JSONObject input=(JSONObject)new JSONParser().parse(line);input.put("expected",evaluate(input,folder.resolve("player.xml")));java.lang.System.out.println(JSONValue.toJSONString(input));}}
}
