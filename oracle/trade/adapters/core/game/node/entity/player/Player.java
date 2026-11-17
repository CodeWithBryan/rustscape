package core.game.node.entity.player;

import core.game.container.Container;
import core.game.dialogue.DialogueInterpreter;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.link.IronmanManager;
import core.game.node.entity.player.link.IronmanMode;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.request.RequestManager;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneMonitor;
import core.net.IoSession;
import java.util.HashMap;
import java.util.Map;

/**
 * Recording Player for the trade oracle. Every member the unchanged classes
 * reach is a method (no public field shadows a Kotlin getter); answers come
 * from the case. Attributes and extensions are plain maps keyed as in Entity.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public class Player extends core.game.node.entity.Entity {
	private final int number;
	private final String username;
	private Location place;
	private boolean active;
	private final boolean artificial;
	private final Map<String, Object> attributes = new HashMap<>();
	private final Map<Class, Object> extensions = new HashMap<>();
	private final InterfaceManager interfaceManager;
	private final PacketDispatch packetDispatch;
	private final DialogueInterpreter dialogueInterpreter;
	private final ZoneMonitor zoneMonitor;
	private final RequestManager requestManager;
	private final IronmanManager ironmanManager;
	private final PlayerDetails details;
	private final IoSession session;
	private final Container inventory = new Container(28);

	public Player(int number, Map spec) {
		super((String) spec.get("name"), null);
		this.number = number;
		this.username = (String) spec.get("name");
		this.place = new Location(((Number) spec.get("x")).intValue(), ((Number) spec.get("y")).intValue(), ((Number) spec.get("z")).intValue());
		this.active = !Boolean.FALSE.equals(spec.get("active"));
		this.artificial = Boolean.TRUE.equals(spec.get("artificial"));
		this.session = new IoSession(this);
		this.details = new PlayerDetails(this, (String) spec.get("ip"), (String) spec.get("mac"), (String) spec.get("host"));
		this.interfaceManager = new InterfaceManager(this);
		this.packetDispatch = new PacketDispatch(this);
		this.dialogueInterpreter = new DialogueInterpreter(this);
		this.zoneMonitor = new ZoneMonitor(this);
		this.requestManager = new RequestManager(this);
		this.ironmanManager = new IronmanManager(this);
		this.ironmanManager.setMode(IronmanMode.valueOf((String) spec.getOrDefault("ironman", "NONE")));
		if (Boolean.TRUE.equals(spec.get("bot"))) {
			attributes.put("not_on_highscores", true);
		}
		Container source = new Container(28);
		for (Object row : (java.util.List) spec.get("inv")) {
			java.util.List r = (java.util.List) row;
			source.replace(new core.game.node.item.Item(((Number) r.get(1)).intValue(), ((Number) r.get(2)).intValue()), ((Number) r.get(0)).intValue(), false);
		}
		inventory.copy(source);
		inventory.register(new oracle.Support.InventoryRecorder(this));
	}

	public String getUsername() {
		return username;
	}

	public boolean isActive() {
		return active;
	}

	public void setActive(boolean active) {
		this.active = active;
	}

	public Location getLocation() {
		return place;
	}

	public void setLocation(Location location) {
		this.place = location;
	}

	public boolean isArtificial() {
		return artificial;
	}

	public InterfaceManager getInterfaceManager() {
		return interfaceManager;
	}

	public PacketDispatch getPacketDispatch() {
		return packetDispatch;
	}

	public DialogueInterpreter getDialogueInterpreter() {
		return dialogueInterpreter;
	}

	public ZoneMonitor getZoneMonitor() {
		return zoneMonitor;
	}

	public RequestManager getRequestManager() {
		return requestManager;
	}

	public IronmanManager getIronmanManager() {
		return ironmanManager;
	}

	public PlayerDetails getDetails() {
		return details;
	}

	public IoSession getSession() {
		return session;
	}

	public Container getInventory() {
		return inventory;
	}

	public <T> T getAttribute(String key, T fail) {
		Object value = attributes.get(key);
		if (value != null) {
			return (T) value;
		}
		return fail;
	}

	public void setAttribute(String key, Object value) {
		attributes.put(key, value);
	}

	public void removeAttribute(String key) {
		attributes.remove(key);
	}

	public <T> T getExtension(Class<T> type) {
		return (T) extensions.get(type);
	}

	public void addExtension(Class type, Object value) {
		extensions.put(type, value);
		if (value instanceof core.game.node.entity.player.link.request.trade.TradeModule) {
			oracle.Support.module((core.game.node.entity.player.link.request.trade.TradeModule) value);
		}
	}

	public void removeExtension(Class type) {
		extensions.remove(type);
	}

	public void sendMessage(String text) {
		oracle.Support.emit("ironmanMessage", oracle.Support.index(this), text);
	}

	public void dispatch(core.game.event.InterfaceOpenEvent e) {
		oracle.Support.emit("dispatchOpen", oracle.Support.index(this), e.component == null ? null : e.component.getId());
	}

	public void dispatch(core.game.event.InterfaceCloseEvent e) {
		oracle.Support.emit("dispatchClose", oracle.Support.index(this), e.component == null ? null : e.component.getId());
	}

	public SpellBook getSpellBookManager() {
		return new SpellBook();
	}

	public Familiar getFamiliarManager() {
		return new Familiar();
	}

	public Props getProperties() {
		return new Props();
	}

	public static class SpellBook {
		public int getSpellBook() {
			return 192;
		}
	}

	public static class Familiar {
		public boolean hasFamiliar() {
			return false;
		}
	}

	public static class Props {
		public Spell getAutocastSpell() {
			return null;
		}
	}

	public static class Spell {
		public int getSpellId() {
			return 0;
		}
	}
}
