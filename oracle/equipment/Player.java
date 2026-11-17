package core.game.node.entity.player;
import core.game.container.*;
import core.game.container.impl.EquipmentContainer;
import core.game.node.entity.combat.equipment.WeaponInterface;
import oracle.EquipmentOracle;
public class Player {
 public final Container inventory=new Container(28);
 public final EquipmentContainer equipment=new EquipmentContainer(this);
 public final Properties properties=new Properties();public final Settings settings=new Settings();public final Appearance appearance=new Appearance();public final Manager manager=new Manager();public final Dispatch dispatch=new Dispatch();
 public boolean dfs,special,extension=true;public int weights,animations,appearanceUpdates,defence,capes;
 public Container getInventory(){EquipmentOracle.hit("inventory");return inventory;}
 public EquipmentContainer getEquipment(){EquipmentOracle.hit("equipment");return equipment;}
 public Dispatch getPacketDispatch(){EquipmentOracle.hit("dispatch");return dispatch;}
 public Properties getProperties(){EquipmentOracle.hit("properties");return properties;}
 public Settings getSettings(){EquipmentOracle.hit("settings");return settings;}
 public Appearance getAppearance(){EquipmentOracle.hit("appearance");return appearance;}
 public Manager getInterfaceManager(){EquipmentOracle.hit("interfaces");return manager;}
 public <T>T getExtension(Class<T> t){EquipmentOracle.hit("extension");return extension?t.cast(new WeaponInterface()):null;}
 public boolean getAttribute(String k,boolean d){EquipmentOracle.hit("attribute",k,d);return dfs;}
 public Object removeAttribute(String k){EquipmentOracle.hit("removeAttribute",k);dfs=false;return null;}
 public void updateAppearance(){EquipmentOracle.hit("updateAppearance");appearanceUpdates++;}
 public Session getSession(){return new Session();}
 public static class Properties {public int speed;public int[] bonuses=new int[15];public void setAttackSpeed(int v){EquipmentOracle.hit("attackSpeed",v);speed=v;}public void updateDefenceAnimation(){EquipmentOracle.hit("defence");EquipmentOracle.player.defence++;}public CombatPulse getCombatPulse(){EquipmentOracle.hit("combatPulse");return new CombatPulse();}public void setBonuses(int[] b){EquipmentOracle.hit("setBonuses",b);bonuses=b;}public int[] getBonuses(){EquipmentOracle.hit("bonuses");return bonuses;}}
 public static class CombatPulse{public void setHandler(Object v){EquipmentOracle.hit("clearHandler");}}
 public static class Settings{public boolean isSpecialToggled(){EquipmentOracle.hit("special");return EquipmentOracle.player.special;}public void updateWeight(){EquipmentOracle.hit("weight");EquipmentOracle.player.weights++;}}
 public static class Appearance{public void setAnimations(){EquipmentOracle.hit("animations");EquipmentOracle.player.animations++;}}
 public static class Manager{public boolean hasMainComponent(int id){EquipmentOracle.hit("hasMain",id);return Boolean.TRUE.equals(EquipmentOracle.row.get("main"));}}
 public static class Dispatch{public void sendMessage(String text){EquipmentOracle.hit("message",text);}public void sendString(String text,int i,int child){EquipmentOracle.hit("text",text,i,child);}}
 public static class Session{public Pair getIsaacPair(){return new Pair();}public void write(core.net.packet.IoBuffer b){EquipmentOracle.frame(b);}}
 public static class Pair{public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}}
}
