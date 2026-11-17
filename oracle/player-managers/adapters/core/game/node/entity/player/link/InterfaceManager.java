package core.game.node.entity.player.link;

import core.game.component.Component;
import core.game.node.entity.player.Player;
import oracle.Support;

/**
 * The manager members these classes reach. One opened field: open stores the
 * component, close runs its close event and clears the field when the event
 * allows it, isOpened reads the field. Tabs and the rest record only.
 */
public class InterfaceManager {
	private final Player player;
	private Component opened;

	public InterfaceManager(Player player) {
		this.player = player;
	}

	public Component getOpened() {
		return opened;
	}

	public Component open(Component component) {
		Support.emit("open", player.slot, component.getId());
		opened = component;
		return component;
	}

	public boolean close() {
		Support.emit("close", player.slot);
		if (opened != null && opened.close(player)) {
			opened = null;
		}
		return opened == null;
	}

	public boolean isOpened() {
		Support.emit("isOpened", player.slot);
		return opened != null;
	}

	public void openTab(int slot, Component component) {
		Support.emit("openTab", player.slot, slot, component.getId());
	}

	public void restoreTabs() {
		Support.emit("restoreTabs", player.slot);
	}
}
