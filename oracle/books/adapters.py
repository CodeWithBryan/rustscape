"""Recording adapters for the books oracle.

Everything here stands in for a system outside the compiled originals
(BookInterface.kt and the five book classes). `/* ORIGINAL_* */` markers are
replaced with pinned original text at generation time (see generate.py
`markers`). Adapter fields never share a name with a getter's Kotlin
property.
"""

ADAPTERS = {
    'core/ServerConstants.kt': '''package core
class ServerConstants {
    companion object {
        @JvmField var SERVER_NAME: String = ""
    }
}
''',
    'core/game/world/GameWorld.kt': '''package core.game.world
object GameWorld {
    @JvmStatic var settings: GameSettings? = null
}
class GameSettings(var name: String)
''',
    'core/api/ContentInterface.java': 'package core.api;\npublic interface ContentInterface {}\n',
    'core/api/ContentAPI.kt': '''package core.api
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.component.Component
/* ORIGINAL_GET_ATTRIBUTE */
/* ORIGINAL_SET_ATTRIBUTE */
/* ORIGINAL_OPEN_INTERFACE */
/* ORIGINAL_CLOSE_INTERFACE */
fun storeBookInHouse(player: Player, node: Node): Boolean {
    oracle.Support.emit("storeBookInHouse", player.name, node.name)
    return oracle.Support.answer("storeBookInHouse", true)
}
''',
    'core/game/component/Component.java': 'package core.game.component;public class Component{private final int componentId;public Component(int id){componentId=id;}public int getId(){return componentId;}}',
    'core/game/node/Node.java': '''package core.game.node;
public class Node{
 private final String nodeName;
 public Node(String n){nodeName=n;}
 public String getName(){return nodeName;}
}''',
    'core/game/node/entity/Entity.java': '''package core.game.node.entity;
import java.util.*;
public class Entity extends core.game.node.Node{
 public final Map<String,Object> attributeMap=new HashMap<>();
 public Entity(String name){super(name);}
 public void setAttribute(String key,Object value){oracle.Support.emit("setAttribute",getName(),key,oracle.Support.value(value));attributeMap.put(key,value);}
 public <T> T getAttribute(String key,T fail){oracle.Support.emit("getAttribute",getName(),key);Object o=attributeMap.get(key);if(o==null)return fail;return(T)o;}
}''',
    'core/game/node/entity/player/Player.java': '''package core.game.node.entity.player;
import core.game.component.Component;
public class Player extends core.game.node.entity.Entity{
 private final Interfaces interfacesObject=new Interfaces(this);private final Dispatch dispatchObject=new Dispatch(this);
 public Player(String name){super(name);}
 public Interfaces getInterfaceManager(){return interfacesObject;}
 public Dispatch getPacketDispatch(){return dispatchObject;}
 public static class Interfaces{
  private final Player owner;Interfaces(Player p){owner=p;}
  public boolean close(){oracle.Support.emit("closeInterface",owner.getName());return oracle.Support.answer("closeInterface",true);}
  public void open(Component c){oracle.Support.emit("openInterface",owner.getName(),c.getId());}
 }
 public static class Dispatch{
  private final Player owner;Dispatch(Player p){owner=p;}
  public void sendString(String s,int iface,int child){oracle.Support.emit("sendString",owner.getName(),s,iface,child);}
  public void sendInterfaceConfig(int iface,int child,boolean hidden){oracle.Support.emit("interfaceConfig",owner.getName(),iface,child,hidden);}
  public void sendModelOnInterface(int model,int iface,int child,int zoom){oracle.Support.emit("modelOnInterface",owner.getName(),model,iface,child,zoom);}
  public void sendAngleOnInterface(int iface,int child,int zoom,int pitch,int yaw){oracle.Support.emit("angleOnInterface",owner.getName(),iface,child,zoom,pitch,yaw);}
  public void sendItemOnInterface(int item,int amount,int iface,int child){oracle.Support.emit("itemOnInterface",owner.getName(),item,amount,iface,child);}
 }
}''',
    'core/game/interaction/InteractionListener.kt': '''package core.game.interaction
import core.api.ContentInterface
import core.game.node.Node
import core.game.node.entity.player.Player
interface InteractionListener : ContentInterface {
    fun defineListeners()
    fun on(id: Int, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(id, type.name, arrayOf(*option), handler)
    }
}
''',
    'core/game/interaction/InterfaceListener.kt': '''package core.game.interaction
import core.api.ContentInterface
import core.game.component.Component
import core.game.node.entity.player.Player
interface InterfaceListener : ContentInterface {
    fun defineInterfaceListeners()
    fun on(componentID: Int, handler: (player: Player, component: Component, opcode: Int, buttonID: Int, slot: Int, itemID: Int) -> Boolean) {
        oracle.Support.onInterface(componentID, handler)
    }
}
''',
}
