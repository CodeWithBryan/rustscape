package core.game.node.entity.player;
import oracle.PinOracle;import core.game.node.entity.player.link.BankPinManager;
public class Player {
 public final int id;public BankPinManager manager;
 public Player(int id){this.id=id;manager=new BankPinManager(this);}
 public BankPinManager getBankPinManager(){PinOracle.hit("pinManager",id);return manager;}
 public Interfaces getInterfaceManager(){PinOracle.hit("interfaces",id);return PinOracle.nil("interfaces")?null:new Interfaces(id);}
 public Dispatch getPacketDispatch(){PinOracle.hit("dispatch",id);return PinOracle.nil("dispatch")?null:new Dispatch(id);}
 public Bank getBank(){PinOracle.hit("bank",id);return PinOracle.nil("bank")?null:new Bank(id);}
 public Dialogue getDialogueInterpreter(){PinOracle.hit("dialogue",id);return PinOracle.nil("dialogue")?null:new Dialogue(id);}
 public void sendMessage(String value){PinOracle.hit("message",id,PinOracle.units(value));}
 public static class Interfaces{
  int id;Interfaces(int id){this.id=id;}
  public void close(){PinOracle.hit("close",id);if(PinOracle.flag("invokeClose"))PinOracle.closeCurrent(id);}
  public void open(core.game.component.Component c){PinOracle.hit("open",id,c.id);PinOracle.current[id]=c;}
 }
 public static class Dispatch{
  int id;Dispatch(int id){this.id=id;}
  public void sendString(String value,int component,int child){PinOracle.hit("string",id,PinOracle.units(value),component,child);}
  public void sendInterfaceConfig(int component,int child,boolean hidden){PinOracle.hit("config",id,component,child,hidden);}
  public void sendMessage(String value){PinOracle.hit("packetMessage",id,PinOracle.units(value));}
 }
 public static class Bank{int id;Bank(int id){this.id=id;}public void open(){PinOracle.hit("openBank",id);}}
 public static class Dialogue{int id;Dialogue(int id){this.id=id;}public void sendDialogue(String...values){PinOracle.hit("sendDialogue",id,PinOracle.units(values[0]));}}
}
