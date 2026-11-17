package core.game.node.entity.player;
import oracle.ListenerOracle;
public class Player{
 public Settings settings=new Settings();public Familiar[] fam={new Familiar(0),new Familiar(1)};public int active;public Appearance appearance=new Appearance();public int familiarReads;
 public Settings getSettings(){ListenerOracle.hit("settings");return ListenerOracle.nil("settings")?null:settings;}
 public Familiar getFamiliarManager(){ListenerOracle.hit("familiar");familiarReads++;return ListenerOracle.nil("familiar"+familiarReads)?null:fam[active];}
 public Appearance getAppearance(){ListenerOracle.hit("appearance");return ListenerOracle.nil("appearance")?null:appearance;}
 public Session getSession(){return new Session();}
 public static class Settings{public void updateWeight(){ListenerOracle.hit("weight");}}
 public static class Familiar{public final int id;public boolean has;public Familiar(int i){id=i;}public boolean isHasPouch(){ListenerOracle.hit("has",id);return has;}public void setHasPouch(boolean v){ListenerOracle.hit("set",id,v);has=v;}}
 public static class Appearance{public int syncs;public void sync(){ListenerOracle.hit("sync");syncs++;}}
 public static class Session{public Pair getIsaacPair(){return new Pair();}public void write(core.net.packet.IoBuffer b){ListenerOracle.frame(b);}}
 public static class Pair{public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}}
}
