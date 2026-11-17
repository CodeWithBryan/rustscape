package core.game.node.entity.player.info;

import core.game.node.entity.player.Player;
import core.net.IoSession;

/** The account uid (from the case), the session and getDetails (a recorded service). */
public class PlayerDetails {
	private final Player player;
	public final Account accountInfo;

	public PlayerDetails(Player player, int uid) {
		this.player = player;
		this.accountInfo = new Account(uid);
	}

	public int getUid() {
		return accountInfo.getUid();
	}

	public IoSession getSession() {
		return player.getSession();
	}

	public static PlayerDetails getDetails(String username) {
		return oracle.Support.details(username);
	}

	/** UserAccountInfo's uid. */
	public static class Account {
		private final int uid;

		public Account(int uid) {
			this.uid = uid;
		}

		public int getUid() {
			return uid;
		}
	}
}