package oracle;
import java.util.*;import java.io.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
public class Oracle {
 static Map input;static List<Object>events;static Map<String,Integer>calls;static Set<Integer>hooks;
 static Player[]players;static Dispatch[]dispatches;static Manager[]managers;static Interpreter[]interpreters;static List<Component>components;static Plugin[]plugins;static Action[]actions;static boolean listener;static String[]currentLines;
 static class Failure extends RuntimeException{}
 static int n(Object o){return((Number)o).intValue();}static int v(Map a,String k,int d){return a.get(k)==null?d:n(a.get(k));}
 static Object obj(Object...v){Map out=new TreeMap();for(int i=0;i<v.length;i+=2)out.put(v[i],v[i+1]);return out;}
 public static Object units(String s){if(s==null)return null;List<Integer>out=new ArrayList<>();for(char c:s.toCharArray())out.add((int)c);return out;}
 static String text(Object raw){if(raw==null)return null;if(raw instanceof String)return(String)raw;StringBuilder s=new StringBuilder();for(Object u:(List)raw)s.append((char)n(u));return s.toString();}
 public static void emit(String phase,Object...args){List row=new ArrayList();row.add(phase);row.addAll(Arrays.asList(args));events.add(row);int at=calls.merge(phase,1,Integer::sum);List hs=(List)input.getOrDefault("hooks",Collections.emptyList());for(int i=0;i<hs.size();i++){Map h=(Map)hs.get(i);if(phase.equals(h.get("event"))&&at==v(h,"at",1)&&hooks.add(i))for(Object a:(List)h.get("actions"))act((Map)a);}Map fails=(Map)input.getOrDefault("failures",Collections.emptyMap());if(fails.containsKey(phase)&&n(fails.get(phase))==at)throw new Failure();}
 public static class Player {
  public final int id;private Dispatch dispatch;private Manager manager;private Interpreter interpreter;
  Player(int id){this.id=id;}public Dispatch getPacketDispatch(){emit("dispatch",id);return dispatch;}public Manager getInterfaceManager(){emit("manager",id);return manager;}public Interpreter getDialogueInterpreter(){emit("interpreter",id);return interpreter;}public void debug(String s){emit("debug",id,s);}
 }
 public static class Dispatch{public final int id;Dispatch(int i){id=i;}public void sendString(String s,int iface,int child){emit("text",id,units(s),iface,child);}public void sendInterfaceConfig(int iface,int child,boolean hidden){emit("hidden",id,iface,child,hidden);}}
 public static class Component{public final int ref;private int id;private Plugin plugin;Component(int id){this.id=id;this.ref=components.size();components.add(this);}public int getId(){emit("id",ref);return id;}public Plugin getPlugin(){emit("plugin",ref);return plugin;}}
 public static class Manager{public final int id;private Component chatbox;public boolean veto;private Map<Integer,Component>lookup=new TreeMap<>();Manager(int i){id=i;}public void openChatbox(int iface){emit("open",id,iface);chatbox=new Component(iface);}public void closeChatbox(){emit("closeChatbox",id);if(!veto)chatbox=components.get(0);}public Component getChatbox(){emit("chatbox",id);return chatbox;}public Component getComponent(int iface){emit("component",id,iface);if(chatbox!=null&&chatbox.id==iface)return chatbox;return lookup.get(iface);}}
 public static class Action{public final int id;Action(int i){id=i;}public void handle(Player p,int child){emit("action",id,p.id,child);}}
 public static class Plugin{public final int id;Plugin(int i){id=i;}public boolean handle(Player p,Component c,int op,int child,int slot,int item){emit("pluginHandle",id,p.id,c.ref,op,child,slot,item);return false;}}
 public static boolean run(Player p,Component c,int op,int child,int slot,int item){emit("listener",p.id,c.ref,op,child,slot,item);return listener;}
 static Object act(Map a){int p=v(a,"player",0);switch((String)a.get("op")){
 case"send":Bridge.send(players[p],text(a.get("text")));return null;
 case"lines":Object raw=a.get("lines");String[]lines=null;if(raw!=null){List values=(List)raw;lines=new String[values.size()];for(int i=0;i<lines.length;i++)lines[i]=text(values.get(i));}currentLines=lines;Component c=interpreters[v(a,"interpreter",0)].sendDialogue(lines);return c==null?null:c.ref;
 case"option":Bridge.option(new Packet.ContinueOption(players[p],v(a,"iface",210),v(a,"child",1),a.get("slot")==null?-1:n(a.get("slot")),v(a,"opcode",132)));return null;
 case"field":Object value=a.get("value");int id=value instanceof Number?n(value):0;switch((String)a.get("field")){
  case"dispatch":players[p].dispatch=value==null?null:dispatches[id];break;case"manager":players[p].manager=value==null?null:managers[id];break;case"interpreter":players[p].interpreter=value==null?null:interpreters[id];break;
  case"chatbox":managers[v(a,"manager",0)].chatbox=value==null?null:components.get(id);break;case"id":components.get(v(a,"component",1)).id=id;break;case"plugin":components.get(v(a,"component",1)).plugin=value==null?null:plugins[id];break;
  case"active":interpreters[v(a,"interpreter",0)].active=value;break;case"listener":listener=(Boolean)value;break;case"veto":managers[v(a,"manager",0)].veto=(Boolean)value;break;
  case"lookup":if(value==null)managers[v(a,"manager",0)].lookup.remove(v(a,"iface",210));else managers[v(a,"manager",0)].lookup.put(v(a,"iface",210),components.get(id));break;
 }break;
 case"line":currentLines[v(a,"index",0)]=text(a.get("value"));break;
 case"replaceArray":List replacement=(List)a.get("values");currentLines=new String[replacement.size()];for(int i=0;i<currentLines.length;i++)currentLines[i]=text(replacement.get(i));break;
 case"queue":List<Action>q=interpreters[v(a,"interpreter",0)].rawActions();q.clear();for(Object queued:(List)a.get("values"))q.add(queued==null?null:actions[n(queued)]);break;
 default:throw new AssertionError(a);
 }return null;}
 static Object snapshot(Object result,String error){List ps=new ArrayList(),ms=new ArrayList(),cs=new ArrayList(),qs=new ArrayList();for(Player p:players)ps.add(Arrays.asList(p.dispatch==null?null:p.dispatch.id,p.manager==null?null:p.manager.id,p.interpreter==null?null:p.interpreter.id));for(Manager m:managers)ms.add(m.chatbox==null?null:m.chatbox.ref);for(Component c:components)cs.add(Arrays.asList(c.id,c.plugin==null?null:c.plugin.id));for(Interpreter d:interpreters){List q=new ArrayList();for(Action a:d.rawActions())q.add(a==null?null:a.id);qs.add(q);}return obj("error",error,"result",result,"events",new ArrayList(events),"players",ps,"chatboxes",ms,"components",cs,"queues",qs);}
 static String error(Throwable e){return e instanceof Failure?"Binding":e instanceof NullPointerException?"Null":e instanceof IndexOutOfBoundsException?"Index":e.getClass().getSimpleName();}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));String line;while((line=r.readLine())!=null){input=(Map)new JSONParser().parse(line);
  if(input.containsKey("split")){Map q=(Map)input.get("split");Object out=null;String error=null;try{String[]parts=Bridge.split(text(q.get("text")),v(q,"limit",54));List rows=new ArrayList();for(String s:parts)rows.add(units(s));out=rows;}catch(Throwable e){error=error(e);}input.put("expected",obj("error",error,"lines",out));System.out.println(JSONValue.toJSONString(input));continue;}
  events=new ArrayList<>();calls=new HashMap<>();hooks=new HashSet<>();players=new Player[3];dispatches=new Dispatch[3];managers=new Manager[3];interpreters=new Interpreter[4];components=new ArrayList<>();plugins=new Plugin[]{new Plugin(0),new Plugin(1)};actions=new Action[]{new Action(0),new Action(1)};listener=false;currentLines=null;for(int id:new int[]{137,210,211,212,213,762,763})new Component(id);
  for(int i=0;i<3;i++){players[i]=new Player(i);dispatches[i]=new Dispatch(i);managers[i]=new Manager(i);interpreters[i]=new Interpreter(i,players[i]);players[i].dispatch=dispatches[i];players[i].manager=managers[i];players[i].interpreter=interpreters[i];managers[i].chatbox=components.get(0);}interpreters[3]=new Interpreter(3,null);
  List expected=new ArrayList();for(Object a:(List)input.get("actions")){String error=null;Object result=null;try{result=act((Map)a);}catch(Throwable e){error=error(e);}expected.add(snapshot(result,error));}input.put("expected",expected);System.out.println(JSONValue.toJSONString(input));}}
}
