package oracle;
import java.util.*;import oracle.Oracle.*;
public class Interpreter {
 public final int id;private final Player player;public Object active;private final List<Action> actions=new ArrayList<>();
 public Interpreter(int id,Player player){this.id=id;this.player=player;}
 public Object getDialogue(){Oracle.emit("dialogue",id);return active;}
 public List<Action> rawActions(){return actions;}
 public List<Action> getActions(){Oracle.emit("actions",id);return actions;}
 public void handle(int iface,int child){Oracle.emit("activeHandle",id,iface,child);}
 /* ORIGINAL_SEND */
}
