// Scenario driver and scripted original-style content for the dialogue oracle.
// generate.py splits this bundle at each "//// path" marker. The scripted plugin
// and file classes extend the unmodified DialoguePlugin/DialogueFile and only
// call their inherited methods; reflection is used as a private-field observer
// and to empty the static PLUGINS registry between scenarios.
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
  return v.getClass().getSimpleName();
 }
 public static Object plugin(DialoguePlugin p){
  if(p==null)return null;
  if(p instanceof ScriptPlugin)return ((ScriptPlugin)p).label;
  String s=seen.get(p);
  if(s==null){String c=p.getClass().getSimpleName();s=c+"#"+counts.merge(c,1,Integer::sum);seen.put(p,s);seenPlugins.add(p);}
  return s;
 }
 public static Object file(DialogueFile f){
  if(f==null)return null;
  if(f instanceof ScriptFile)return ((ScriptFile)f).label;
  String s=seen.get(f);
  if(s==null){String c=f.getClass().getSimpleName();s=c+"#"+counts.merge(c,1,Integer::sum);seen.put(f,s);}
  return s;
 }
 public static Object event(Event e){
  if(e instanceof DialogueOpenEvent)return Arrays.asList("open",plugin(((DialogueOpenEvent)e).getDialogue()));
  if(e instanceof DialogueCloseEvent)return Arrays.asList("close",plugin(((DialogueCloseEvent)e).getDialogue()));
  DialogueOptionSelectionEvent s=(DialogueOptionSelectionEvent)e;Object d=s.getDialogue();
  return Arrays.asList("select",d instanceof DialogueFile?file((DialogueFile)d):plugin((DialoguePlugin)d),s.getCurrentStage(),s.getOptionId());
 }
}
//// oracle/Act.java
package oracle;
import core.game.dialogue.DialogueAction;import core.game.node.entity.player.Player;
public final class Act implements DialogueAction {
 public final int id;public Act(int id){this.id=id;}
 public void handle(Player player,int buttonId){Rec.emit("action",id,player==null?null:player.oid,buttonId);}
}
//// oracle/ScriptPlugin.java
package oracle;
import java.util.*;
import core.game.dialogue.*;import core.game.node.entity.player.Player;import core.game.node.entity.npc.NPC;import core.game.node.entity.Entity;
/** Scripted content written against the unmodified DialoguePlugin contract. */
public abstract class ScriptPlugin extends DialoguePlugin {
 public Map spec;public String label;
 protected ScriptPlugin(){super();}
 protected ScriptPlugin(Player player){super(player);}
 public ScriptPlugin bind(Map spec,String label){this.spec=spec;this.label=label;return this;}
 protected ScriptPlugin instance(ScriptPlugin made){made.bind(spec,"P"+Oracle.instances.size());Oracle.instances.add(made);return made;}
 protected abstract ScriptPlugin create(Player player);
 @Override public DialoguePlugin newInstance(Player player){
  Rec.emit("newInstance",label,player==null?null:player.oid);
  String mode=spec.get("mode")==null?"override":(String)spec.get("mode");
  switch(mode){
   case"null":return null;
   case"throw":throw new Rec.Failure();
   case"reflect":case"noctor":{int before=Oracle.reflected.size();DialoguePlugin made=super.newInstance(player);if(made instanceof ScriptPlugin&&Oracle.reflected.size()>before)instance((ScriptPlugin)made);return made;}
   default:return instance(create(player));
  }
 }
 @Override public int[] getIds(){Rec.emit("getIds",label);List ids=(List)spec.get("ids");int[] out=new int[ids.size()];for(int i=0;i<out.length;i++)out[i]=Rec.n(ids.get(i));return out;}
 @Override public boolean open(Object...args){
  Object open=spec.get("open");
  if(open==null)return super.open(args);
  Rec.emit("pluginOpen",label,args.length);
  if(args.length>0&&args[0] instanceof NPC)npc=(NPC)args[0];
  run((List)open,0,0);
  return !Boolean.FALSE.equals(spec.get("openRet"));
 }
 @Override public boolean close(){Rec.emit("pluginClose",label);if("veto".equals(spec.get("close")))return false;return super.close();}
 @Override public boolean handle(int interfaceId,int buttonId){
  Rec.emit("pluginHandle",label,interfaceId,buttonId,stage);
  Map script=(Map)spec.get("script");
  if(script==null)return true;
  Object ops=script.get(String.valueOf(stage));if(ops==null)ops=script.get("*");
  return ops==null||run((List)ops,interfaceId,buttonId);
 }
 public boolean run(List ops,int iface,int button){
  boolean ret=true;
  for(Object raw:ops){List o=(List)raw;String op=(String)o.get(0);
   switch(op){
    case"npc":npc(Oracle.lines(o.get(1)));break;
    case"npcId":npc(Rec.n(o.get(1)),Oracle.lines(o.get(2)));break;
    case"npcE":npc(Oracle.expr(o.get(1)),Oracle.lines(o.get(2)));break;
    case"player":player(Oracle.lines(o.get(1)));break;
    case"playerE":player(Oracle.expr(o.get(1)),Oracle.lines(o.get(2)));break;
    case"npcl":npcl(Oracle.expr(o.get(1)),Oracle.text(o.get(2)));break;
    case"playerl":playerl(Oracle.expr(o.get(1)),Oracle.text(o.get(2)));break;
    case"options":options(Oracle.lines(o.get(1)));break;
    case"line":sendDialogue(Oracle.lines(o.get(1)));break;
    case"normal":sendNormalDialogue(Oracle.entity(o.get(1)),Oracle.expr(o.get(2)),Oracle.lines(o.get(3)));break;
    case"stage":setStage(Rec.n(o.get(1)));break;
    case"next":next();break;
    case"inc":increment();break;
    case"getInc":Rec.emit("value",getAndIncrement());break;
    case"finish":finish();break;
    case"end":end();break;
    case"close":Rec.emit("value",close());break;
    case"loadFile":loadFile(Oracle.file(o.get(1)));break;
    case"addOption":addOption(Oracle.text(o.get(1)),Oracle.file(o.get(2)));break;
    case"choices":Rec.emit("value",sendChoices());break;
    case"topics":Rec.emit("value",showTopics(Oracle.topics(o.get(1))));break;
    case"pirate":Rec.emit("value",Rec.units(pirateGender()));break;
    case"finished":Rec.emit("value",isFinished());break;
    case"setFile":file=Oracle.file(o.get(1));break;
    case"setNpc":npc=o.get(1)==null?null:Oracle.npcs.get(Rec.n(o.get(1)));break;
    case"i":Oracle.act((Map)o.get(1));break;
    case"btn":if(button==Rec.n(o.get(1)))ret&=run((List)o.get(2),iface,button);break;
    case"fail":throw new Rec.Failure();
    case"ret":ret=(Boolean)o.get(1);break;
    default:throw new AssertionError(op);
   }
  }
  return ret;
 }
}
//// oracle/ScriptPluginA.java
package oracle;
import core.game.node.entity.player.Player;
public class ScriptPluginA extends ScriptPlugin {
 public ScriptPluginA(){super();}
 public ScriptPluginA(Player player){super(player);}
 protected ScriptPlugin create(Player player){return new ScriptPluginA(player);}
}
//// oracle/ScriptPluginB.java
package oracle;
import core.game.node.entity.player.Player;
public class ScriptPluginB extends ScriptPlugin {
 public ScriptPluginB(){super();}
 public ScriptPluginB(Player player){super(player);}
 protected ScriptPlugin create(Player player){return new ScriptPluginB(player);}
}
//// oracle/ReflectPlugin.java
package oracle;
import core.game.node.entity.player.Player;
/** Has the (Player) constructor the unmodified reflective newInstance needs. */
public class ReflectPlugin extends ScriptPlugin {
 public ReflectPlugin(){super();}
 public ReflectPlugin(Player player){super(player);Oracle.reflected.add(this);}
 protected ScriptPlugin create(Player player){return new ReflectPlugin(player);}
}
//// oracle/NoCtorPlugin.java
package oracle;
import core.game.node.entity.player.Player;
/** Lacks a (Player) constructor: the unmodified reflective newInstance fails. */
public class NoCtorPlugin extends ScriptPlugin {
 public NoCtorPlugin(){super();}
 protected ScriptPlugin create(Player player){return new NoCtorPlugin();}
}
//// oracle/ScriptFile.java
package oracle;
import java.util.*;
import core.game.dialogue.*;import core.game.node.entity.player.Player;import core.game.node.entity.npc.NPC;
/** Scripted content written against the unmodified DialogueFile contract. */
public class ScriptFile extends DialogueFile {
 public Map spec;public String label;public boolean shadow;public int shadowStage=-1;public Integer current;
 public ScriptFile(Map spec,String label){this.spec=spec;this.label=label;this.shadow=Boolean.TRUE.equals(spec.get("shadow"));if(spec.get("current")!=null)current=Rec.n(spec.get("current"));}
 public int baseStage(){return super.getStage();}
 @Override public int getStage(){return shadow?shadowStage:super.getStage();}
 @Override public void setStage(int v){if(shadow)shadowStage=v;else super.setStage(v);}
 @Override public int getCurrentStage(){return current!=null?current:super.getCurrentStage();}
 @Override public void handle(int componentID,int buttonID){
  Rec.emit("fileHandle",label,componentID,buttonID,getStage());
  Map script=(Map)spec.get("script");
  if(script==null)return;
  Object ops=script.get(String.valueOf(getStage()));if(ops==null)ops=script.get("*");
  if(ops!=null)run((List)ops,componentID,buttonID);
 }
 public void run(List ops,int iface,int button){
  for(Object raw:ops){List o=(List)raw;String op=(String)o.get(0);
   switch(op){
    case"npc":npc(Oracle.lines(o.get(1)));break;
    case"npcId":npc(Rec.n(o.get(1)),Oracle.lines(o.get(2)));break;
    case"npcE":npc(Oracle.expr(o.get(1)),Oracle.lines(o.get(2)));break;
    case"npcIdE":npc(Rec.n(o.get(1)),Oracle.expr(o.get(2)),Oracle.lines(o.get(3)));break;
    case"npcT":npc(Rec.n(o.get(1)),Oracle.text(o.get(2)),Oracle.expr(o.get(3)),Oracle.lines(o.get(4)));break;
    case"player":player(Oracle.lines(o.get(1)));break;
    case"playerE":player(Oracle.expr(o.get(1)),Oracle.lines(o.get(2)));break;
    case"npcl":npcl(Oracle.expr(o.get(1)),Oracle.text(o.get(2)));break;
    case"npclId":npcl(Rec.n(o.get(1)),Oracle.expr(o.get(2)),Oracle.text(o.get(3)));break;
    case"npclT":npcl(Rec.n(o.get(1)),Oracle.text(o.get(2)),Oracle.expr(o.get(3)),Oracle.text(o.get(4)));break;
    case"npcl1":npcl(Oracle.text(o.get(1)));break;
    case"playerl":playerl(Oracle.expr(o.get(1)),Oracle.text(o.get(2)));break;
    case"playerl1":playerl(Oracle.text(o.get(1)));break;
    case"options":Oracle.defaulted("options",new Class<?>[]{DialogueFile.class,String[].class,String.class,int.class,Object.class},this,Oracle.lines(o.get(1)),null,2,null);break;
    case"optionsT":options(Oracle.lines(o.get(1)),Oracle.text(o.get(2)));break;
    case"line":dialogue(Oracle.lines(o.get(1)));break;
    case"normal":sendNormalDialogue(Oracle.entity(o.get(1)),Oracle.expr(o.get(2)),Oracle.lines(o.get(3)));break;
    case"stage":setStage(Rec.n(o.get(1)));break;
    case"cur":Rec.emit("value",getCurrentStage());break;
    case"end":end();break;
    case"endFile":endFile();break;
    case"returnAt":returnAtStage(Rec.n(o.get(1)));break;
    case"abandon":abandonFile();break;
    case"topics":Rec.emit("value",Oracle.defaulted("showTopics",new Class<?>[]{DialogueFile.class,Topic[].class,String.class,int.class,Object.class},this,Oracle.topics(o.get(1)),null,2,null));break;
    case"topicsT":Rec.emit("value",showTopics(Oracle.topics(o.get(1)),Oracle.text(o.get(2))));break;
    case"load":{List a=(List)o;load(a.get(1)==null?null:Oracle.players[Rec.n(a.get(1))],a.get(2)==null?null:Oracle.npcs.get(Rec.n(a.get(2))),a.get(3)==null?null:Oracle.interps[Rec.n(a.get(3))]);break;}
    case"i":Oracle.act((Map)o.get(1));break;
    case"btn":if(button==Rec.n(o.get(1)))run((List)o.get(2),iface,button);break;
    case"fail":throw new Rec.Failure();
    default:throw new AssertionError(op);
   }
  }
 }
}
//// oracle/Oracle.java
package oracle;
import java.io.*;import java.lang.reflect.Field;import java.util.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.dialogue.*;import core.game.component.Component;import core.game.node.entity.Entity;import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;import core.game.node.entity.player.link.*;import core.game.node.item.Item;
import core.cache.def.impl.*;import core.game.world.GameWorld;import core.game.system.config.ItemConfigParser;
public class Oracle implements Rec.Driver {
 public static Player[] players;public static PacketDispatch[] dispatches;public static InterfaceManager[] managers;public static DialogueInterpreter[] interps;
 public static List<NPC> npcs;public static List<ScriptPlugin> protos,instances,reflected;public static List<ScriptFile> files;public static List<Topic<?>> topics;public static Act[] acts;public static String[] cur;
 static int mark;
 static int v(Map a,String k,int d){return a.get(k)==null?d:Rec.n(a.get(k));}
 static boolean b(Map a,String k){return Boolean.TRUE.equals(a.get(k));}
 public static String text(Object raw){if(raw==null)return null;if(raw instanceof String)return(String)raw;StringBuilder s=new StringBuilder();for(Object u:(List)raw)s.append((char)Rec.n(u));return s.toString();}
 public static String[] lines(Object raw){if(raw==null){cur=null;return null;}List l=(List)raw;String[] out=new String[l.size()];for(int i=0;i<out.length;i++)out[i]=text(l.get(i));cur=out;return out;}
 public static FacialExpression expr(Object raw){return raw==null?null:FacialExpression.valueOf((String)raw);}
 public static ScriptFile file(Object raw){return raw==null?null:files.get(Rec.n(raw));}
 public static Topic<?>[] topics(Object raw){List l=(List)raw;Topic<?>[] out=new Topic<?>[l.size()];for(int i=0;i<out.length;i++)out[i]=topics.get(Rec.n(l.get(i)));return out;}
 public static Entity entity(Object raw){if(raw==null)return null;String s=(String)raw;int i=Integer.parseInt(s.substring(1));return s.charAt(0)=='p'?players[i]:npcs.get(i);}
 static Object comp(Component c){return c==null?null:c.ref;}
 static Object[] args(Map a){List l=(List)a.get("args");if(l==null)return new Object[0];Object[] out=new Object[l.size()];for(int i=0;i<out.length;i++){Object x=l.get(i);
   if(x instanceof Map){Map m=(Map)x;if(m.containsKey("npc"))x=npcs.get(Rec.n(m.get("npc")));else if(m.containsKey("file"))x=files.get(Rec.n(m.get("file")));else if(m.containsKey("s"))x=text(m.get("s"));else throw new AssertionError(m);}
   else if(x instanceof Number)x=Rec.n(x);
   out[i]=x;}return out;}
 static DialoguePlugin pluginRef(Map a){if(a.get("proto")!=null)return protos.get(Rec.n(a.get("proto")));if(a.get("inst")!=null)return instances.get(Rec.n(a.get("inst")));return null;}
 static ScriptPlugin proto(Map spec,int index){
  String cls=spec.get("cls")==null?"A":(String)spec.get("cls");String mode=(String)spec.get("mode");
  ScriptPlugin p="noctor".equals(mode)?new NoCtorPlugin():"reflect".equals(mode)?new ReflectPlugin():"B".equals(cls)?new ScriptPluginB():new ScriptPluginA();
  return p.bind(spec,"p"+index);
 }
 public void hook(Map action){act(action);}
 public static Object act(Map a){
  int p=v(a,"p",0);DialogueInterpreter d=interps[v(a,"d",p)];
  switch((String)a.get("op")){
   case"open":return d.open(Rec.n(a.get("key")),args(a));
   case"openName":return d.open(text(a.get("name")),args(a));
   case"openFile":d.open(file(a.get("file")),args(a));return null;
   case"handle":d.handle(v(a,"c",0),v(a,"b",0));return null;
   case"close":return d.close();
   case"dialogue":return comp(d.sendDialogue(lines(a.get("lines"))));
   case"plain":return comp(d.sendPlainMessage(b(a,"hide"),lines(a.get("lines"))));
   case"destroy":return comp(d.sendDestroyItem(v(a,"id",0),text(a.get("message"))));
   case"blue":return comp(d.sendPlaneMessageWithBlueTitle(text(a.get("title")),lines(a.get("lines"))));
   case"scroll":return comp(d.sendScrollMessageWithBlueTitle(text(a.get("title")),lines(a.get("lines"))));
   case"item":if(a.containsKey("id"))return comp(d.sendItemMessage(Rec.n(a.get("id")),lines(a.get("lines"))));return comp(d.sendItemMessage(a.get("item")==null?null:Item.all.get(Rec.n(a.get("item"))),lines(a.get("lines"))));
   case"double":{if(a.containsKey("ids")){List ids=(List)a.get("ids");return comp(d.sendDoubleItemMessage(Rec.n(ids.get(0)),Rec.n(ids.get(1)),lines(a.get("lines"))));}
    List it=(List)a.get("items");return comp(d.sendDoubleItemMessage(it.get(0)==null?null:Item.all.get(Rec.n(it.get(0))),it.get(1)==null?null:Item.all.get(Rec.n(it.get(1))),lines(a.get("lines"))));}
   case"say":{String form=(String)a.get("form");Entity e=entity(a.get("e"));int npc=v(a,"npc",0);FacialExpression x=expr(a.get("expr"));int anim=v(a,"anim",-1);boolean hide=b(a,"hide");String[] l=lines(a.get("lines"));
    switch(form){
     case"eF":return comp(d.sendDialogues(e,x,l));
     case"eI":return comp(d.sendDialogues(e,anim,l));
     case"nFh":return comp(d.sendDialogues(npc,x,hide,l));
     case"eFh":return comp(d.sendDialogues(e,x,hide,l));
     case"eIh":return comp(d.sendDialogues(e,anim,hide,l));
     case"nF":return comp(d.sendDialogues(npc,x,l));
     case"nI":return comp(d.sendDialogues(npc,anim,l));
     case"nIh":return comp(d.sendDialogues(npc,anim,hide,l));
     default:throw new AssertionError(form);}}
   case"options":return comp(d.sendOptions(text(a.get("title")),lines(a.get("lines"))));
   case"input":d.sendInput(b(a,"string"),args(a));return null;
   case"long":d.sendLongInput(args(a));return null;
   case"message":d.sendMessageInput(text(a.get("to")));return null;
   case"add":DialogueInterpreter.add(Rec.n(a.get("id")),pluginRef(a));return null;
   case"init":protos.get(Rec.n(a.get("proto"))).init();return null;
   case"simple":new SimpleDialoguePlugin().init();return null;
   case"contains":return DialogueInterpreter.contains(Rec.n(a.get("id")));
   case"keyOf":return DialogueInterpreter.getDialogueKey(text(a.get("name")));
   case"subst":return Rec.units(DialogueInterpreter.doSubstitutions(a.containsKey("nullPlayer")?null:players[p],text(a.get("msg"))));
   case"addAction":d.addAction(a.get("action")==null?null:acts[Rec.n(a.get("action"))]);return null;
   case"runAction":d.getActions().get(v(a,"index",0)).handle(players[p],v(a,"b",0));return null;
   case"setDialogue":d.setDialogue(pluginRef(a));return null;
   case"new":{ScriptPlugin proto=protos.get(Rec.n(a.get("proto")));ScriptPlugin made="B".equals(proto.spec.get("cls"))?new ScriptPluginB(a.containsKey("nullPlayer")?null:players[p]):new ScriptPluginA(a.containsKey("nullPlayer")?null:players[p]);made.bind(proto.spec,"P"+instances.size());instances.add(made);return made.label;}
   case"exprs":{List out=new ArrayList();for(FacialExpression x:FacialExpression.values())out.add(Arrays.asList(x.name(),x.getAnimationId()));return out;}
   case"pm":{ScriptPlugin target=a.get("proto")!=null?protos.get(Rec.n(a.get("proto"))):instances.get(Rec.n(a.get("inst")));return target.run((List)a.get("ops"),0,v(a,"b",0));}
   case"fm":files.get(Rec.n(a.get("file"))).run((List)a.get("ops"),0,v(a,"b",0));return null;
   case"line":cur[v(a,"i",0)]=text(a.get("v"));return null;
   case"field":field(a,p);return null;
   default:throw new AssertionError(a);
  }
 }
 static void field(Map a,int p){
  Object value=a.get("v");int m=v(a,"m",p);
  switch((String)a.get("f")){
   case"dispatch":players[p].rawDispatch(value==null?null:dispatches[Rec.n(value)]);break;
   case"manager":players[p].rawManager(value==null?null:managers[Rec.n(value)]);break;
   case"interpreter":players[p].rawInterpreter(value==null?null:interps[Rec.n(value)]);break;
   case"chatbox":managers[m].rawChatbox(value==null?null:Component.all.get(Rec.n(value)));break;
   case"closeEvent":{Component c=a.get("c")!=null?Component.all.get(Rec.n(a.get("c"))):managers[m].rawChatbox();c.rawCloseEvent(Boolean.TRUE.equals(value)?new Object():null);break;}
   case"componentId":Component.all.get(Rec.n(a.get("c"))).rawId(Rec.n(value));break;
   case"veto":managers[m].veto=(Boolean)value;break;
   case"username":players[p].rawUsername(text(value));break;
   case"male":players[p].rawMale((Boolean)value);break;
   case"debug":players[p].rawDebug((Boolean)value);break;
   case"nodeId":players[p].rawNodeId(Rec.n(value));break;
   case"server":GameWorld.settings.name=text(value);break;
   case"noSettings":GameWorld.noSettings=(Boolean)value;break;
   case"npcName":NPCDefinition.names.put(Rec.n(a.get("id")),text(value));break;
   case"npcMissing":NPCDefinition.missing.add(Rec.n(a.get("id")));break;
   case"npcId":npcs.get(Rec.n(a.get("n"))).rawId=Rec.n(value);break;
   case"shown":npcs.get(Rec.n(a.get("n"))).rawShown=value==null?null:npcs.get(Rec.n(value));break;
   case"noQueue":npcs.get(Rec.n(a.get("n"))).noQueue=(Boolean)value;break;
   case"noPulses":npcs.get(Rec.n(a.get("n"))).noPulses=(Boolean)value;break;
   case"itemId":Item.all.get(Rec.n(a.get("item"))).rawId=Rec.n(value);break;
   case"itemAmount":Item.all.get(Rec.n(a.get("item"))).rawAmount=Rec.n(value);break;
   case"def":{ItemDefinition def=ItemDefinition.all.computeIfAbsent(Rec.n(a.get("id")),ItemDefinition::new);List l=(List)value;def.zoom=Rec.n(l.get(0));def.rotationX=Rec.n(l.get(1));def.rotationY=Rec.n(l.get(2));break;}
   case"defName":ItemDefinition.all.computeIfAbsent(Rec.n(a.get("id")),ItemDefinition::new).name=text(value);break;
   case"destroyMessage":ItemDefinition.all.computeIfAbsent(Rec.n(a.get("id")),ItemDefinition::new).handlers.put(ItemConfigParser.DESTROY_MESSAGE,value instanceof Number?(Object)Rec.n(value):text(value));break;
   case"attr":players[p].rawAttributes().put((String)a.get("key"),value instanceof Number?(Object)Rec.n(value):value);break;
   default:throw new AssertionError(a);
  }
 }
 /** Calls a Kotlin $default bridge so the original default argument is used. */
 public static Object defaulted(String name,Class<?>[] sig,Object...args){
  try{return DialogueFile.class.getMethod(name+"$default",sig).invoke(null,args);}
  catch(java.lang.reflect.InvocationTargetException e){Throwable c=e.getCause();if(c instanceof RuntimeException)throw(RuntimeException)c;if(c instanceof Error)throw(Error)c;throw new AssertionError(c);}
  catch(ReflectiveOperationException e){throw new AssertionError(e);}
 }
 static Object get(Class<?> c,Object o,String name){try{Field f=c.getDeclaredField(name);f.setAccessible(true);return f.get(o);}catch(ReflectiveOperationException e){throw new AssertionError(e);}}
 static Object pluginState(DialoguePlugin p){
  List<Object> names=new ArrayList<>();for(Object s:(List)get(DialoguePlugin.class,p,"optionNames"))names.add(Rec.units((String)s));
  List<Object> optionFiles=new ArrayList<>();for(Object f:(List)get(DialoguePlugin.class,p,"optionFiles"))optionFiles.add(Describe.file((DialogueFile)f));
  NPC npc=(NPC)get(DialoguePlugin.class,p,"npc");Player owner=(Player)get(DialoguePlugin.class,p,"player");DialogueInterpreter di=(DialogueInterpreter)get(DialoguePlugin.class,p,"interpreter");
  return Arrays.asList(Describe.plugin(p),p.stage,get(DialoguePlugin.class,p,"finished"),Describe.file(p.file),npc==null?null:npc.oid,owner==null?null:owner.oid,index(di),names,optionFiles);
 }
 static Object index(DialogueInterpreter d){if(d==null)return null;for(int i=0;i<interps.length;i++)if(interps[i]==d)return i;return -1;}
 static String error(Throwable e){return e instanceof Rec.Failure?"Binding":e instanceof NullPointerException?"Null":e instanceof IndexOutOfBoundsException?"Index":e instanceof ClassCastException?"Cast":e instanceof NumberFormatException?"Number":e instanceof IllegalArgumentException?"IllegalArgument":e.getClass().getSimpleName();}
 static Object snapshot(Object result,Throwable e){
  Map<String,Object> out=new TreeMap<>();
  out.put("error",e==null?null:error(e));
  out.put("message",e instanceof IllegalArgumentException&&e.getMessage()!=null&&e.getMessage().startsWith("Dialogue ")?e.getMessage():null);
  out.put("result",result);
  List<Object> is=new ArrayList<>();
  for(DialogueInterpreter d:interps){
   List<Object> as=new ArrayList<>();for(DialogueAction x:d.getActions())as.add(x==null?null:((Act)x).id);
   List<Object> ts=new ArrayList<>();for(Topic<?> t:d.activeTopics)ts.add(topics.indexOf(t));
   is.add(Arrays.asList(Describe.plugin(d.getDialogue()),get(DialogueInterpreter.class,d,"key"),as,ts));
  }
  out.put("interp",is);
  List<Object> ps=new ArrayList<>();
  for(ScriptPlugin p:protos)ps.add(pluginState(p));
  for(ScriptPlugin p:instances)ps.add(pluginState(p));
  for(int i=0;i<Describe.seenPlugins.size();i++)ps.add(pluginState(Describe.seenPlugins.get(i)));
  out.put("plugins",ps);
  List<Object> fs=new ArrayList<>();
  for(ScriptFile f:files)fs.add(Arrays.asList(f.baseStage(),f.shadowStage,f.getPlayer()==null?null:f.getPlayer().oid,f.getNpc()==null?null:f.getNpc().oid,index(f.getInterpreter()),Describe.plugin(f.getDialoguePlugin())));
  out.put("files",fs);
  List<Object> at=new ArrayList<>();
  for(Player q:players){Map<String,Object> m=new TreeMap<>();for(Map.Entry<String,Object> en:q.rawAttributes().entrySet())m.put(en.getKey(),Describe.value(en.getValue()));at.add(m);}
  out.put("attrs",at);
  List<Object> ch=new ArrayList<>();for(InterfaceManager m:managers)ch.add(m.rawChatbox()==null?null:m.rawChatbox().ref);
  out.put("chat",ch);
  List<Object> cs=new ArrayList<>();for(Component c:Component.all)cs.add(Arrays.asList(c.rawId(),c.rawCloseEvent()!=null));
  out.put("comps",cs);
  out.put("events",new ArrayList<>(Rec.events.subList(mark,Rec.events.size())));mark=Rec.events.size();
  return out;
 }
 @SuppressWarnings("unchecked") static void world(Map s){
  Rec.reset(new HashMap());Describe.reset();Component.all=new ArrayList<>();Item.all=new ArrayList<>();ItemDefinition.all=new TreeMap<>();NPCDefinition.names=new TreeMap<>();NPCDefinition.missing=new TreeSet<>();GameWorld.settings=new GameWorld.Settings();GameWorld.noSettings=false;
  ((Map)get(DialogueInterpreter.class,null,"PLUGINS")).clear();
  players=new Player[2];dispatches=new PacketDispatch[3];managers=new InterfaceManager[3];interps=new DialogueInterpreter[3];
  for(int i=0;i<3;i++){dispatches[i]=new PacketDispatch(i);managers[i]=new InterfaceManager(i);}
  for(int i=0;i<2;i++){players[i]=new Player(i);interps[i]=new DialogueInterpreter(players[i]);players[i].raw(dispatches[i],managers[i],interps[i]);}
  interps[2]=new DialogueInterpreter(null);
  npcs=new ArrayList<>();List ns=(List)s.get("npcs");if(ns==null)ns=Arrays.asList(1L,9000L,2L);for(Object id:ns)npcs.add(new NPC(npcs.size(),Rec.n(id)));
  List items=(List)s.getOrDefault("items",Collections.emptyList());for(Object it:items){List l=(List)it;new Item(Rec.n(l.get(0)),Rec.n(l.get(1)));}
  files=new ArrayList<>();for(Object f:(List)s.getOrDefault("files",Collections.emptyList()))files.add(new ScriptFile((Map)f,"F"+files.size()));
  topics=new ArrayList<>();for(Object raw:(List)s.getOrDefault("topics",Collections.emptyList())){Map t=(Map)raw;
   Object to=t.get("file")!=null?(Object)files.get(Rec.n(t.get("file"))):t.get("stage")!=null?(Object)Rec.n(t.get("stage")):"other";
   FacialExpression x=t.get("expr")==null?FacialExpression.ASKING:expr(t.get("expr"));boolean skip=b(t,"skip");
   topics.add(t.get("if")!=null?new IfTopic<Object>(x,text(t.get("text")),to,(Boolean)t.get("if"),skip):new Topic<Object>(x,text(t.get("text")),to,skip));}
  acts=new Act[]{new Act(0),new Act(1),new Act(2)};
  protos=new ArrayList<>();instances=new ArrayList<>();reflected=new ArrayList<>();
  for(Object raw:(List)s.getOrDefault("protos",Collections.emptyList()))protos.add(proto((Map)raw,protos.size()));
  cur=null;Rec.reset(s);mark=0;
 }
 public static void main(String[] args)throws Exception{
  Rec.driver=new Oracle();
  PrintStream out=new PrintStream(new FileOutputStream(FileDescriptor.out),false,"UTF-8");
  System.setErr(new PrintStream(new OutputStream(){public void write(int b){}}){
   @Override public void println(Object x){if(x instanceof Throwable)Rec.emit("trace",error((Throwable)x));}
   @Override public void println(String x){Rec.emit("stderr",Rec.units(x));}
  });
  BufferedReader r=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));String line;
  while((line=r.readLine())!=null){
   Map input=(Map)new JSONParser().parse(line);world(input);
   List<Object> expected=new ArrayList<>();
   for(Object a:(List)input.get("actions")){Throwable error=null;Object result=null;try{result=act((Map)a);}catch(Throwable e){if(e instanceof AssertionError)throw e;error=e;}expected.add(snapshot(result,error));}
   input.put("expected",expected);out.println(JSONValue.toJSONString(input));
  }
  out.flush();
 }
}
