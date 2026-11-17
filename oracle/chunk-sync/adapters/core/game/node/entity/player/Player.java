package core.game.node.entity.player;
import java.util.*;import core.game.world.map.Location;import core.game.node.entity.Entity;import core.net.packet.IoBuffer;
public class Player extends Entity{
 public int id,uid;public String name;public Flags flags=new Flags(this);public Map<String,Boolean>attributes=new HashMap<>();public Details details=new Details(this);public Session session=new Session(this);
 public Player(int i,int u){id=i;uid=u;name="p"+i;flags.anchor=new Location(3207,3216,0);}public Details getDetails(){return details;}public Flags getPlayerFlags(){return flags;}public Session getSession(){return session;}public String getUsername(){String n=name;oracle.Support.emit("username",id);return n;}public boolean getAttribute(String k,boolean d){boolean v=attributes.getOrDefault(k,d);oracle.Support.emit("attr",id,k);return v;}
 public static class Details{Player p;Details(Player p){this.p=p;}public int getUid(){int u=p.uid;oracle.Support.emit("uid",p.id,u);return u;}}
 public static class Flags{Player p;public Location anchor;Flags(Player p){this.p=p;}public Location getLastSceneGraph(){Location a=anchor;oracle.Support.emit("scene",p.id);return a;}}
 public static class Session{Player p;Session(Player p){this.p=p;}public void write(IoBuffer b){oracle.Support.send(p.id,b);}}
}
