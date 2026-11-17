package core.game.node.item;
import core.game.world.map.Location;
public class Item {
 protected Location location;protected int index=-1;protected Interact interactPlugin;private int idHash,amount;
 public Item(int id){this(id,1,1000);}public Item(int id,int amount,int charge){idHash=(id<<16)|charge;this.amount=amount;interactPlugin=new Interact(this);oracle.Support.emit("item",id,amount,charge);}
 public int getId(){return(idHash>>16)&65535;}public int getAmount(){return amount;}public int getCharge(){return idHash&65535;}public void setAmount(int a){amount=Math.max(a,0);}public void setId(int id){idHash=(id<<16)|(idHash&65535);}public Location getLocation(){return location;}public void setLocation(Location l){location=l;}public int getIndex(){return index;}public boolean isActive(){return true;}
 public static class Interact{Item owner;Interact(Item i){owner=i;}public void setDefault(){oracle.Support.register((GroundItem)owner);}}
}
