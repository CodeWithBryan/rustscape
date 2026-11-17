package core.game.component;
import core.game.node.entity.player.Player;
public class ComponentPlugin {
 public int label,count;public boolean result=true;public ComponentPlugin(int l){label=l;}
 public boolean handle(Player p,Component c,int opcode,int child,int slot,int item){oracle.Oracle.emit("pluginHandle",oracle.Oracle.pluginRef(this),label,p.id,oracle.Oracle.componentRef(c),opcode,child,slot,item);count++;return result;}
}
