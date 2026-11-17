package core.game.node.entity.player;
import core.game.container.*;import core.game.node.item.Item;import core.game.container.impl.BankContainer;import core.game.component.*;import core.game.node.entity.player.link.IronmanMode;import oracle.BankOracle;
public class Player {
 public final int id;public Container inventory=new Container(28);public BankContainer bank;public Container equipment=new Container(14);
 public Player(int id){this.id=id;}
 public Container getInventory(){BankOracle.hit("inventory",id);return inventory;}
 public BankContainer getBank(){BankOracle.hit("bank",id);return bank;}
 public Ironman getIronmanManager(){BankOracle.hit("ironman",id);return new Ironman(id);}
 public Pin getBankPinManager(){BankOracle.hit("pin",id);return new Pin(id);}
 public Interfaces getInterfaceManager(){BankOracle.hit("interfaces",id);return new Interfaces(id);}
 public Dispatch getPacketDispatch(){BankOracle.hit("dispatch",id);return new Dispatch(id);}
 public void sendMessage(String s){BankOracle.hit("message",id,s);}
 public void removeAttribute(String key){BankOracle.hit("removeAttribute",id,key);BankOracle.attributes.get(id).remove(key);}
 public Container getEquipment(){BankOracle.hit("equipment",id);return equipment;}
 /* ORIGINAL_HAS_ITEM */
 public Session getSession(){return new Session();}
 public static class Ironman{int id;Ironman(int id){this.id=id;}public boolean checkRestriction(IronmanMode mode){BankOracle.hit("restriction",id);return BankOracle.flag("restricted");}}
 public static class Pin{int id;Pin(int id){this.id=id;}public boolean isUnlocked(){BankOracle.hit("unlocked",id);return !BankOracle.flag("locked");}public void openType(int n){BankOracle.hit("openPin",id,n);}}
 public static class Interfaces{int id;Interfaces(int id){this.id=id;}public Component open(Component c){BankOracle.hit("open",id,c.id);return c;}public Component openComponent(int n){BankOracle.hit("openComponent",id,n);return new Component(n);}public void removeTabs(int...tabs){BankOracle.hit("removeTabs",id,tabs);}public void openDefaultTabs(){BankOracle.hit("defaultTabs",id);}public void openSingleTab(Component c){BankOracle.hit("singleTab",id,c.id);}public void closeSingleTab(){BankOracle.hit("closeSingle",id);}}
 public static class Dispatch{int id;Dispatch(int id){this.id=id;}public void sendIfaceSettings(int a,int b,int c,int d,int e){BankOracle.hit("ifaceSettings",id,a,b,c,d,e);}public void sendRunScript(int script,String text,Object...args){BankOracle.hit("script",id,script);}public void sendMessage(String text){BankOracle.hit("packetMessage",id,text);}}
 public static class Session{public Pair getIsaacPair(){return new Pair();}public void write(core.net.packet.IoBuffer b){BankOracle.frame(b);}}
 public static class Pair{public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}}
}
