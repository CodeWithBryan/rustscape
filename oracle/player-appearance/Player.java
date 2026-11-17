package core.game.node.entity.player;
import core.game.container.Container;import core.game.node.entity.player.link.appearance.Appearance;import core.game.world.update.flag.context.Animation;import oracle.AppearanceOracle;
public class Player {
 public final int id;public final Container equipment=new Container(14);public Appearance appearance;public final Render[] renders={new Render(0),new Render(1)};public int activeRender,size=1;public final Settings settings=new Settings();public final Properties properties=new Properties();
 public Player(int id){this.id=id;}
 public Container getEquipment(){AppearanceOracle.hit("equipment",id);return equipment;}
 public Render getRenderInfo(){AppearanceOracle.hit("render",id);return renders[activeRender];}
 public Appearance getAppearance(){AppearanceOracle.hit("appearance",id);return appearance;}
 public Settings getSettings(){AppearanceOracle.hit("settings",id);return settings;}
 public Properties getProperties(){AppearanceOracle.hit("properties",id);return properties;}
 public void setSize(int s){AppearanceOracle.hit("size",id,s);size=s;}
 public void updateAppearance(){AppearanceOracle.hit("sync",id);}
 public boolean animate(Animation a){AppearanceOracle.hit("animate",id,AppearanceOracle.animation(a));return Boolean.TRUE.equals(AppearanceOracle.row.get("animate"));}
 public Object getAttribute(String key){AppearanceOracle.hit("attribute",id,key,null);return AppearanceOracle.override;}
 @SuppressWarnings("unchecked") public <T>T getAttribute(String key,T d){AppearanceOracle.hit("attribute",id,key,d);return AppearanceOracle.override==null?d:(T)AppearanceOracle.override;}
 public static class Render{public final int id;public boolean prepared;public Render(int i){id=i;}public boolean preparedAppearance(){AppearanceOracle.hit("prepared",id);return prepared;}public void setPreparedAppearance(boolean v){AppearanceOracle.hit("setPrepared",id,v);prepared=v;}}
 public static class Settings{public int getAttackStyleIndex(){AppearanceOracle.hit("style");return ((Number)AppearanceOracle.row.get("style")).intValue();}}
 public static class Properties{public Animation attack,defence,death;public void setAttackAnimation(Animation a){AppearanceOracle.hit("combat","Attack",AppearanceOracle.animation(a));attack=a;}public void setDefenceAnimation(Animation a){AppearanceOracle.hit("combat","Defence",AppearanceOracle.animation(a));defence=a;}public void setDeathAnimation(Animation a){AppearanceOracle.hit("combat","Death",AppearanceOracle.animation(a));death=a;}}
}
