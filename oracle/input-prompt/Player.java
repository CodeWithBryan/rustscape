package core.game.node.entity.player;
import java.util.*;import oracle.Oracle;
public class Player {
 public final int id;public Map<String,Object> attributes=new TreeMap<>();
 private Oracle.Bank bank;private Oracle.Interpreter interpreter;private Oracle.Locks locks;
 public Player(int id){this.id=id;bank=new Oracle.Bank(id);interpreter=new Oracle.Interpreter(id);locks=new Oracle.Locks(id);}
 public <T>T getAttribute(String key,T fallback){Oracle.emit("get",id,key);Object v=attributes.get(key);return v==null?fallback:(T)v;}
 public void setAttribute(String key,Object v){attributes.put(key,v);Oracle.emit("set",id,key,Oracle.value(v));}
 public void removeAttribute(String key){attributes.remove(key);Oracle.emit("remove",id,key);}
 public Oracle.Bank getBank(){Oracle.emit("bank",id);return bank;}
 public Oracle.Interpreter getDialogueInterpreter(){Oracle.emit("interpreter",id);return interpreter;}
 public Oracle.Locks getLocks(){Oracle.emit("locks",id);return locks;}
 public void bank(Oracle.Bank v){bank=v;}public void interpreter(Oracle.Interpreter v){interpreter=v;}public void locks(Oracle.Locks v){locks=v;}
}
