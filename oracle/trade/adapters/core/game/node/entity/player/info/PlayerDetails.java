package core.game.node.entity.player.info;

import core.game.node.entity.player.Player;
import core.net.IoSession;

/** The three login-detail strings and the session; values come from the case. */
public class PlayerDetails {
	private final Player player;
	private final String ip;
	private final String mac;
	private final String host;

	public PlayerDetails(Player player, String ip, String mac, String host) {
		this.player = player;
		this.ip = ip;
		this.mac = mac;
		this.host = host;
	}

	public String getIpAddress() {
		return ip;
	}

	public String getMacAddress() {
		return mac;
	}

	public String getCompName() {
		return host;
	}

	public IoSession getSession() {
		return player.getSession();
	}
}