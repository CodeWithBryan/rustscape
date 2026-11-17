package core.game.node.entity.player;
import core.net.IoSession;
import core.game.world.map.Location;
public class Player {
 public final IoSession session=new IoSession(); public final Interfaces interfaces=new Interfaces();
 public final core.game.node.entity.skill.Skills skills=new core.game.node.entity.skill.Skills();public final Settings settings=new Settings();
 public core.game.node.entity.skill.Skills getSkills(){return skills;}public Settings getSettings(){return settings;}
 public static class Settings{public double energy;public double getRunEnergy(){return energy;}}
 public IoSession getSession(){return session;} public Player getDetails(){return this;}
 public boolean isArtificial(){return false;}
 public Interfaces getInterfaceManager(){return interfaces;} public Player getPlayerFlags(){return this;}
 public Location getLastSceneGraph(){return new Location(0,0);} public void sendMessage(String x){throw new UnsupportedOperationException("outside scope");}
 public static class Interfaces{public int count;public int getPacketCount(int increment){int old=count;count+=increment;return old;}}
}
