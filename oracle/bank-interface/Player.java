package core.game.node.entity.player;
import java.util.*;import oracle.Oracle;import core.game.container.Container;
public class Player {
 public final int id;private Container bank,inventory;public Map<String,Object>attributes=new TreeMap<>();
 public boolean rawDialogue,rawLocked,rawZone,rawResizable=true,closeAllowed=true;
 public final Manager rawManager;private final DialogueInterpreter interpreter;private final Locks locks;private final ZoneMonitor zone;
 public Player(int n){id=n;rawManager=new Manager(this);interpreter=new DialogueInterpreter(this);locks=new Locks(this);zone=new ZoneMonitor(this);}
 public DialogueInterpreter getDialogueInterpreter(){Oracle.emit("dialogue",id);return interpreter;}
 public Locks getLocks(){Oracle.emit("locks",id);return locks;}
 public ZoneMonitor getZoneMonitor(){Oracle.emit("zone",id);return zone;}
 public Manager getInterfaceManager(){Oracle.emit("manager",id);return rawManager;}
 public<T>T getAttribute(String key){Object n=attributes.get(key);Oracle.emit("getAttr",id,key,n);return(T)n;}
 public void removeAttribute(String key){attributes.remove(key);Oracle.emit("removeAttr",id,key);}
 public void dispatch(core.game.event.ButtonClickEvent event){Oracle.emit("buttonEvent",id,event.id,event.child);}
 public void unlock(){rawLocked=false;Oracle.emit("unlock",id);}
 public static class Manager {final Player p;public final Map<Integer,core.game.component.Component>visible=new TreeMap<>();Manager(Player p){this.p=p;}public boolean close(){Oracle.emit("managerClose",p.id);boolean n=p.closeAllowed;if(n)visible.clear();return n;}public core.game.component.Component getComponent(int id){core.game.component.Component c=visible.get(id);Oracle.emit("lookup",p.id,id,Oracle.componentRef(c));return c;}public boolean isResizable(){boolean n=p.rawResizable;Oracle.emit("resizable",p.id);return n;}}
 public static class DialogueInterpreter{final Player p;DialogueInterpreter(Player p){this.p=p;}public Object getDialogue(){Object n=p.rawDialogue?this:null;Oracle.emit("active",p.id);return n;}public void close(){p.rawDialogue=false;Oracle.emit("dialogueClose",p.id);}}
 public static class Locks{final Player p;Locks(Player p){this.p=p;}public boolean isComponentLocked(){boolean n=p.rawLocked;Oracle.emit("componentLocked",p.id);return n;}}
 public static class ZoneMonitor{final Player p;ZoneMonitor(Player p){this.p=p;}public boolean clickButton(int iface,int child,int slot,int item,int opcode){Oracle.emit("zoneClick",p.id,iface,child,slot,item,opcode);return p.rawZone;}}

 public Container rawBank(){return bank;}public void rawBank(Container c){bank=c;}public Container rawInventory(){return inventory;}public void rawInventory(Container c){inventory=c;}
 public Container getBank(){Container c=bank;Oracle.emit("bank",id,c==null?null:c.ref);return c;}
 public Container getInventory(){Container c=inventory;Oracle.emit("inventory",id,c==null?null:c.ref);return c;}
 public Object attribute(String key,Object fallback){Object n=attributes.get(key);if(n==null)n=fallback;Oracle.emit("lastTab",id,n);return n;}
 public Dispatch getPacketDispatch(){Oracle.emit("dispatch",id);return new Dispatch(id);}
 public void debug(String text){Oracle.emit(Oracle.packetDebug?"packetDebug":"debug",id,text);}
 public static class Dispatch {final int id;public Dispatch(int n){id=n;}public void sendWindowsPane(int iface,int type){Oracle.emit("pane",id,iface,type);}public void sendIfaceSettings(int settings,int child,int iface,int start,int end){Oracle.emit("settings",id,settings,child,iface,start,end);}}
}
