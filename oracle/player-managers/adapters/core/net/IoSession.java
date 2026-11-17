package core.net;

import core.cache.crypto.ISAACCipher;
import core.game.node.entity.player.Player;
import core.net.packet.IoBuffer;

/** Zero ISAAC and a recording write: the frame's opcode and payload bytes. */
public class IoSession {
	private final Player owner;

	public IoSession(Player owner) {
		this.owner = owner;
	}

	public IoSession getIsaacPair() {
		return this;
	}

	public ISAACCipher getOutput() {
		return new ISAACCipher();
	}

	public void write(IoBuffer buffer) {
		oracle.Support.sent(owner, buffer);
	}
}
