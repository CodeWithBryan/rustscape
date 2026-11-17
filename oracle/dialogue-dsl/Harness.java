// Scenario driver for the dialogue DSL oracle. generate.py splits this bundle
// at each "//// path" marker. Reflection only observes private fields.
//// oracle/Describe.java
package oracle;
import java.util.*;
import core.game.dialogue.*;import core.game.event.*;
public final class Describe {
 public static Map<Object,String> seen=new IdentityHashMap<>();public static Map<String,Integer> counts=new HashMap<>();public static List<DialoguePlugin> seenPlugins=new ArrayList<>();
 public static void reset(){seen=new IdentityHashMap<>();counts=new HashMap<>();seenPlugins=new ArrayList<>();}
 public static Object value(Object v){
  if(v==null||v instanceof Integer||v instanceof Boolean)return v;
  if(v instanceof String)return Arrays.asList("s",Rec.units((String)v));
  if(v instanceof kotlin.Function)return "fn";
  if(v instanceof Enum)return ((Enum)v).name();
  if(v instanceof SkillDialogueHandler)return Oracle.handlerLabel(v);
  return v.getClass().getSimpleName();
 }
 public static Object plugin(DialoguePlugin p){
  if(p==null)return null;
  String s=seen.get(p);
  if(s==null){String c=p.getClass().getSimpleName();s=c+"#"+counts.merge(c,1,Integer::sum);seen.put(p,s);seenPlugins.add(p);}
  return s;
 }
 public static Object file(DialogueFile f){if(f==null)return null;int i=Oracle.files.indexOf(f);return i<0?f.getClass().getSimpleName():"F"+i;}
 public static Object event(Event e){
  if(e instanceof DialogueOpenEvent)return Arrays.asList("open",plugin(((DialogueOpenEvent)e).getDialogue()));
  if(e instanceof DialogueCloseEvent)return Arrays.asList("close",plugin(((DialogueCloseEvent)e).getDialogue()));
  DialogueOptionSelectionEvent s=(DialogueOptionSelectionEvent)e;Object d=s.getDialogue();
  return Arrays.asList("select",d instanceof DialogueFile?file((DialogueFile)d):plugin((DialoguePlugin)d),s.getCurrentStage(),s.getOptionId());
 }
}
//// oracle/Oracle.java
package oracle;
import java.io.*;import java.lang.reflect.Field;import java.util.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import kotlin.jvm.functions.Function1;
import core.game.dialogue.*;import core.game.component.*;import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;import core.game.node.entity.player.link.*;import core.game.node.item.Item;
import core.cache.def.impl.*;import core.game.event.*;
public class Oracle {
 public static Player player;public static PacketDispatch dispatch;public static InterfaceManager manager;public static DialogueInterpreter interp;
 public static List<NPC> npcs;public static List<DialogueFile> files;public static List<SkillDialogueHandler> handlers;
 static Map world;static Map<String,Object> bank;
 public static String text(Object raw){if(raw==null)return null;if(raw instanceof String)return(String)raw;StringBuilder s=new StringBuilder();for(Object u:(List)raw)s.append((char)Rec.n(u));return s.toString();}
 public static boolean flag(String key){return Boolean.TRUE.equals(bank.get(key));}
 public static String text(String key){return (String)bank.get(key);}
 public static void toggle(String key){bank.put(key,!flag(key));}
 public static int inventory(int id){Map m=(Map)world.getOrDefault("inventory",Collections.emptyMap());Object v=m.get(String.valueOf(id));return v==null?0:Rec.n(v);}
 public static int quest(String name){Map m=(Map)world.getOrDefault("quests",Collections.emptyMap());Object v=m.get(name);return v==null?0:Rec.n(v);}
 public static Object handlerLabel(Object h){int i=handlers.indexOf(h);return i<0?"SkillDialogueHandler":"H"+i;}
 /** A fresh instance of file spec k, labelled by creation order. */
 public static DialogueFile newFile(int k){
  Map spec=(Map)((List)world.get("files")).get(k);String kind=(String)spec.get("kind");DialogueFile f;
  if("banker".equals(kind))f=new content.global.handlers.npc.BankerNPC.BankerDialogueLabellerFile();
  else if("labeller".equals(kind))f=new ScriptLabeller((List)spec.get("ops"));
  else{ScriptBuilder.pending=spec;try{f=new ScriptBuilder();}finally{ScriptBuilder.pending=null;}}
  files.add(f);return f;
 }
 static Object comp(Component c){return c==null?null:c.ref;}
 static NPC npc(Map a){return a.get("npc")==null?null:npcs.get(Rec.n(a.get("npc")));}
 static SkillDialogueHandler skill(Map a){
  Map spec=(Map)a.get("handler");String type=(String)spec.get("type");List items=(List)spec.getOrDefault("items",Collections.emptyList());
  Item[] its=new Item[items.size()];for(int i=0;i<its.length;i++)its[i]=Item.all.get(Rec.n(items.get(i)));
  SkillDialogueHandler h=new ScriptSkill(player,type==null?null:SkillDialogueHandler.SkillDialogue.valueOf(type),spec,"H"+handlers.size(),its);
  handlers.add(h);return h;
 }
 @SuppressWarnings("unchecked") static Object act(Map a)throws Throwable{
  switch((String)a.get("op")){
   case"talk":{NPC n=npc(a);DialogueFile f=newFile(Rec.n(a.get("file")));DialogueLabeller.Companion.open(player,f,n);return null;}
   case"open":{NPC n=npc(a);DialogueFile f=newFile(Rec.n(a.get("file")));if(n==null)core.api.ContentAPIKt.openDialogue(player,f);else core.api.ContentAPIKt.openDialogue(player,f,n);return null;}
   case"openKey":core.api.ContentAPIKt.openDialogue(player,Rec.n(a.get("key")));return null;
   case"openOther":core.api.ContentAPIKt.openDialogue(player,"other");return null;
   case"newFile":{try{newFile(Rec.n(a.get("file")));}finally{}return "F"+(files.size()-1);}
   case"handle":interp.handle(a.get("c")==null?0:Rec.n(a.get("c")),Rec.n(a.get("b")));return null;
   case"close":return interp.close();
   case"input":{Object fn=player.rawAttributes().get("runscript");if(!(fn instanceof Function1))return "none";Object v=a.get("v");((Function1<Object,Object>)fn).invoke(v instanceof Number?(Object)Rec.n(v):text(v));return null;}
   case"closeEvent":{Component c=manager.rawChatbox();CloseEvent e=c==null?null:c.rawCloseEvent();if(e==null)return "none";return e.close(player,c);}
   case"skill":{SkillDialogueHandler h=skill(a);if("api".equals(a.get("via")))core.api.ContentAPIKt.openDialogue(player,h);else h.open();return handlerLabel(h);}
   case"skillTable":{Map spec=new HashMap();spec.put("type",a.get("type"));spec.put("items",a.get("items"));Map b=new HashMap();b.put("handler",spec);SkillDialogueHandler h=skill(b);
    List<Object> out=new ArrayList<>();for(int i=0;i<=26;i++){int amount=h.getType().getAmount(h,i);out.add(Arrays.asList(amount,h.getType().getIndex(h,i)));}return out;}
   case"forLength":{SkillDialogueHandler.SkillDialogue d=SkillDialogueHandler.SkillDialogue.Companion.forLength(Rec.n(a.get("n")));return d==null?null:d.name();}
   case"gem":return new ActivateEnchantedGem().handle(player,null,"activate");
   case"gemInit":{ActivateEnchantedGem g=new ActivateEnchantedGem();g.newInstance(null);return g.isWalk();}
   default:throw new AssertionError(a);
  }
 }
 static Object get(Class<?> c,Object o,String name){try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(o);}catch(ReflectiveOperationException e){throw new AssertionError(e);}}
 static Object node(DialogueNode n){
  String k=n.getClass().getSimpleName();List<Object> t=new ArrayList<>();Object attr=null;
  if(n instanceof PlaceholderNode)t.add(((PlaceholderNode)n).getTargetStage());
  else if(n instanceof GotoNode)t.add(((GotoNode)n).getNode().getTargetStage());
  else if(n instanceof OptionsNode||n instanceof OptionsDispatchNode){OptionsData d=n instanceof OptionsNode?((OptionsNode)n).getOptions():((OptionsDispatchNode)n).getOptions();for(OptionEntry e:d.getEntries())t.add(e.getNextStage());attr=d.getAttr();}
  else if(n instanceof BranchNode){TreeMap<Integer,Integer> m=new TreeMap<>(((BranchNode)n).getBranches().getBranches());for(Map.Entry<Integer,Integer> e:m.entrySet()){t.add(e.getKey());t.add(e.getValue());}}
  return Arrays.asList(k,t,attr);
 }
 static Object fileState(DialogueFile f){
  Map<String,Object> m=new TreeMap<>();
  m.put("base",get(DialogueFile.class,f,"stage"));
  m.put("player",f.getPlayer()==null?null:f.getPlayer().oid);m.put("npc",f.getNpc()==null?null:f.getNpc().oid);m.put("interp",f.getInterpreter()==null?null:f.getInterpreter()==interp?0:-1);
  if(f instanceof DialogueLabeller){DialogueLabeller l=(DialogueLabeller)f;
   m.put("stage",l.getStage());m.put("counter",l.getDialogueCounter());m.put("start",l.getStartingStage());m.put("hit",l.getStageHit());m.put("jump",l.getJumpTo());
   m.put("button",l.getButtonID());m.put("optButton",l.getOptButton());m.put("optInput",Describe.value(l.getOptInput()));
   List<Object> fo=new ArrayList<>();for(DialogueOption o:l.getFilteredOptions())fo.add(o.getGoto());m.put("filtered",fo);
   List<Object> ls=new ArrayList<>();for(Map.Entry<String,Integer> e:new TreeMap<>(l.getLabelStageMap()).entrySet())ls.add(Arrays.asList(e.getKey(),e.getValue()));m.put("labels",ls);
  }else{List<Object> cs=new ArrayList<>();for(DialogueClause c:((DialogueBuilderFile)f).getData()){List<Object> ns=new ArrayList<>();for(DialogueNode n:c.getNodes())ns.add(node(n));cs.add(ns);}m.put("clauses",cs);}
  return m;
 }
 static String error(Throwable e){return e instanceof Rec.Failure?"Binding":e instanceof NullPointerException?"Null":e instanceof IndexOutOfBoundsException?"Index":e instanceof ClassCastException?"Cast":e instanceof NumberFormatException?"Number":e instanceof IllegalArgumentException?"IllegalArgument":e.getClass().getSimpleName();}
 static Object snapshot(Object result,Throwable e,int mark){
  Map<String,Object> out=new TreeMap<>();
  out.put("error",e==null?null:error(e));
  out.put("result",result);
  out.put("interp",Arrays.asList(Describe.plugin(interp.getDialogue()),get(DialogueInterpreter.class,interp,"key"),interp.activeTopics.size()));
  List<Object> fs=new ArrayList<>();for(DialogueFile f:files)fs.add(fileState(f));out.put("files",fs);
  List<Object> ps=new ArrayList<>();for(int i=0;i<Describe.seenPlugins.size();i++){DialoguePlugin p=Describe.seenPlugins.get(i);ps.add(Arrays.asList(Describe.plugin(p),p.stage,get(DialoguePlugin.class,p,"finished"),Describe.file(p.file)));}
  out.put("plugins",ps);
  Map<String,Object> at=new TreeMap<>();for(Map.Entry<String,Object> en:player.rawAttributes().entrySet())at.put(en.getKey(),Describe.value(en.getValue()));out.put("attrs",at);
  out.put("chat",comp(manager.rawChatbox()));
  List<Object> cs=new ArrayList<>();for(Component c:Component.all)cs.add(Arrays.asList(c.rawId(),c.rawCloseEvent()!=null));out.put("comps",cs);
  List<EventHook> hooks=player.rawHooks().get(DialogueCloseEvent.class);out.put("hooks",hooks==null?0:hooks.size());
  out.put("bank",Arrays.asList(flag("secondary")));
  out.put("events",new ArrayList<>(Rec.events.subList(mark,Rec.events.size())));
  return out;
 }
 @SuppressWarnings("unchecked") static void world(Map s){
  Rec.events=new ArrayList<>();Rec.count=0;Rec.failAt=0;Describe.reset();Component.all=new ArrayList<>();Item.all=new ArrayList<>();ItemDefinition.all=new TreeMap<>();
  ((Map)get(DialogueInterpreter.class,null,"PLUGINS")).clear();
  world=s;bank=new HashMap<>((Map)s.getOrDefault("bank",Collections.emptyMap()));
  core.ServerConstants.SECOND_BANK=Boolean.TRUE.equals(bank.get("secondBank"));
  dispatch=new PacketDispatch(0);manager=new InterfaceManager(0);player=new Player(0);interp=new DialogueInterpreter(player);player.raw(dispatch,manager,interp);
  if(Boolean.FALSE.equals(s.get("male")))player.rawMale(false);
  if(Boolean.TRUE.equals(s.get("noLocation")))player.rawLocation=null;
  npcs=new ArrayList<>();List ns=(List)s.get("npcs");if(ns==null)ns=Arrays.asList(494L);for(Object id:ns)npcs.add(new NPC(npcs.size(),Rec.n(id)));
  for(Object it:(List)s.getOrDefault("items",Collections.emptyList())){List l=(List)it;Item item=new Item(Rec.n(l.get(0)),Rec.n(l.get(1)));if(l.size()>2)item.rawName=(String)l.get(2);}
  for(Map.Entry<Object,Object> e:((Map<Object,Object>)s.getOrDefault("attrs",Collections.emptyMap())).entrySet()){Object v=e.getValue();player.rawAttributes().put((String)e.getKey(),v instanceof Number?(Object)Rec.n(v):v);}
  files=new ArrayList<>();handlers=new ArrayList<>();
  if(Boolean.TRUE.equals(s.get("skill")))new SkillDialoguePlugin().init();
  Rec.events=new ArrayList<>();Rec.count=0;
 }
 public static void main(String[] args)throws Exception{
  PrintStream out=new PrintStream(new FileOutputStream(FileDescriptor.out),false,"UTF-8");
  System.setErr(new PrintStream(new OutputStream(){public void write(int b){}}){
   @Override public void println(Object x){if(x instanceof Throwable)Rec.emit("trace",error((Throwable)x));}
   @Override public void println(String x){Rec.emit("stderr",Rec.units(x));}
  });
  System.setOut(new PrintStream(new OutputStream(){public void write(int b){}}){
   @Override public PrintStream printf(String format,Object...a){Rec.emit("stdout",Rec.units(format));return this;}
  });
  BufferedReader r=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));String line;
  while((line=r.readLine())!=null){
   Map input=(Map)new JSONParser().parse(line);world(input);
   List<Object> expected=new ArrayList<>();List actions=(List)input.get("actions");
   for(int i=0;i<actions.size();i++){Map a=(Map)actions.get(i);int mark=Rec.events.size();Rec.count=0;Rec.failAt=a.get("failAt")==null?0:Rec.n(a.get("failAt"));
    Throwable error=null;Object result=null;try{result=act(a);}catch(Throwable e){if(e instanceof AssertionError)throw (AssertionError)e;error=e;}Rec.failAt=0;expected.add(snapshot(result,error,mark));}
   input.put("expected",expected);out.println(JSONValue.toJSONString(input));
  }
  out.flush();
 }
}
