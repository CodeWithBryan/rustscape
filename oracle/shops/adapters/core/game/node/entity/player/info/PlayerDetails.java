package core.game.node.entity.player.info;

import core.game.node.entity.player.Player;
import core.net.IoSession;

/** The account uid (from the case) and the session. */
public class PlayerDetails {
	private final Player player;
	private final int uid;

	public PlayerDetails(Player player, int uid) {
		this.player = player;
		this.uid = uid;
	}

	public int getUid() {
		return uid;
	}

	public IoSession getSession() {
		return player.getSession();
	}
}
