package core.game.node.entity.player;
import core.game.container.Container;import core.game.node.entity.player.link.Settings;import core.net.packet.PacketRepository;import core.net.packet.out.StringPacket;import core.net.packet.context.StringContext;
public class Player{
 public final Container inventory=new Container(28),equipment=new Container(14);public final Settings settings=new Settings(this);public final Familiar familiar=new Familiar();public final Appearance appearance=new Appearance(this);public final Manager manager=new Manager();public final Dispatch dispatch=new Dispatch(this);public int syncs;private final Session session=new Session();
 public Container getInventory(){return inventory;}public Container getEquipment(){return equipment;}public Settings getSettings(){return settings;}public Familiar getFamiliarManager(){return familiar;}public Appearance getAppearance(){return appearance;}public Manager getInterfaceManager(){return manager;}public Dispatch getPacketDispatch(){return dispatch;}public Session getSession(){return session;}public Details getDetails(){return new Details(session);}public void updateAppearance(){syncs++;}
 public static class Familiar{private boolean hasPouch;/*FAMILIAR*/}
 public static class Appearance{private final Player player;public Appearance(Player p){player=p;}/*APPEARANCE*/}
 public static class Manager{public int packetCount;/*COUNTER*/}
 public static class Dispatch{private final Player player;public Dispatch(Player p){player=p;}/*TEXT*/}
 public static class Session{public Pair getIsaacPair(){return new Pair();}public void write(core.net.packet.IoBuffer buffer){oracle.BoundOracle.frame(buffer);}}
 public static class Pair{public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}}
 public static class Details{private final Session session;public Details(Session s){session=s;}public Session getSession(){return session;}}
}
