package core.game.node.entity.player;
import core.game.component.*;import core.game.node.entity.player.link.*;import core.net.IoSession;import oracle.Support;
public class Player{
 public InterfaceManager manager;public IoSession session;public Player(int mode){manager=new InterfaceManager(this);session=new IoSession(mode);}
 public InterfaceManager getInterfaceManager(){return manager;}public IoSession getSession(){return session;}public Player getDetails(){return this;}public boolean isArtificial(){return false;}
 public <T>T getExtension(Class<T>c){return(T)Support.extension();}public void addExtension(Class c,Object o){Support.addWeapon((Component)o);}
 public void setAttribute(String key,Object value){if(key.equals("close_c_"))oracle.ObjectsSupport.setAttribute(value);}public<T>T getAttribute(String key,T fallback){if(key.equals("runscript"))return(T)oracle.ManagerSupport.getRunscript();if(key.equals("close_c_"))return(T)oracle.ObjectsSupport.getAttribute(fallback);boolean v=Support.bool("tutorial",true);Support.emit("tutorial");return(T)(Boolean)v;}
 public void removeAttribute(String key){if(key.equals("runscript"))oracle.ManagerSupport.removeRunscript();}
 public void dispatch(core.game.event.InterfaceCloseEvent e){Support.emit("dispatchClose",Support.slot(e.component));}
 public PacketDispatch getPacketDispatch(){return new PacketDispatch();}
 public class PacketDispatch{public void sendRunScript(int id,String types,Object...args){Support.emit("script",id,types,java.util.Arrays.asList(args));core.net.packet.PacketRepository.send(core.net.packet.out.RunScriptPacket.class,new core.net.packet.context.RunScriptContext(Player.this,id,types,args));}public void sendInterfaceConfig(int pane,int child,boolean hidden){Support.emit("interfaceConfig",pane,child,hidden);core.net.packet.PacketRepository.send(core.net.packet.out.InterfaceConfig.class,new core.net.packet.context.InterfaceConfigContext(Player.this,pane,child,hidden));}}
 public void dispatch(core.game.event.InterfaceOpenEvent e){Support.emit("dispatch",Support.slot(e.component));}
 public SpellBook getSpellBookManager(){return new SpellBook();}public Familiar getFamiliarManager(){return new Familiar();}public Props getProperties(){return new Props();}public Appearance getAppearance(){return new Appearance();}
 public static class SpellBook{public int getSpellBook(){int id=((Number)Support.input.getOrDefault("spellbook",192)).intValue();Support.emit("spellbook");return id;}}
 public static class Familiar{public boolean hasFamiliar(){boolean b=Support.bool("familiar",false);Support.emit("familiar");return b;}}
 public static class Props{public Spell getAutocastSpell(){Object v=Support.input.get("autocast");Support.emit("autocast");return v==null?null:new Spell(((Number)v).intValue());}}
 public static class Spell{int id;Spell(int id){this.id=id;}public int getSpellId(){return id;}}
 public static class Appearance{public void sync(){Support.emit("appearance");}}
}
