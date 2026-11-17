package core.game.node.entity.player;

import core.game.container.Container;
import core.game.dialogue.DialogueInterpreter;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.link.IronmanManager;
import core.game.node.entity.player.link.IronmanMode;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.PacketDispatch;
import core.net.IoSession;
import java.util.HashMap;
import java.util.Map;

/**
 * Recording Player for the GE oracle. Every member the unchanged classes
 * reach is a method (no public field shadows a Kotlin getter); answers come
 * from the case. Attributes are a plain map read as GameAttributes reads it
 * (a null value answers the default).
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public class Player extends core.game.node.entity.Entity {
	private final String username;
	private final Map<String, Object> attributes = new HashMap<>();
	private final InterfaceManager interfaceManager;
	private final PacketDispatch packetDispatch;
	private final DialogueInterpreter dialogueInterpreter;
	private final IronmanManager ironmanManager;
	private final PlayerDetails details;
	private final IoSession session;
	private final Container inventory = new Container(28);
	private final BankPin bankPinManager = new BankPin();
	private final boolean artificial;
	public boolean pinUnlocked;

	public Player(Map spec) {
		super((String) spec.get("name"), null);
		this.username = (String) spec.get("name");
		this.artificial = Boolean.TRUE.equals(spec.get("artificial"));
		this.pinUnlocked = !Boolean.FALSE.equals(spec.get("pin"));
		this.session = new IoSession(this);
		this.details = new PlayerDetails(this, ((Number) spec.get("uid")).intValue());
		this.interfaceManager = new InterfaceManager(this);
		this.packetDispatch = new PacketDispatch(this);
		this.dialogueInterpreter = new DialogueInterpreter(this);
		this.ironmanManager = new IronmanManager(this);
		this.ironmanManager.setMode(IronmanMode.valueOf((String) spec.getOrDefault("ironman", "NONE")));
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

	public boolean isArtificial() {
		return artificial;
	}

	private final Map<Class, Object> extensions = new HashMap<>();

	public <T> T getExtension(Class<T> type) {
		return (T) extensions.get(type);
	}

	public void addExtension(Class type, Object value) {
		extensions.put(type, value);
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

	public BankPin getBankPinManager() {
		return bankPinManager;
	}

	/** BankPinManager's isUnlocked and openType: recorded services. */
	public class BankPin {
		public boolean isUnlocked() {
			oracle.Support.emit("pinUnlocked", oracle.Support.index(Player.this));
			return pinUnlocked;
		}

		public void openType(int type) {
			oracle.Support.emit("pinOpen", oracle.Support.index(Player.this), type);
		}
	}

	@Override
	public <T> T getAttribute(String key, T fail) {
		Object value = attributes.get(key);
		if (value != null) {
			return (T) value;
		}
		return fail;
	}

	public <T> T getAttribute(String key) {
		return (T) attributes.get(key);
	}

	@Override
	public void setAttribute(String key, Object value) {
		attributes.put(key, value);
	}

	@Override
	public void removeAttribute(String key) {
		attributes.remove(key);
	}

	public Map<String, Object> attributes() {
		return attributes;
	}

	/** Player.sendMessage: a PacketDispatch.sendMessage per line (recorded). */
	public void sendMessage(String text) {
		oracle.Support.emit("message", oracle.Support.index(this), text);
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