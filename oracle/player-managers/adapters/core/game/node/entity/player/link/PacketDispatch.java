package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import oracle.Support;

/** The dispatch members these classes reach, recorded with their arguments. */
public class PacketDispatch {
	private final Player player;

	public PacketDispatch(Player player) {
		this.player = player;
	}

	public void sendMessage(String message) {
		Support.emit("message", player.slot, message);
	}

	public void sendString(String string, int interfaceId, int lineId) {
		Support.emit("string", player.slot, string, interfaceId, lineId);
	}

	public void sendInterfaceConfig(int interfaceId, int childId, boolean hide) {
		Support.emit("ifaceConfig", player.slot, interfaceId, childId, hide);
	}

	public void sendRunScript(int id, String string, Object... objects) {
		List<Object> args = new ArrayList<>(Arrays.asList(objects));
		Support.emit("runScript", player.slot, id, string, args);
	}
}
