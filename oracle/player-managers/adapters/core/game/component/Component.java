package core.game.component;

import core.game.node.entity.player.Player;
import oracle.Support;

/** A component id with its close event; close runs the event as Component.close does. */
public class Component {
	private final int id;
	private CloseEvent closeEvent;

	public Component(int id) {
		this.id = id;
	}

	public int getId() {
		return id;
	}

	public Component setCloseEvent(CloseEvent closeEvent) {
		this.closeEvent = closeEvent;
		return this;
	}

	public CloseEvent getCloseEvent() {
		return closeEvent;
	}

	public boolean close(Player player) {
		Support.emit("componentClose", player.slot, id);
		return closeEvent == null || closeEvent.close(player, this);
	}
}
