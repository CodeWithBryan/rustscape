"""Compose the real original movement classes with required external services."""
import importlib.util
def compose(adapter,read,c,server,run):
    # Resolve against this package rather than depending on caller cwd.
    root=__import__('pathlib').Path(__file__).resolve().parents[2]
    spec=importlib.util.spec_from_file_location('navigation_adapters',root/'oracle/navigation/adapters.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
    base.compose(adapter,read,c,server,run)
    p=run/'core/game/node/Node.java';text=p.read_text(encoding='utf-8').replace('public class Node{','public class Node{public Node(){}public Node(String n,Location l){name=n;location=l;}')
    text=text.replace('oracle.Support.emit("active",name);','')
    adapter('core/game/node/Node.java',text)
    p=run/'core/game/node/entity/Entity.java';text=p.read_text(encoding='utf-8')
    text=text.replace('public class Entity extends Node{','public class Entity extends Node{public Entity(){super();}public Entity(String n,Location l){super(n,l);}')
    text=text.replace('public Props props=', 'public core.game.node.entity.impl.WalkingQueue walkingQueue=walking;public Props props=')
    text=text.replace('public PulseManager getPulseManager(){return new PulseManager();}','public core.game.node.entity.impl.PulseManager manager=new core.game.node.entity.impl.PulseManager(this);public core.game.node.entity.impl.PulseManager getPulseManager(){return manager;}')
    text=text.replace('public boolean isMovementLocked(){return false;}','public boolean isMovementLocked(){oracle.Support.emit("locked");return oracle.Support.locked;}')
    text=text.replace('public static class Scripts{Entity e;', 'public static class Scripts{Entity e;public void reset(){oracle.Support.emit("resetScripts",e.name);}public void preMovement(){oracle.Support.emit("pre",e.name);}public void postMovement(boolean b){oracle.Support.emit("post",e.name,b);}')
    text=text.replace('public int agility=1,incrementer;', 'public Skills skills=new Skills();public Timers timers=new Timers();public Masks updateMasks=new Masks();public static class Skills{public void pulse(){oracle.Support.emit("skills");}}public static class Timers{public void processTimers(){oracle.Support.emit("timers");}}public static class Masks{public void prepare(Entity e){oracle.Support.emit("masks",e.name);}}public static class TickEvent{int tick;public TickEvent(int t){tick=t;}}public void dispatch(TickEvent e){oracle.Support.emit("tickEvent",name,e.tick);}public int agility=1,incrementer;')
    text=text.replace('import core.game.interaction.MovementPulse;','import core.game.interaction.MovementPulse;import core.game.world.GameWorld;import java.util.Objects;')
    source=read(server/'core/game/node/entity/Entity.java');text=text[:-1]+c.method(source,'public void tick()')+'}'
    adapter('core/game/node/entity/Entity.java',text)
    p=run/'core/game/node/entity/player/Player.java';text=p.read_text(encoding='utf-8').replace('public class Player extends core.game.node.entity.Entity{','public class Player extends core.game.node.entity.Entity{public Player(String n,core.game.world.map.Location l){super(n,l);}public Interfaces getInterfaceManager(){return new Interfaces();}public Dialogue getDialogueInterpreter(){return new Dialogue();}public void faceLocation(core.game.world.map.Location l){oracle.Support.emit("faceLocation",name);}public void debug(String s){oracle.Support.emit("debug",name,s);}public static class Definition{public boolean isWalkable(){return false;}}public static class UI{public Definition getDefinition(){return new Definition();}}public static class Interfaces{public boolean isOpened(){return false;}public boolean hasChatbox(){return false;}public UI getOpened(){return new UI();}public UI getChatbox(){return new UI();}public boolean close(){throw new UnsupportedOperationException();}public void closeChatbox(){throw new UnsupportedOperationException();}}public class Dialogue{public boolean close(){oracle.Support.emit("dialogue",name);return true;}}')
    adapter('core/game/node/entity/player/Player.java',text)
    source=read(server/'core/net/packet/PacketProcessor.kt')
    adapter('core/net/packet/PacketProcessor.kt','''package core.net.packet
import core.game.node.entity.player.Player
import core.game.world.map.Location
import core.api.utils.Vector
import core.game.interaction.MovementPulse
import core.ServerConstants
import core.net.packet.`in`.Packet
object PacketProcessor{@JvmStatic fun handle(pkt:Packet){processWalkPacket(pkt)};fun sendClearMinimap(p:Player){oracle.Support.emit("minimap",p.name)}
'''+c.method(source,'private fun processWalkPacket(pkt: Packet)')+'}')
    adapter('core/net/packet/in/Packet.java','''package core.net.packet.in;import core.game.node.entity.player.Player;public class Packet{public static class WorldspaceWalk extends Packet{public Player player;public int destX,destY;public boolean isRun;}public static class InteractWalk extends Packet{public Player player;public int destX,destY;public boolean isRun;}public static class MinimapWalk extends Packet{public Player player;public int destX,destY;public boolean isRun;}}''')
    adapter('core/game/world/GameWorld.kt','''package core.game.world
object System{@JvmStatic fun currentTimeMillis()=0L}
object GameWorld{@JvmStatic var Pulser=PulseRunner();@JvmStatic var ticks=0;var settings:Settings?=Settings()};class Settings{var smartpathfinder_bfs=false}''')
    adapter('core/ServerConstants.java','package core;public class ServerConstants{public static int MAX_PATHFIND_DISTANCE=25;public static boolean GRAFANA_LOGGING=false;}')
    adapter('core/game/node/entity/combat/DeathTask.java','package core.game.node.entity.combat;public class DeathTask{public static boolean isDead(core.game.node.entity.Entity e){throw new UnsupportedOperationException();}}')
    adapter('core/game/node/entity/combat/CombatPulse.java','package core.game.node.entity.combat;public abstract class CombatPulse extends core.game.system.task.Pulse{}')
    adapter('core/game/bots/GeneralBotCreator.java','package core.game.bots;public class GeneralBotCreator{public abstract static class BotScriptPulse extends core.game.system.task.Pulse{public Object botScript;}}')
    adapter('core/tools/Log.java','package core.tools;public enum Log{ERR,WARN,FINE;}')
    adapter('core/integrations/grafana/Grafana.kt','''package core.integrations.grafana
object Grafana{fun addPulseLength(n:String,v:Int){oracle.Support.emit("length",n,v)};fun countPulse(n:String){oracle.Support.emit("count",n)};var botPulseTime=0;var otherPulseTime=0}''')
    p=run/'core/api/ContentAPI.kt';adapter('core/api/ContentAPI.kt',p.read_text(encoding='utf-8').replace('fun log(c:Class<*>,l:core.tools.Log,s:String){}','fun log(c:Class<*>,l:core.tools.Log,s:String){oracle.Support.emit("log",l.name,s)}'))
