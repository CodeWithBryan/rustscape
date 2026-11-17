package core.game.node.entity.player;
import java.util.HashMap;import oracle.VarsOracle;import core.game.node.entity.player.link.PacketDispatch;
public class Player {
 public final int id;
 // These are fields in the reference Player, not synthetic Kotlin getters.
 public HashMap<Integer,Integer> varpMap=new HashMap<>();
 public HashMap<Integer,Boolean> saveVarp=new HashMap<>();
 public int counter;
 public Player(int id){this.id=id;}
 public PacketDispatch getPacketDispatch(){VarsOracle.hit("dispatch",id);return VarsOracle.nil("dispatch")?null:new PacketDispatch(this);}
 public Session getSession(){VarsOracle.hit("session",id);return VarsOracle.nil("session")?null:new Session(id);}
 public Details getDetails(){VarsOracle.hit("details",id);return VarsOracle.nil("details")?null:new Details(id);}
 public boolean isArtificial(){VarsOracle.hit("artificial",id);return VarsOracle.flag("artificial");}
 public Interfaces getInterfaceManager(){VarsOracle.hit("interfaces",id);return VarsOracle.nil("interfaces")?null:new Interfaces(id);}
 public static class Details{int id;Details(int id){this.id=id;}public Session getSession(){VarsOracle.hit("detailSession",id);return VarsOracle.nil("detailSession")?null:new Session(id);}}
 public static class Session{int id;Session(int id){this.id=id;}public Pair getIsaacPair(){VarsOracle.hit("pair",id);return VarsOracle.nil("pair")?null:new Pair(id);}public void write(core.net.packet.IoBuffer b){VarsOracle.hit("write",id);VarsOracle.frame(b);}}
 public static class Pair{int id;Pair(int id){this.id=id;}public core.cache.crypto.ISAACCipher getOutput(){VarsOracle.hit("cipher",id);return new core.cache.crypto.ISAACCipher();}}
 public static class Interfaces{int id;Interfaces(int id){this.id=id;}public int getPacketCount(int delta){VarsOracle.hit("counter",id,delta);int value=VarsOracle.players[id].counter;VarsOracle.players[id].counter+=delta;return value;}}
}
