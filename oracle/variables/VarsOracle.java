package oracle;
import java.io.*;import java.util.*;import java.nio.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.entity.player.*;import core.cache.def.impl.VarbitDefinition;import core.net.packet.*;import core.net.packet.context.*;import core.net.packet.out.*;import core.api.ContentAPIKt;import core.game.system.config.CustomVarbitParser;
public class VarsOracle {
 static File ownedRoot;static int loadCount;
 public static Map row;public static Player[]players;static VarpManager manager;static CustomVarbitParser parser;
 static List<Object>log,frames;static Map<String,Integer>counts;static Map<String,VarbitDefinition>aliases;static List<HashMap<Integer,Integer>>heldValues;static List<HashMap<Integer,Boolean>>heldSaved;static List<FileReader>readers;
 public static int n(Map m,String key,int d){Object v=m.get(key);return v instanceof Number?((Number)v).intValue():d;}
 public static boolean flag(String key){return Boolean.TRUE.equals(row.get(key));}
 public static boolean nil(String key){return key.equals(row.get("null"));}
 public static void hit(String label,Object...args){List<Object>event=new ArrayList<>();event.add(label);event.addAll(Arrays.asList(args));log.add(event);int count=counts.merge(label,1,Integer::sum);
  for(Object value:(List)row.get("hooks")){Map hook=(Map)value;if(!label.equals(hook.get("when"))||n(hook,"at",1)!=count)continue;
   switch((String)hook.get("action")){
    case"fail":throw new IllegalStateException("controlled");
    case"put":players[0].varpMap.put(n(hook,"id",0),n(hook,"value",7));break;
    case"clear":players[0].varpMap.clear();break;
    case"replace":players[0].varpMap=new HashMap<>();players[0].varpMap.put(n(hook,"id",9),n(hook,"value",7));break;
    case"savedNull":players[0].saveVarp=null;break;
    case"savedReplace":players[0].saveVarp=new HashMap<>();break;
    case"create":VarbitDefinition.create(n(hook,"varp",2),n(hook,"id",7),n(hook,"start",2),n(hook,"end",4));break;
    case"counter":players[0].counter=n(hook,"value",2147483647);break;
    default:throw new AssertionError(hook);
   }
  }
 }
 public static byte[] file(int group,int id){hit("file",group,id);Map files=(Map)row.get("files");Object raw=files.get(""+((group<<10)|id));return raw==null?null:hex((String)raw);}
 static byte[] hex(String s){byte[]b=new byte[s.length()/2];for(int i=0;i<b.length;i++)b[i]=(byte)Integer.parseInt(s.substring(i*2,i*2+2),16);return b;}
 public static void packet(Class<?>type,Object context){
  if(context instanceof ConfigContext){ConfigContext c=(ConfigContext)context;hit("packetVarp",c.getPlayer().id,c.getId(),c.getValue());new Config().send(c);}
  else{VarcUpdateContext c=(VarcUpdateContext)context;hit("packetVarc",c.getPlayer().id,c.varcId,c.value);new VarcUpdate().send(c);}
 }
 public static void frame(IoBuffer b){ByteBuffer buf=b.toByteBuffer();buf.flip();StringBuilder s=new StringBuilder();while(buf.hasRemaining())s.append(String.format("%02x",buf.get()&255));frames.add(Arrays.asList(b.opcode()&255,s.toString()));}
 static int tag(Object o,IdentityHashMap<Object,Integer>ids){return ids.computeIfAbsent(o,x->ids.size());}
 static Object def(VarbitDefinition d,IdentityHashMap<Object,Integer>ids){return d==null?null:Arrays.asList(tag(d,ids),d.getId(),d.getVarpId(),d.getStartBit(),d.getEndBit(),d.toString());}
 static Object map(Map<?,?>m,IdentityHashMap<Object,Integer>ids){if(m==null)return null;Map<String,Object>values=new TreeMap<>();m.forEach((k,v)->values.put(k.toString(),v));return Map.of("ref",tag(m,ids),"values",values);}
 static Object state(){IdentityHashMap<Object,Integer>ids=new IdentityHashMap<>();List<Object>values=new ArrayList<>(),saved=new ArrayList<>(),hv=new ArrayList<>(),hs=new ArrayList<>();Map<String,Object>mapping=new TreeMap<>(),refs=new TreeMap<>();
  for(Player p:players){values.add(map(p.varpMap,ids));saved.add(map(p.saveVarp,ids));}
  Map<Integer,VarbitDefinition>sorted=new TreeMap<>(VarbitDefinition.getMapping());for(Map.Entry<Integer,VarbitDefinition>e:sorted.entrySet())mapping.put(""+e.getKey(),def(e.getValue(),ids));for(Map.Entry<String,VarbitDefinition>e:aliases.entrySet())refs.put(e.getKey(),def(e.getValue(),ids));
  for(Map m:heldValues)hv.add(map(m,ids));for(Map m:heldSaved)hs.add(map(m,ids));List<Object>rs=new ArrayList<>();for(FileReader r:readers)rs.add(r==null?null:tag(r,ids));
  Map<String,Object>out=new TreeMap<>();out.put("values",values);out.put("saved",saved);out.put("mapping",mapping);out.put("aliases",refs);out.put("heldValues",hv);out.put("heldSaved",hs);out.put("counters",Arrays.asList(players[0].counter,players[1].counter));out.put("reader",parser.getReader()==null?null:tag(parser.getReader(),ids));out.put("readers",rs);return out;
 }
 static VarbitDefinition definition(Map a){return aliases.get((String)a.getOrDefault("alias","a"));}
 static Object action(Map a)throws Exception{String op=(String)a.get("op");int player=n(a,"player",0);Player p=player<0?null:players[player];int id=n(a,"id",0),value=n(a,"value",0);boolean save=Boolean.TRUE.equals(a.get("save"));
  switch(op){
   case"get":return ContentAPIKt.getVarp(p,id);case"set":ContentAPIKt.setVarp(p,id,value,save);break;case"saveFlag":ContentAPIKt.saveVarp(p,id);break;case"unsave":ContentAPIKt.unsaveVarp(p,id);break;case"varc":ContentAPIKt.setVarc(p,id,value);break;case"reinit":ContentAPIKt.reinitVarps(p);break;
   case"getBit":return ContentAPIKt.getVarbit(p,id);case"setBit":ContentAPIKt.setVarbit(p,id,value,save);break;case"getDef":return ContentAPIKt.getVarbit(p,definition(a));case"setDef":ContentAPIKt.setVarbit(p,definition(a),value,save);break;case"defValue":return definition(a).getValue(p);
   case"lookup":{VarbitDefinition d;switch(n(a,"kind",0)){case 1:d=VarbitDefinition.forObjectID(id);break;case 2:d=VarbitDefinition.forNPCID(id);break;case 3:d=VarbitDefinition.forItemID(id);break;default:d=VarbitDefinition.forId(id);}aliases.put((String)a.getOrDefault("alias","a"),d);break;}
   case"create":VarbitDefinition.create(n(a,"varp",0),id,n(a,"start",0),n(a,"end",0));break;case"mapNull":VarbitDefinition.getMapping().put(id,null);break;case"mapAlias":VarbitDefinition.getMapping().put(id,definition(a));break;case"mask":return definition(a).getMask();
   case"parse":manager.parse((JSONArray)a.get("data"));break;case"noopSave":{JSONObject root=(JSONObject)a.get("data");manager.save(root);return root;}
   case"constructorNull":new VarpManager(null);break;
   case"clear":p.varpMap.clear();break;case"capture":heldValues.add(p.varpMap);heldSaved.add(p.saveVarp);break;
   case"rawValue":p.varpMap.put(id,a.get("value")==null?null:value);break;case"rawSaved":p.saveVarp.put(id,(Boolean)a.get("value"));break;
   case"valuesNull":p.varpMap=null;break;case"savedNull":p.saveVarp=null;break;case"replaceValues":p.varpMap=new HashMap<>();break;case"replaceSaved":p.saveVarp=new HashMap<>();break;
   case"load":{File directory=new File(ownedRoot,"load-"+(loadCount++));directory.mkdirs();core.ServerConstants.CONFIG_PATH=directory.getAbsolutePath()+File.separator;File target=new File(core.ServerConstants.CONFIG_PATH+"varbit_definitions.json");if(target.exists()&&!target.delete())throw new IOException("Cannot reset owned oracle file");if(!Boolean.TRUE.equals(a.get("missing"))){try(Writer w=new OutputStreamWriter(new FileOutputStream(target),java.nio.charset.StandardCharsets.UTF_8)){w.write((String)a.get("text"));}}parser.load();break;}
   case"reader":readers.add(parser.getReader());break;
   default:throw new AssertionError(op);
  }return null;
 }
 static Object portable(Object o){if(o==null)return null;if(o instanceof Map){Map<String,Object>m=new TreeMap<>();((Map<?,?>)o).forEach((k,v)->m.put(k.toString(),portable(v)));return m;}if(o instanceof Collection){List<Object>out=new ArrayList<>();for(Object x:(Collection)o)out.add(portable(x));return out;}return o;}
 static Object evaluate(Map input)throws Exception {
  row=input;players=new Player[]{new Player(0),new Player(1)};manager=new VarpManager(players[n(row,"managerOwner",0)]);parser=new CustomVarbitParser();aliases=new TreeMap<>();heldValues=new ArrayList<>();heldSaved=new ArrayList<>();readers=new ArrayList<>();log=new ArrayList<>();frames=new ArrayList<>();counts=new HashMap<>();VarbitDefinition.getMapping().clear();
  for(int i=0;i<2;i++){Map vs=(Map)((List)row.get("values")).get(i),ss=(Map)((List)row.get("saved")).get(i);for(Object k:vs.keySet())players[i].varpMap.put(Integer.parseInt(k.toString()),vs.get(k)==null?null:((Number)vs.get(k)).intValue());for(Object k:ss.keySet())players[i].saveVarp.put(Integer.parseInt(k.toString()),(Boolean)ss.get(k));}
  players[0].counter=n(row,"counter",0);List<Object>out=new ArrayList<>();
  for(Object entry:(List)row.get("actions")){Object result=null;String error=null;try{result=action((Map)entry);}catch(NullPointerException e){error="Null";}catch(ClassCastException e){error="Type";}catch(NumberFormatException e){error="Number";}catch(BufferUnderflowException e){error="Underflow";}catch(ConcurrentModificationException e){error="Concurrent";}catch(IOException e){error="Io";}catch(org.json.simple.parser.ParseException e){error="Json";}catch(IllegalStateException e){error="Binding";}out.add(Map.of("result",result==null?"__NULL__":portable(result),"error",error==null?"__NULL__":error,"state",portable(state()),"log",portable(log),"frames",portable(frames)));}
  return out;
 }
 public static void main(String[]args)throws Exception {ownedRoot=new File(args[0]);ownedRoot.mkdirs();core.ServerConstants.CONFIG_PATH=ownedRoot.getAbsolutePath()+File.separator;BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=r.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(row));}}
}
