package oracle;
import java.util.*;
import core.game.component.*;
import core.game.node.entity.player.link.InterfaceManager;
/** Observations and boundary services only; lifecycle bodies are the whole original class. */
public class ManagerSupport {
 static Object runscript;
 public static void reset(){runscript=null;}
 public static Object getRunscript(){Object captured=runscript;Support.emit("runscriptGet");return captured;}
 public static void removeRunscript(){runscript=null;Support.emit("runscriptRemove");}
 static Component component(Object index){return index==null?null:Support.components.get(Support.n(index));}
 public static boolean handles(Map a){return ((String)a.get("op")).startsWith("manager");}
 public static Object act(Map a){InterfaceManager m=Support.player.manager;Component c=component(a.get("component"));switch((String)a.get("op")){
 case"managerOpen":return Support.slot(m.open(c));
 case"managerOpenId":return Support.slot(m.openComponent(Support.n(a.get("id"))));
 case"managerClose":return m.close();
 case"managerCloseComponent":return m.close(c);
 case"managerCloseChat":m.closeChatbox();break;
 case"managerSingle":return Support.slot(m.openSingleTab(c));
 case"managerCloseSingle":return m.closeSingleTab();
 case"managerOverlay":m.openOverlay(c);break;
 case"managerCloseOverlay":m.closeOverlay();break;
 case"managerRemoveTabs":List l=(List)a.get("indices");int[]indices=new int[l.size()];for(int i=0;i<indices.length;i++)indices[i]=Support.n(l.get(i));m.removeTabs(indices);break;
 case"managerRestoreTabs":m.restoreTabs();break;
 case"managerCloseTabs":m.closeDefaultTabs();break;
 case"managerViewedTab":m.setViewedTab(Support.n(a.get("index")));break;
 case"managerGet":return Support.slot(m.getComponent(Support.n(a.get("id"))));
 case"managerWalkable":return m.isWalkable();
 case"managerHasMain":return m.hasMainComponent(Support.n(a.get("id")));
 case"managerIsOpened":return m.isOpened();
 case"managerHasChat":return m.hasChatbox();
 case"managerSameClose":return c.getCloseEvent()==component(a.get("other")).getCloseEvent();
 case"managerState":
  for(String key:new String[]{"overlay","singleTab","wildyOverlay"})if(a.containsKey(key))Support.field(m,key,component(a.get(key)));
  if(a.containsKey("index"))m.setCurrentTabIndex(Support.n(a.get("index")));
  if(a.containsKey("runscript"))runscript=a.get("runscript");
  if(a.containsKey("tabsNull"))m.setTabs(null);
  break;
 default:throw new IllegalArgumentException((String)a.get("op"));
 }return null;}
 public static Map snapshot(){InterfaceManager m=Support.player.manager;Map out=new LinkedHashMap();out.put("single",Support.slot(m.getSingleTab()));out.put("overlay",Support.slot(m.getOverlay()));out.put("index",m.getCurrentTabIndex());out.put("runscript",runscript);List events=new ArrayList();for(Component c:Support.components)events.add(c.getCloseEvent()!=null);out.put("closeEvents",events);return out;}
}
