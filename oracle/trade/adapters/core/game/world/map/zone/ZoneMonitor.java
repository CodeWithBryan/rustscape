package core.game.world.map.zone;

import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.request.RequestType;

/** Recording zone monitor: canRequest answers from the case. */
public class ZoneMonitor {
	private final Player player;

	public ZoneMonitor(Player player) {
		this.player = player;
	}

	public boolean canRequest(RequestType type, Player target) {
		return oracle.Support.zone(player, type, target);
	}
}