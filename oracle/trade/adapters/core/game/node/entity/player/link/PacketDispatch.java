package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;
import core.net.packet.PacketRepository;
import core.net.packet.context.AccessMaskContext;
import core.net.packet.context.InterfaceConfigContext;
import core.net.packet.context.RunScriptContext;
import core.net.packet.context.StringContext;
import core.net.packet.out.AccessMask;
import core.net.packet.out.InterfaceConfig;
import core.net.packet.out.RunScriptPacket;
import core.net.packet.out.StringPacket;

/** The original sendString/sendIfaceSettings/sendRunScript/sendInterfaceConfig bodies; sendMessage is a recorded service. */
public class PacketDispatch {
	private final Player player;

	public PacketDispatch(Player player) {
		this.player = player;
	}

	public void sendMessage(String message) {
		oracle.Support.emit("message", oracle.Support.index(player), message);
	}

	/* ORIGINAL_SEND_STRING */

	/* ORIGINAL_SEND_IFACE_SETTINGS */

	/* ORIGINAL_SEND_RUN_SCRIPT */

	/* ORIGINAL_SEND_INTERFACE_CONFIG */
}