package oracle;
import java.io.*;import java.util.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.entity.player.Player;import core.game.node.entity.player.link.BankPinManager;import core.game.node.entity.player.link.BankPinManager.PinStatus;
import core.game.component.*;import content.global.handlers.iface.bank.BankPinInterface;
public class PinOracle {
 public static Map row;public static Player[]players;public static Component[]current;public static List<BankPinManager>managers;public static List<Component>events;public static Map<Integer,Object>plugins;
 static List<Object>log;static Map<String,Integer>counts;public static BankPinInterface program;static long clock;static int randomCalls;static long clockCalls;
 public static int n(Map m,String key,int d){Object v=m.get(key);return v instanceof Number?((Number)v).intValue():d;}
 public static long l(Map m,String key,long d){Object v=m.get(key);return v instanceof Number?((Number)v).longValue():d;}
 public static boolean flag(String key){return Boolean.TRUE.equals(row.get(key));}
 public static boolean nil(String key){return key.equals(row.get("null"));}
 public static List<Integer>units(String s){if(s==null)return null;List<Integer>v=new ArrayList<>();for(char c:s.toCharArray())v.add((int)c);return v;}
 public static int mid(BankPinManager m){if(m==null)return-1;for(int i=0;i<managers.size();i++)if(managers.get(i)==m)return i;throw new AssertionError("untracked manager");}
 static Field field(String name){try{Field f=BankPinManager.class.getDeclaredField(name);f.setAccessible(true);return f;}catch(Exception e){throw new AssertionError(e);}}
 public static Object get(BankPinManager m,String name){try{return field(name).get(m);}catch(Exception e){throw new AssertionError(e);}}
 static void set(BankPinManager m,String key,Object value){try{Field f=field(key);if(f.getType()==int.class)f.setInt(m,((Number)value).intValue());else if(f.getType()==long.class)f.setLong(m,((Number)value).longValue());else if(f.getType()==boolean.class)f.setBoolean(m,(Boolean)value);else if(key.equals("status"))f.set(m,value==null?null:PinStatus.values()[((Number)value).intValue()]);else f.set(m,value);}catch(Exception e){throw new AssertionError(e);}}
 public static BankPinManager captured(CloseEvent event){for(Field f:event.getClass().getDeclaredFields())if(f.getType()==BankPinManager.class){try{f.setAccessible(true);return(BankPinManager)f.get(event);}catch(Exception e){throw new AssertionError(e);}}throw new AssertionError("no captured manager");}
 public static void closeCurrent(int id){Component c=current[id];if(c!=null&&c.close!=null)c.close.close(players[id],c);}
 public static void hit(String label,Object...args){List<Object>event=new ArrayList<>();event.add(label);event.addAll(Arrays.asList(args));log.add(event);int count=counts.merge(label,1,Integer::sum);
  for(Object value:(List)row.get("hooks")){Map hook=(Map)value;if(!label.equals(hook.get("when"))||n(hook,"at",1)!=count)continue;BankPinManager m=managers.get(n(hook,"target",0));switch((String)hook.get("action")){
   case"fail":throw new IllegalStateException("controlled");
   case"set":set(m,(String)hook.get("field"),hook.get("value"));break;
   case"cancel":m.doCancelPin();break;
   case"closeEvent":{Component c=events.get(n(hook,"event",0));c.close.close(players[1],c);break;}
   case"replaceManager":{int p=n(hook,"player",0);BankPinManager replacement=new BankPinManager(players[p]);managers.add(replacement);players[p].manager=replacement;break;}
   case"nullManager":players[n(hook,"player",0)].manager=null;break;
   case"reenter":m.openType(n(hook,"button",4));break;
   default:throw new AssertionError(hook);
  }}
 }
 public static long now(){List values=(List)row.get("times");long value=values.isEmpty()?clock:((Number)values.get((int)Math.min(clockCalls,values.size()-1))).longValue();clockCalls++;clock+=l(row,"clockStep",0);hit("clock",value);return value;}
 public static int random(int min,int max){hit("random",min,max);List values=(List)row.get("randoms");return values.isEmpty()?min:((Number)values.get((randomCalls++)%values.size())).intValue();}
 public static void event(Component c,CloseEvent e){BankPinManager m=captured(e);hit("closeEvent",c.id,mid(m));c.close=e;events.add(c);}
 static Object state(){List<Object>ms=new ArrayList<>(),es=new ArrayList<>(),cs=new ArrayList<>();for(BankPinManager m:managers){Map<String,Object>v=new TreeMap<>();Player p=m.getPlayer();v.put("player",p==null?null:p.id);for(String key:new String[]{"digits","unlocked","longRecovery","pendingDelay","openId","tries","changeState","deleting","tryDelay"})v.put(key,get(m,key));v.put("pin",units(m.getPin()));v.put("tempPin",units((String)get(m,"tempPin")));v.put("status",m.getStatus()==null?null:m.getStatus().ordinal());ms.add(v);}for(Component c:events)es.add(Arrays.asList(c.id,mid(captured(c.close))));for(Component c:current)cs.add(c==null?null:c.id);Map<String,Object>ps=new TreeMap<>();plugins.forEach((k,v)->ps.put(""+k,v==program?0:1));return Map.of("managers",ms,"playerManagers",Arrays.asList(mid(players[0].manager),mid(players[1].manager)),"events",es,"current",cs,"plugins",ps,"clockCalls",clockCalls,"randomCalls",randomCalls);}
 static String[]messages(Map a){if(a.get("messages")==null)return null;List in=(List)a.get("messages");String[]out=new String[in.size()];for(int i=0;i<out.length;i++)out[i]=(String)in.get(i);return out;}
 static Object action(Map a)throws Throwable {BankPinManager m=managers.get(n(a,"target",0));int value=n(a,"value",0);switch((String)a.get("op")){
  case"set":set(m,(String)a.get("field"),a.get("value"));break;
  case"parse":m.parse((JSONObject)a.get("data"));break;
  case"settings":m.openSettings();break;case"messages":m.openSettings(messages(a));break;
  case"type":m.openType(value);break;case"confirm":m.handleConfirmInterface(value);break;case"toggle":m.toggleConfirmInterface(Boolean.TRUE.equals(a.get("value")));break;
  case"pin":m.openPin();break;case"stage":m.handlePinStage(value);break;case"click":m.updateTempPin(value);break;
  case"digit":{List<Integer>digits=(List<Integer>)get(m,"digits");m.updateTempPin(digits.indexOf(value));break;}
  case"cancel":m.cancelPin(messages(a));break;case"doCancel":m.doCancelPin();break;
  case"switch":m.switchRecovery();break;case"login":m.drawLoginMessage();break;case"days":return m.getPendingDays();case"blocked":return m.isPinBlocked();
  case"getters":return Arrays.asList(m.getStage(),m.getRecoveryDelay(),m.hasPin(),m.hasPendingPin(),m.isUnlocked(),m.isLongRecovery(),m.getPinSeconds(),m.getTries(),m.getTryDelay(),m.getPendingDelay());
  case"statusMessages":{String[]v=m.getStatus().getMessages(m);List<Object>out=new ArrayList<>();for(String s:v)out.add(units(s));return out;}
  case"statusLogin":return units(m.getStatus().getLoginMessage(m));case"statusDraw":m.getStatus().draw(m,m.getPlayer());break;
  case"closeEvent":{Component c=events.get(value);return c.close.close(players[n(a,"player",1)],c);}
  case"replaceManager":{int p=n(a,"player",0);BankPinManager fresh=new BankPinManager(players[p]);managers.add(fresh);players[p].manager=fresh;break;}
  case"handle":{Player p=n(a,"player",0)<0?null:players[n(a,"player",0)];Component component=Boolean.TRUE.equals(a.get("nullComponent"))?null:new Component(n(a,"component",14));return program.handle(p,component,n(a,"opcode",0),value,n(a,"slot",0),n(a,"item",0));}
  case"register":return program.newInstance(null)==program;
  default:throw new AssertionError(a);
 }return null;}
 static Object portable(Object o){if(o==null)return null;if(o instanceof Map){Map<String,Object>m=new TreeMap<>();((Map<?,?>)o).forEach((k,v)->m.put(k.toString(),portable(v)));return m;}if(o instanceof Collection){List<Object>v=new ArrayList<>();for(Object x:(Collection)o)v.add(portable(x));return v;}return o;}
 static Object evaluate(Map input)throws Throwable {
  row=input;log=new ArrayList<>();counts=new HashMap<>();managers=new ArrayList<>();players=new Player[]{new Player(0),new Player(1)};for(Player p:players)managers.add(p.manager);if(flag("nullOwner")){BankPinManager m=new BankPinManager(null);managers.set(0,m);players[0].manager=m;}
  current=new Component[2];events=new ArrayList<>();plugins=new HashMap<>();program=new BankPinInterface();clock=l(row,"clock",1000000);clockCalls=0;randomCalls=0;
  Field rng=Collections.class.getDeclaredField("r");rng.setAccessible(true);rng.set(null,new Random(l(row,"seed",1)));
  Map init=(Map)row.get("initial");for(Object key:init.keySet())set(managers.get(0),key.toString(),init.get(key));List<Object>out=new ArrayList<>();
  for(Object value:(List)row.get("actions")){Object result=null;String error=null;try{result=action((Map)value);}catch(NullPointerException e){error="Null";}catch(ClassCastException e){error="Type";}catch(NumberFormatException e){error="Number";}catch(IndexOutOfBoundsException e){error="Index";}catch(IllegalStateException e){error="Binding";}out.add(Map.of("result",result==null?"__NULL__":portable(result),"error",error==null?"__NULL__":error,"state",portable(state()),"log",portable(log)));}
  return out;
 }
 public static void main(String[]args)throws Throwable {BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=r.readLine())!=null){Map row=(Map)new JSONParser().parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(row));}}
}
