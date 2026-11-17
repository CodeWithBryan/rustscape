package core.game.component;
public class Component {
 private int id;private boolean hidden;private ComponentPlugin plugin;
 public Component(int n){id=n;}public int getId(){return id;}public int rawId(){return id;}public void rawId(int n){id=n;}
 public boolean rawHidden(){return hidden;}public void rawHidden(boolean n){hidden=n;}public ComponentPlugin rawPlugin(){return plugin;}public void rawPlugin(ComponentPlugin p){plugin=p;}
 public boolean isHidden(){boolean n=hidden;oracle.Oracle.emit("hidden",oracle.Oracle.componentRef(this));return n;}
 public ComponentPlugin getPlugin(){ComponentPlugin p=plugin;oracle.Oracle.emit("pluginGet",oracle.Oracle.componentRef(this),oracle.Oracle.pluginRef(p));return p;}
}
