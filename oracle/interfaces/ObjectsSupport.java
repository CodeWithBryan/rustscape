package oracle;
import java.util.*;import core.game.component.*;import core.game.node.entity.player.Player;
/** Object identities are observation references, never runtime dispatch IDs. */
public class ObjectsSupport {
 static Map<String,Object>aliases;static List<Object>objects;static Object attribute;
 public static void reset(){aliases=new TreeMap<>();objects=new ArrayList<>();attribute=null;}
 static Integer ref(Object value){if(value==null)return null;for(int i=0;i<objects.size();i++)if(objects.get(i)==value)return i;objects.add(value);return objects.size()-1;}
 public static class Plugin extends Support.Plugin {
  int count;Plugin(int label){super(label);}
  public void open(Player p,Component c){Support.emit("objectPlugin",ref(this),id,Support.slot(c));count++;}
 }
 public static class Event implements CloseEvent {
  final int label;boolean result;int count;Event(int label,boolean result){this.label=label;this.result=result;}
  public boolean close(Player p,Component c){Support.emit("objectClose",ref(this),label,Support.slot(c));count++;return result;}
 }
 public static void setAttribute(Object value){Support.emit("setAttr",value);attribute=value;}
 public static Object getAttribute(Object fallback){Support.emit("getAttr");return attribute==null?fallback:attribute;}
 static Component component(Map a){Object index=a.get("component");return index==null?null:Support.components.get(Support.n(index));}
 static Object alias(Map a){return a.get("alias")==null?null:aliases.get((String)a.get("alias"));}
 public static Object act(Map a){Component c;Object value;switch((String)a.get("op")){
  case"objectPlugin":value=new Plugin(a.get("label")==null?1:Support.n(a.get("label")));ref(value);aliases.put((String)a.get("alias"),value);break;
  case"objectEvent":value=new Event(a.get("label")==null?1:Support.n(a.get("label")),!Boolean.FALSE.equals(a.get("result")));ref(value);aliases.put((String)a.get("alias"),value);break;
  case"objectAlias":aliases.put((String)a.get("alias"),aliases.get((String)a.get("from")));break;
  case"removeAlias":aliases.remove((String)a.get("alias"));break;
  case"callbackSame":return aliases.get((String)a.get("a"))==aliases.get((String)a.get("b"));
  case"assignPlugin":component(a).setPlugin((ComponentPlugin)alias(a));break;
  case"assignDefinitionPlugin":ComponentDefinition.forId(Support.n(a.get("id"))).setPlugin((ComponentPlugin)alias(a));break;
  case"assignClose":component(a).setCloseEvent((CloseEvent)alias(a));break;
  case"aliasPlugin":value=component(a).getPlugin();ref(value);aliases.put((String)a.get("alias"),value);break;
  case"aliasClose":value=component(a).getCloseEvent();ref(value);aliases.put((String)a.get("alias"),value);break;
  case"eventResult":((Event)alias(a)).result=(Boolean)a.get("value");break;
  case"guard":Component.setUnclosable(Support.player,component(a));ref(component(a).getCloseEvent());break;
  case"attribute":attribute=a.get("value");break;
  case"removeDefinition":ComponentDefinition.getDefinitions().remove(Support.n(a.get("id")));break;
  case"fireDefault":return((ComponentPlugin)alias(a)).fireEvent((String)a.get("identifier"),(Object[])null);
  case"newInstance":return ref(((Support.Plugin)alias(a)).newInstance(null));
  case"getters":c=component(a);return Arrays.asList(c.getId(),c.isHidden(),ref(c.getPlugin()),ref(c.getCloseEvent()),c.getDefinition()==ComponentDefinition.forId(c.getId()));
  default:throw new IllegalArgumentException((String)a.get("op"));
 }return null;}
 static Object pluginRaw(Component c){try{java.lang.reflect.Field f=Component.class.getDeclaredField("plugin");f.setAccessible(true);return f.get(c);}catch(Exception e){throw new AssertionError(e);}}
 public static Object snapshot(){
  List<Object>components=new ArrayList<>();for(Component c:Support.components)components.add(Arrays.asList(ref(pluginRaw(c)),ref(c.getCloseEvent()),c.getDefinition()==null?null:ref(c.getDefinition().getPlugin())));
  Map<String,Object>registry=new TreeMap<>();new TreeMap<Integer,ComponentDefinition>(ComponentDefinition.getDefinitions()).forEach((id,d)->registry.put(""+id,ref(d.getPlugin())));
  Map<String,Object>names=new TreeMap<>();aliases.forEach((name,o)->names.put(name,ref(o)));
  List<Object>values=new ArrayList<>();for(int i=0;i<objects.size();i++){Object o=objects.get(i);Map<String,Object>v=new TreeMap<>();v.put("ref",i);if(o instanceof Plugin){Plugin p=(Plugin)o;v.put("kind","plugin");v.put("label",p.id);v.put("count",p.count);}else if(o instanceof Event){Event e=(Event)o;v.put("kind","close");v.put("label",e.label);v.put("count",e.count);v.put("result",e.result);}else v.put("kind","guard");values.add(v);}
  Map<String,Object>out=new TreeMap<>();out.put("callbacks",values);out.put("aliases",names);out.put("components",components);out.put("registry",registry);out.put("attribute",attribute);return out;
 }
}
