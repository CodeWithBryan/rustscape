package core.net;

import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.ClientInfo;

/** Fixed-mode client info, zero ISAAC and a recording write. */
public class IoSession {
	private final Player owner;
	private final ClientInfo info = new ClientInfo(0, 1, 0, 0);

	public IoSession(Player owner) {
		this.owner = owner;
	}

	public ClientInfo getClientInfo() {
		return info;
	}

	public IoSession getIsaacPair() {
		return this;
	}

	public core.cache.crypto.ISAACCipher getOutput() {
		return new core.cache.crypto.ISAACCipher();
	}

	public void write(core.net.packet.IoBuffer b) {
		oracle.Support.sent(owner, b);
	}
}
