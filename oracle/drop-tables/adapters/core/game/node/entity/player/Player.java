package core.game.node.entity.player;

import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;

/**
 * A looting player. The oracle reads only the case's recorded answers; every
 * method createDrop would call is compile-only.
 */
public class Player extends Entity {
	public boolean wealth;
	public java.util.List<Long> equipped = new java.util.ArrayList<>();
	public boolean hasSpace;
	public boolean full;
	private final Container inventory = new Container(this);

	public Container getInventory() {
		return inventory;
	}

	public void sendMessage(String message) {
		throw new UnsupportedOperationException();
	}

	public <T> T getAttribute(String key, T fallback) {
		throw new UnsupportedOperationException();
	}

	public void setAttribute(String key, Object value) {
		throw new UnsupportedOperationException();
	}

	public Communication getCommunication() {
		throw new UnsupportedOperationException();
	}

	public IronmanManager getIronmanManager() {
		throw new UnsupportedOperationException();
	}

	public GlobalData getGlobalData() {
		throw new UnsupportedOperationException();
	}

	public InterfaceManager getInterfaceManager() {
		throw new UnsupportedOperationException();
	}

	public String getUsername() {
		throw new UnsupportedOperationException();
	}

	public Location getLocation() {
		throw new UnsupportedOperationException();
	}

	public Skills getSkills() {
		throw new UnsupportedOperationException();
	}

	public enum Rank {
		NONE
	}

	public static class Clan {
		public Rank getLootRequirement() {
			throw new UnsupportedOperationException();
		}
	}

	public static class Communication {
		public Clan getClan() {
			throw new UnsupportedOperationException();
		}

		public boolean isLootShare() {
			throw new UnsupportedOperationException();
		}

		public Rank getLootRequirement() {
			throw new UnsupportedOperationException();
		}
	}

	public static class IronmanManager {
		public boolean isIronman() {
			throw new UnsupportedOperationException();
		}
	}

	public static class GlobalData {
		public int getLootSharePoints() {
			throw new UnsupportedOperationException();
		}

		public void setLootSharePoints(int points) {
			throw new UnsupportedOperationException();
		}

		public boolean isEnableBoneCrusher() {
			throw new UnsupportedOperationException();
		}
	}

	public static class InterfaceManager {
		public boolean isResizable() {
			throw new UnsupportedOperationException();
		}
	}
}
