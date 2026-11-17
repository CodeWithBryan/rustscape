package core.game.node.entity.player;

import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.player.link.GlobalData;
import core.game.node.entity.player.link.HintIconManager;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.SkullManager;
import core.game.node.entity.player.link.WarningMessages;
import core.game.node.entity.player.link.diary.AchievementDiaryManager;
import core.game.node.entity.skill.Skills;
import core.game.node.item.Item;
import core.net.IoSession;
import java.util.Arrays;
import java.util.Map;
import oracle.Support;

/**
 * Recording Player. The five managers under test are the real classes; every
 * other member records its call and answers from the case. debug and hasItem
 * are the original text.
 */
@SuppressWarnings({"rawtypes", "unchecked"})
public class Player extends Entity {
	private final String username;
	private final boolean artificial;
	private final Map spec;
	private final SkullManager skullManager;
	private final HintIconManager hintIconManager = new HintIconManager();
	private final WarningMessages warningMessages = new WarningMessages();
	private final AchievementDiaryManager achievementDiaryManager;
	private final Appearance appearance = new Appearance();
	private final Locks locks = new Locks();
	private final Ironman ironman = new Ironman();
	private final SavedData savedData = new SavedData();
	private final Container inventory;
	private final Container equipment;
	private final Container bank;
	private final IoSession session;
	private final InterfaceManager interfaceManager;
	private final PacketDispatch packetDispatch;
	private final Skills skills;

	public Player(int slot, Map spec) {
		super(slot, spec);
		this.spec = spec;
		this.username = (String) Support.get(spec, "name", "player" + slot);
		this.artificial = Support.flag(spec, "artificial", false);
		this.inventory = new Container(this, "inventory", 28, spec.get("inventory"));
		this.equipment = new Container(this, "equipment", 14, spec.get("equipment"));
		this.bank = new Container(this, "bank", 496, spec.get("bank"));
		this.session = new IoSession(this);
		this.interfaceManager = new InterfaceManager(this);
		this.packetDispatch = new PacketDispatch(this);
		this.skills = new Skills(this, (Map) Support.get(spec, "levels", new java.util.HashMap()));
		this.skullManager = new SkullManager(this);
		this.achievementDiaryManager = new AchievementDiaryManager(this);
	}

	public class Appearance {
		public void setSkullIcon(int icon) {
			Support.emit("skullIcon", slot, icon);
		}

		public void sync() {
			Support.emit("appearanceSync", slot);
		}
	}

	public class Locks {
		public void lockTeleport(int ticks) {
			Support.emit("lockTeleport", slot, ticks);
		}

		public void unlockTeleport() {
			Support.emit("unlockTeleport", slot);
		}
	}

	public class Ironman {
		public boolean checkRestriction() {
			Support.emit("ironmanRestriction", slot);
			Boolean answer = Support.answer("ironmanRestriction");
			return answer != null && answer;
		}
	}

	public class SavedData {
		private final GlobalData globalData = new GlobalData();

		public GlobalData getGlobalData() {
			Support.emit("globalData", slot);
			return globalData;
		}

		public GlobalData raw() {
			return globalData;
		}
	}

	public SkullManager getSkullManager() {
		return skullManager;
	}

	public HintIconManager getHintIconManager() {
		return hintIconManager;
	}

	public WarningMessages getWarningMessages() {
		return warningMessages;
	}

	public AchievementDiaryManager getAchievementDiaryManager() {
		return achievementDiaryManager;
	}

	public Appearance getAppearance() {
		return appearance;
	}

	public void updateAppearance() {
		Support.emit("updateAppearance", slot);
	}

	public Locks getLocks() {
		return locks;
	}

	public Ironman getIronmanManager() {
		return ironman;
	}

	public SavedData getSavedData() {
		return savedData;
	}

	public Container getInventory() {
		return inventory;
	}

	public Container getEquipment() {
		return equipment;
	}

	public Container getBank() {
		return bank;
	}

	public IoSession getSession() {
		return session;
	}

	public InterfaceManager getInterfaceManager() {
		return interfaceManager;
	}

	public PacketDispatch getPacketDispatch() {
		return packetDispatch;
	}

	public Skills getSkills() {
		return skills;
	}

	public String getUsername() {
		Support.emit("username", slot);
		return username;
	}

	public boolean isArtificial() {
		Support.emit("isArtificial", slot);
		return artificial;
	}

	public boolean hasModalOpen() {
		return false;
	}

	public void sendMessage(String message) {
		Support.emit("sendMessage", slot, message);
	}

	public void sendMessages(String... messages) {
		Support.emit("sendMessages", slot, Arrays.asList(messages));
	}

	/* ORIGINAL_DEBUG */

	/* ORIGINAL_HAS_ITEM */
}
