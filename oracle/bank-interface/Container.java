package core.game.container;
import java.util.*;import oracle.Oracle;
public class Container {
 public final int ref;public Map<Integer,Item>items=new TreeMap<>();public int tab=10,last=50;public boolean insert,note;
 public Container(int r){ref=r;}
 public Item get(int slot){Item item=items.get(slot);Oracle.emit("item",ref,slot,Oracle.itemRef(item));return item;}
 int amount(int id){int n=0;for(Item item:items.values())if(item!=null&&item.id()==id)n+=item.amount;return n;}
 public int getAmount(Item item){int n=amount(item.id());Oracle.emit("amount",ref,Oracle.itemRef(item));return n;}
 public int getAmount(int id){int n=amount(id);Oracle.emit("amountId",ref,id);return n;}
 public int getLastAmountX(){int n=last;Oracle.emit("last",ref);return n;}
 public int getTabIndex(){int n=tab;Oracle.emit("tab",ref);return n;}
 public void setTabIndex(int n){tab=n;Oracle.emit("setTab",ref,n);}
 public boolean isInsertItems(){boolean n=insert;Oracle.emit("insert",ref);return n;}
 public void setInsertItems(boolean n){insert=n;Oracle.emit("setInsert",ref,n);}
 public boolean isNoteItems(){boolean n=note;Oracle.emit("note",ref);return n;}
 public void setNoteItems(boolean n){note=n;Oracle.emit("setNote",ref,n);}
 public void collapseTab(int n){Oracle.emit("collapse",ref,n);}
 public void takeItem(int slot,int n){Oracle.emit("take",ref,slot,n);}
 public void addItem(int slot,int n){Oracle.emit("deposit",ref,slot,n);}
 public static class Item {private Definition definition;public int amount;public Item(Definition d,int a){definition=d;amount=a;}public Definition rawDefinition(){return definition;}public void rawDefinition(Definition d){definition=d;}public Definition getDefinition(){Definition d=definition;Oracle.emit("definition",Oracle.itemRef(this),Oracle.defRef(d));return d;}int id(){return definition.id;}}
 public static class Definition {private int id;private String examine,name;public Definition(int i,String n,String e){id=i;name=n;examine=e;}public int rawId(){return id;}public void rawId(int n){id=n;}public String rawName(){return name;}public void rawName(String n){name=n;}public String rawExamine(){return examine;}public void rawExamine(String n){examine=n;}public int getId(){int n=id;Oracle.emit("id",Oracle.defRef(this));return n;}public String getName(){String n=name;Oracle.emit("name",Oracle.defRef(this));return n;}public String getExamine(){String n=examine;Oracle.emit("examine",Oracle.defRef(this));return n;}}
}
